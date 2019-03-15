extern crate cargo_metadata;
extern crate chrono;
extern crate termcolor;
extern crate serde;
#[macro_use]
extern crate maplit;
#[macro_use]
extern crate quick_error;
extern crate dirs;
extern crate regex;
extern crate semver;
extern crate structopt;
extern crate toml;
extern crate toml_edit;

#[cfg(test)]
extern crate assert_fs;
#[cfg(test)]
extern crate predicates;

use std::path::Path;
use std::process::exit;

use chrono::prelude::Local;
use semver::Identifier;
use structopt::StructOpt;

use error::FatalError;
use replace::{do_file_replacements, replace_in, Replacements};

mod cargo;
mod cmd;
mod config;
mod error;
mod git;
mod replace;
mod shell;
mod version;

fn find_root_package(meta: &cargo_metadata::Metadata) -> Result<&cargo_metadata::Package, error::FatalError> {
    let resolve = meta.resolve.as_ref().expect("unclear when this is optional");
    let root_id = resolve.root.as_ref()
        // Cargo.toml has a workspace but no package
        .ok_or_else(|| error::FatalError::NoPackage)?;
    let pkg = meta.packages.iter()
        .find(|p| p.id == *root_id)
        .expect("the root package must exist");
    Ok(pkg)
}

fn find_dependents<'w>(ws_meta: &'w cargo_metadata::Metadata, pkg_meta: &'w cargo_metadata::Package) -> impl Iterator<Item=(&'w cargo_metadata::Package, &'w cargo_metadata::Dependency)> {
    ws_meta.packages.iter().filter_map(move |p| {
        if ws_meta.workspace_members.iter().find(|m| **m == p.id).is_some() {
            p.dependencies.iter().find(|d| d.name == pkg_meta.name).map(|d| (p, d))
        } else {
            None
        }
    })
}

fn execute(args: &ReleaseOpt) -> Result<i32, error::FatalError> {
    let ws_meta = cargo_metadata::MetadataCommand::new()
        .exec()
        .map_err(FatalError::from)?;
    let pkg_meta = find_root_package(&ws_meta)?;

    let manifest_path = pkg_meta.manifest_path.as_path();
    let cwd = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    let cargo_file = cargo::parse_cargo_config(&manifest_path)?;
    let custom_config_path_option = args.config.as_ref();
    // FIXME:
    let release_config = if let Some(custom_config_path) = custom_config_path_option {
        // when calling with -c option
        config::get_config_from_file(Path::new(custom_config_path))?
    } else {
        config::resolve_config(&manifest_path)?
    }.unwrap_or_default();

    // if this execution is dry-run
    let dry_run = args.dry_run;
    // the release level
    let level = args.level.as_ref();
    // flag for gpg signing git commit and tag
    let sign = args.sign || release_config.sign_commit;
    // flag for uploading doc to remote branch
    let upload_doc = args.upload_doc || release_config.upload_doc;
    // default remote for git push
    let git_remote = args.push_remote
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| release_config.push_remote.as_str());
    // default branch for doc push
    let doc_branch = args.doc_branch
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| release_config.doc_branch.as_str());
    // flag to skip `cargo publish`
    let skip_publish = args.skip_publish || release_config.disable_publish;
    // flag to skip `git push`
    let skip_push = args.skip_push || release_config.disable_push;
    // version extension to add after successful release
    let dev_version_ext = args.dev_version_ext
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| release_config.dev_version_ext.as_str());
    // do not bump version or add version extension after release
    let no_dev_version = args.no_dev_version || release_config.no_dev_version;
    // the commit message for removing extension or bump version before a release
    let pre_release_commit_msg = release_config.pre_release_commit_message.as_str();
    // the commit message for adding extension or bump version after a release
    let pro_release_commit_msg = release_config.pro_release_commit_message.as_str();
    // the replacements to execute before release
    let pre_release_replacements = &release_config.pre_release_replacements;
    // the hook script to call after release
    let pre_release_hook = release_config.pre_release_hook
        .as_ref()
        .map(|h| h.args());
    // the commit message for `git tag`
    let tag_msg = release_config.tag_message.as_str();
    // flag to skip `git tag`
    let skip_tag = args.skip_tag || release_config.disable_tag;
    // the commit message for doc generation
    let doc_commit_msg = release_config.doc_commit_message.as_str();
    // flag to skip the confirmation step
    let no_confirm = args.no_confirm;
    // the publish flag in cargo file
    let publish = cargo_file
        .get("package")
        .and_then(|f| f.as_table())
        .and_then(|f| f.get("publish"))
        .and_then(|f| f.as_bool())
        .unwrap_or(!skip_publish);
    let metadata = args.metadata.as_ref();
    // feature list to release
    let feature_list = {
        if ! args.features.is_empty() {
            Some(args.features.clone())
        } else if ! release_config.enable_features.is_empty() {
            Some(release_config.enable_features.clone())
        } else {
            None
        }
    };
    // flag to release all features
    let all_features = args.all_features || release_config.enable_all_features;

    let features = if all_features {
        Features::All
    } else {
        match feature_list {
            Some(vec) => Features::Selective(vec),
            None => Features::None,
        }
    };

    // STEP -1: Check if git is available
    git::git_version()?;

    // STEP 0: Check if working directory is clean
    if !git::status(cwd)? {
        shell::log_warn("Uncommitted changes detected, please commit before release.");
        if !dry_run {
            return Ok(101);
        }
    }

    // STEP 1: Query a bunch of information for later use.
    let mut version = pkg_meta.version.clone();
    let prev_version_string = version.to_string();

    let crate_name = pkg_meta.name.as_str();

    let mut replacements = Replacements::new();
    replacements.insert("{{prev_version}}", prev_version_string.to_string());
    replacements.insert("{{version}}", version.to_string());
    replacements.insert("{{crate_name}}", crate_name.to_string());
    replacements.insert("{{date}}", Local::now().format("%Y-%m-%d").to_string());

    // STEP 2: update current version, save and commit
    if version::bump_version(&mut version, level, metadata)? {
        let new_version_string = version.to_string();
        replacements.insert("{{version}}", new_version_string.clone());
        // Release Confirmation
        if !dry_run {
            if !no_confirm {
                if !shell::confirm(&format!("Release version {} ?", new_version_string)) {
                    return Ok(0);
                }
            }
        }

        shell::log_info(&format!(
            "Update to version {} and commit",
            new_version_string
        ));
        if !dry_run {
            cargo::set_package_version(&manifest_path, &new_version_string)?;
            cargo::update_lock(&manifest_path)?;
        }
        for (pkg, dep) in find_dependents(&ws_meta, &pkg_meta) {
            if ! dep.req.matches(&version) {
                shell::log_warn(&format!("{}'s dependency on {} is now incompatible (currently {})", pkg.name, pkg_meta.name, dep.req));
            }
        }

        if ! pre_release_replacements.is_empty() {
            // try replacing text in configured files
            do_file_replacements(pre_release_replacements, &replacements, cwd, dry_run)?;
        }

        // pre-release hook
        if let Some(pre_rel_hook) = pre_release_hook {
            shell::log_info(&format!("Calling pre-release hook: {:?}", pre_rel_hook));
            let envs = btreemap! {
                "PREV_VERSION" => prev_version_string.as_ref(),
                "NEW_VERSION" => new_version_string.as_ref(),
                "DRY_RUN" => if dry_run { "true" } else { "false" }
            };
            // we use dry_run environmental variable to run the script
            // so here we set dry_run=false and always execute the command.
            if !cmd::call_with_env(pre_rel_hook, envs, cwd, false)? {
                shell::log_warn("Release aborted by non-zero return of prerelease hook.");
                return Ok(107);
            }
        }

        let commit_msg = replace_in(&pre_release_commit_msg, &replacements);
        if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
            // commit failed, abort release
            return Ok(102);
        }
    }

    // STEP 3: cargo publish
    if publish {
        shell::log_info("Running cargo publish");
        if !cargo::publish(dry_run, &manifest_path, features)? {
            return Ok(103);
        }
    }

    // STEP 4: upload doc
    if upload_doc {
        shell::log_info("Building and exporting docs.");
        cargo::doc(dry_run, &manifest_path)?;

        let doc_path = ws_meta.target_directory.join("doc");

        shell::log_info("Commit and push docs.");
        git::init(&doc_path, dry_run)?;
        git::add_all(&doc_path, dry_run)?;
        git::commit_all(&doc_path, doc_commit_msg, sign, dry_run)?;
        let default_remote = git::origin_url(cwd)?;

        let mut refspec = String::from("master:");
        refspec.push_str(&doc_branch);

        git::force_push(&doc_path, default_remote.trim(), &refspec, dry_run)?;
    }

    // STEP 5: Tag
    let root = git::top_level(cwd)?;
    let is_root = root == cwd;
    let tag_prefix = args
        .tag_prefix
        .as_ref()
        .map(|s| s.as_str())
        .or_else(|| release_config.tag_prefix.as_ref().map(|s| s.as_str()))
        .unwrap_or_else(|| {
            // crate_name as default tag prefix for multi-crate project
            if !is_root {
                "{{crate_name}}-"
            } else {
                ""
            }
        });
    let tag_prefix = replace_in(&tag_prefix, &replacements);

    replacements.insert("{{prefix}}", tag_prefix.clone());

    let current_version = version.to_string();
    let tag_name = format!("{}{}", tag_prefix, current_version);

    if !skip_tag {
        let tag_message = replace_in(tag_msg, &replacements);

        shell::log_info(&format!("Creating git tag {}", tag_name));
        if !git::tag(cwd, &tag_name, &tag_message, sign, dry_run)? {
            // tag failed, abort release
            return Ok(104);
        }
    }

    // STEP 6: bump version
    if !version::is_pre_release(level) && !no_dev_version {
        version.increment_patch();
        version
            .pre
            .push(Identifier::AlphaNumeric(dev_version_ext.to_owned()));
        shell::log_info(&format!("Starting next development iteration {}", version));
        let updated_version_string = version.to_string();
        replacements.insert("{{next_version}}", updated_version_string.clone());
        if !dry_run {
            cargo::set_package_version(&manifest_path, &updated_version_string)?;
            cargo::update_lock(&manifest_path)?;
        }
        let commit_msg = replace_in(&pro_release_commit_msg, &replacements);

        if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
            return Ok(105);
        }
    }

    // STEP 7: git push
    if !skip_push {
        shell::log_info("Pushing to git remote");
        if !git::push(cwd, &git_remote, dry_run)? {
            return Ok(106);
        }
        if !skip_tag && !git::push_tag(cwd, &git_remote, &tag_name, dry_run)? {
            return Ok(106);
        }
    }

    shell::log_info("Finished");
    Ok(0)
}

/// Expresses what features flags should be used
pub enum Features {
    /// None - don't use special features
    None,
    /// Only use selected features
    Selective(Vec<String>),
    /// Use all features via `all-features`
    All,
}

#[derive(Debug, StructOpt)]
struct ReleaseOpt {
    /// Release level:  bumping major|minor|patch|rc|beta|alpha version on release or removing prerelease extensions by default
    level: Option<String>,

    #[structopt(short = "c", long = "config")]
    /// Custom config file
    config: Option<String>,

    #[structopt(short = "m")]
    /// Semver metadata
    metadata: Option<String>,

    #[structopt(long = "sign")]
    /// Sign git commit and tag
    sign: bool,

    #[structopt(long = "dry-run")]
    /// Do not actually change anything, just log what are going to do
    dry_run: bool,

    #[structopt(long = "upload-doc")]
    /// Upload rust document to gh-pages branch
    upload_doc: bool,

    #[structopt(long = "push-remote")]
    /// Git remote to push
    push_remote: Option<String>,

    #[structopt(long = "skip-publish")]
    /// Do not run cargo publish on release
    skip_publish: bool,

    #[structopt(long = "skip-push")]
    /// Do not run git push in the last step
    skip_push: bool,

    #[structopt(long = "skip-tag")]
    /// Do not create git tag
    skip_tag: bool,

    #[structopt(long = "doc-branch")]
    /// Git branch to push documentation on
    doc_branch: Option<String>,

    #[structopt(long = "tag-prefix")]
    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    tag_prefix: Option<String>,

    #[structopt(long = "dev-version-ext")]
    /// Pre-release identifier(s) to append to the next development version after release
    dev_version_ext: Option<String>,

    #[structopt(long = "no-dev-version")]
    /// Do not create dev version after release
    no_dev_version: bool,

    #[structopt(long = "no-confirm")]
    /// Skip release confirmation and version preview
    no_confirm: bool,

    #[structopt(long = "features")]
    /// Provide a set of features that need to be enabled
    features: Vec<String>,

    #[structopt(long = "all-features")]
    /// Enable all features via `all-features`. Overrides `features`
    all_features: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo")]
enum Command {
    #[structopt(name = "release")]
    Release(ReleaseOpt),
}

fn main() {
    let Command::Release(ref release_matches) = Command::from_args();
    match execute(release_matches) {
        Ok(code) => exit(code),
        Err(e) => {
            shell::log_warn(&format!("Fatal: {}", e));
            exit(128);
        }
    }
}
