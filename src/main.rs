#[macro_use]
extern crate clap;
#[macro_use]
extern crate maplit;
#[macro_use]
extern crate quick_error;

use structopt;

#[cfg(test)]
extern crate assert_fs;

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;
use std::process::exit;

use boolinator::Boolinator;
use chrono::prelude::Local;
use semver::Identifier;
use structopt::StructOpt;

use crate::error::FatalError;
use crate::replace::{do_file_replacements, Template};

mod cargo;
mod cmd;
mod config;
mod error;
mod git;
mod replace;
mod shell;
mod version;

fn find_dependents<'w>(
    ws_meta: &'w cargo_metadata::Metadata,
    pkg_meta: &'w cargo_metadata::Package,
) -> impl Iterator<Item = (&'w cargo_metadata::Package, &'w cargo_metadata::Dependency)> {
    ws_meta.packages.iter().filter_map(move |p| {
        if ws_meta.workspace_members.iter().any(|m| *m == p.id) {
            p.dependencies
                .iter()
                .find(|d| d.name == pkg_meta.name)
                .map(|d| (p, d))
        } else {
            None
        }
    })
}

struct PackageRelease<'m> {
    meta: &'m cargo_metadata::Package,
    manifest_path: &'m Path,
    package_path: &'m Path,
    config: config::Config,

    prev_version: Version,
    prev_tag: String,
    version: Option<Version>,
    tag: Option<String>,
    post_version: Option<Version>,

    //dependent_version: config::DependentVersion,
    //dependents: Vec<&'m Path>,
    //failed_dependents: Vec<&'m Path>,
    features: Features,
}

struct Version {
    version: semver::Version,
    version_string: String,
}

impl<'m> PackageRelease<'m> {
    fn load(
        args: &ReleaseOpt,
        git_root: &Path,
        ws_meta: &'m cargo_metadata::Metadata,
        pkg_meta: &'m cargo_metadata::Package,
    ) -> Result<Self, error::FatalError> {
        let manifest_path = pkg_meta.manifest_path.as_path();
        let cwd = manifest_path.parent().unwrap_or_else(|| Path::new("."));

        let config = {
            let mut release_config = config::Config::default();

            if !args.isolated {
                let cfg = config::resolve_config(&ws_meta.workspace_root, &manifest_path)?;
                release_config.update(&cfg);
            }

            if let Some(custom_config_path) = args.custom_config.as_ref() {
                // when calling with -c option
                let cfg = config::resolve_custom_config(Path::new(custom_config_path))?
                    .unwrap_or_default();
                release_config.update(&cfg);
            }

            release_config.update(&args.config);

            // the publish flag in cargo file
            let cargo_file = cargo::parse_cargo_config(&manifest_path)?;
            if !cargo_file
                .get("package")
                .and_then(|f| f.as_table())
                .and_then(|f| f.get("publish"))
                .and_then(|f| f.as_bool())
                .unwrap_or(true)
            {
                release_config.disable_publish = Some(true);
            }

            release_config
        };

        let is_root = git_root == cwd;

        let prev_version = Version {
            version: pkg_meta.version.clone(),
            version_string: pkg_meta.version.to_string(),
        };

        let prev_tag = {
            let mut template = Template {
                prev_version: Some(&prev_version.version_string),
                version: Some(&prev_version.version_string),
                crate_name: Some(pkg_meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = config.tag_prefix(is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            template.render(config.tag_name())
        };

        let version = {
            let mut potential_version = prev_version.version.clone();
            if args
                .level
                .bump_version(&mut potential_version, args.metadata.as_ref())?
            {
                let version = potential_version;
                let version_string = version.to_string();
                Some(Version {
                    version,
                    version_string,
                })
            } else {
                None
            }
        };

        let base = version.as_ref().unwrap_or_else(|| &prev_version);

        let tag = if config.disable_tag() {
            None
        } else {
            let mut template = Template {
                prev_version: Some(&prev_version.version_string),
                version: Some(&base.version_string),
                crate_name: Some(pkg_meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = config.tag_prefix(is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            Some(template.render(config.tag_name()))
        };

        let post_version = if !args.level.is_pre_release() && !config.no_dev_version() {
            let mut post = base.version.clone();
            post.increment_patch();
            post.pre.push(Identifier::AlphaNumeric(
                config.dev_version_ext().to_owned(),
            ));
            let post_string = post.to_string();

            Some(Version {
                version: post,
                version_string: post_string,
            })
        } else {
            None
        };

        let features = if config.enable_all_features() {
            Features::All
        } else {
            let features = config.enable_features();
            if features.is_empty() {
                Features::None
            } else {
                Features::Selective(features.to_owned())
            }
        };

        let pkg = PackageRelease {
            meta: pkg_meta,
            manifest_path,
            package_path: cwd,
            config,

            prev_version,
            prev_tag,
            version,
            tag,
            post_version,

            features: features,
        };
        Ok(pkg)
    }
}

fn release_workspace(args: &ReleaseOpt) -> Result<i32, error::FatalError> {
    let ws_meta = args.manifest.metadata().exec().map_err(FatalError::from)?;

    git::git_version()?;
    if git::is_dirty(&ws_meta.workspace_root)? {
        log::warn!("Uncommitted changes detected, please commit before release.");
        if !args.dry_run {
            return Ok(101);
        }
    }

    let pkg_ids = sort_workspace(&ws_meta);

    let (selected_pkgs, _excluded_pkgs) = args.workspace.partition_packages(&ws_meta);
    if selected_pkgs.is_empty() {
        log::info!("No packages selected.");
        return Ok(0);
    }

    let root = git::top_level(&ws_meta.workspace_root)?;
    let pkgs: Result<HashMap<_, _>, _> = selected_pkgs
        .iter()
        .map(|p| PackageRelease::load(args, &root, &ws_meta, p).map(|p| (&p.meta.id, p)))
        .collect();
    let pkgs = pkgs?;

    release_packages(args, &ws_meta, pkg_ids.iter().filter_map(|id| pkgs.get(id)))
}

fn sort_workspace<'m>(ws_meta: &'m cargo_metadata::Metadata) -> Vec<&'m cargo_metadata::PackageId> {
    let members: HashSet<_> = ws_meta.workspace_members.iter().collect();
    let dep_tree: HashMap<_, _> = ws_meta
        .resolve
        .as_ref()
        .expect("cargo-metadata resolved deps")
        .nodes
        .iter()
        .filter_map(|n| {
            if members.contains(&n.id) {
                Some((&n.id, &n.dependencies))
            } else {
                None
            }
        })
        .collect();

    let mut sorted = Vec::new();
    let mut processed = HashSet::new();
    for pkg_id in ws_meta.workspace_members.iter() {
        sort_workspace_inner(&ws_meta, pkg_id, &dep_tree, &mut processed, &mut sorted);
    }

    sorted
}

fn sort_workspace_inner<'m>(
    ws_meta: &'m cargo_metadata::Metadata,
    pkg_id: &'m cargo_metadata::PackageId,
    dep_tree: &HashMap<&'m cargo_metadata::PackageId, &'m std::vec::Vec<cargo_metadata::PackageId>>,
    processed: &mut HashSet<&'m cargo_metadata::PackageId>,
    sorted: &mut Vec<&'m cargo_metadata::PackageId>,
) {
    if !processed.insert(pkg_id) {
        return;
    }

    for dep_id in dep_tree[pkg_id]
        .iter()
        .filter(|dep_id| dep_tree.contains_key(dep_id))
    {
        sort_workspace_inner(ws_meta, dep_id, dep_tree, processed, sorted);
    }

    sorted.push(pkg_id);
}

fn release_packages<'m>(
    args: &ReleaseOpt,
    ws_meta: &cargo_metadata::Metadata,
    pkgs: impl Iterator<Item = &'m PackageRelease<'m>> + Clone,
) -> Result<i32, error::FatalError> {
    let dry_run = args.dry_run;

    for pkg in pkgs.clone() {
        let code = release_package(args, &ws_meta, pkg)?;
        if code != 0 {
            return Ok(code);
        }
    }

    // STEP 7: git push
    let mut pushed: HashSet<_> = HashSet::new();
    for pkg in pkgs {
        if !pkg.config.disable_push() {
            let git_remote = pkg.config.push_remote();
            if !pushed.contains(git_remote) {
                log::info!("Pushing HEAD to {}", git_remote);
                if !git::push(&ws_meta.workspace_root, git_remote, dry_run)? {
                    return Ok(106);
                }
                pushed.insert(git_remote);
            }
            if let Some(tag_name) = pkg.tag.as_ref() {
                log::info!("Pushing {} to {}", tag_name, git_remote);
                if !git::push_tag(&ws_meta.workspace_root, git_remote, &tag_name, dry_run)? {
                    return Ok(106);
                }
            }
        }
    }

    Ok(0)
}

fn release_package(
    args: &ReleaseOpt,
    ws_meta: &cargo_metadata::Metadata,
    pkg: &PackageRelease<'_>,
) -> Result<i32, error::FatalError> {
    // STEP 1: Query a bunch of information for later use.
    let dry_run = args.dry_run;
    let sign = pkg.config.sign_commit();
    let cwd = pkg.package_path;
    let crate_name = pkg.meta.name.as_str();
    let now = Local::now().format("%Y-%m-%d").to_string();

    // STEP 2: update current version, save and commit
    if let Some(version) = pkg.version.as_ref() {
        let prev_tag_name = &pkg.prev_tag;
        if let Some(changed) = git::changed_from(cwd, &prev_tag_name)? {
            if !changed {
                log::warn!(
                    "Releasing {} despite no changes made since tag {}",
                    crate_name,
                    prev_tag_name
                );
            }
        } else {
            log::info!(
                "Cannot detect changes for {} because tag {} is missing",
                crate_name,
                prev_tag_name
            );
        }

        let new_version_string = version.version_string.as_str();
        // Release Confirmation
        if !dry_run && !args.no_confirm {
            let confirmed = shell::confirm(&format!(
                "Release version {} {}?",
                crate_name, new_version_string
            ));
            if !confirmed {
                return Ok(0);
            }
        }

        log::info!("Update {} to version {}", crate_name, new_version_string);
        if !dry_run {
            cargo::set_package_version(&pkg.manifest_path, &new_version_string)?;
        }
        let mut dependents_failed = false;
        for (dep_pkg, dep) in find_dependents(&ws_meta, &pkg.meta) {
            match pkg.config.dependent_version() {
                config::DependentVersion::Ignore => (),
                config::DependentVersion::Warn => {
                    if !dep.req.matches(&version.version) {
                        log::warn!(
                            "{}'s dependency on {} `{}` is incompatible with {}",
                            dep_pkg.name,
                            pkg.meta.name,
                            dep.req,
                            new_version_string
                        );
                    }
                }
                config::DependentVersion::Error => {
                    if !dep.req.matches(&version.version) {
                        log::warn!(
                            "{}'s dependency on {} `{}` is incompatible with {}",
                            dep_pkg.name,
                            pkg.meta.name,
                            dep.req,
                            new_version_string
                        );
                        dependents_failed = true;
                    }
                }
                config::DependentVersion::Fix => {
                    if !dep.req.matches(&version.version) {
                        let new_req = version::set_requirement(&dep.req, &version.version)?;
                        if let Some(new_req) = new_req {
                            if dry_run {
                                log::info!(
                                    "Fixing {}'s dependency on {} to `{}` (from `{}`)",
                                    dep_pkg.name,
                                    pkg.meta.name,
                                    new_req,
                                    dep.req
                                );
                            } else {
                                cargo::set_dependency_version(
                                    &dep_pkg.manifest_path,
                                    &pkg.meta.name,
                                    &new_req,
                                )?;
                            }
                        }
                    }
                }
                config::DependentVersion::Upgrade => {
                    let new_req = version::set_requirement(&dep.req, &version.version)?;
                    if let Some(new_req) = new_req {
                        if dry_run {
                            log::info!(
                                "Upgrading {}'s dependency on {} to `{}` (from `{}`)",
                                dep_pkg.name,
                                pkg.meta.name,
                                new_req,
                                dep.req
                            );
                        } else {
                            cargo::set_dependency_version(
                                &dep_pkg.manifest_path,
                                &pkg.meta.name,
                                &new_req,
                            )?;
                        }
                    }
                }
            }
        }
        if dependents_failed {
            return Ok(110);
        }
        if dry_run {
            log::debug!("Updating lock file");
        } else {
            cargo::update_lock(&pkg.manifest_path)?;
        }

        if !pkg.config.pre_release_replacements().is_empty() {
            // try replacing text in configured files
            let template = Template {
                prev_version: Some(&pkg.prev_version.version_string),
                version: Some(&new_version_string),
                crate_name: Some(crate_name),
                date: Some(now.as_str()),
                tag_name: pkg.tag.as_ref().map(|s| s.as_str()),
                ..Default::default()
            };
            do_file_replacements(
                pkg.config.pre_release_replacements(),
                &template,
                cwd,
                dry_run,
            )?;
        }

        // pre-release hook
        if let Some(pre_rel_hook) = pkg.config.pre_release_hook() {
            let pre_rel_hook = pre_rel_hook.args();
            log::debug!("Calling pre-release hook: {:?}", pre_rel_hook);
            let envs = btreemap! {
                OsStr::new("PREV_VERSION") => pkg.prev_version.version_string.as_ref(),
                OsStr::new("NEW_VERSION") => new_version_string.as_ref(),
                OsStr::new("DRY_RUN") => OsStr::new(if dry_run { "true" } else { "false" }),
                OsStr::new("CRATE_NAME") => OsStr::new(crate_name),
                OsStr::new("WORKSPACE_ROOT") => ws_meta.workspace_root.as_os_str(),
                OsStr::new("CRATE_ROOT") => pkg.manifest_path.parent().unwrap_or_else(|| Path::new(".")).as_os_str(),
            };
            // we use dry_run environmental variable to run the script
            // so here we set dry_run=false and always execute the command.
            if !cmd::call_with_env(pre_rel_hook, envs, cwd, false)? {
                log::warn!(
                    "Release of {} aborted by non-zero return of prerelease hook.",
                    crate_name
                );
                return Ok(107);
            }
        }

        let template = Template {
            prev_version: Some(&pkg.prev_version.version_string),
            version: Some(&new_version_string),
            crate_name: Some(crate_name),
            date: Some(now.as_str()),
            ..Default::default()
        };
        let commit_msg = template.render(pkg.config.pre_release_commit_message());
        if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
            // commit failed, abort release
            return Ok(102);
        }
    }

    // STEP 3: cargo publish
    if !pkg.config.disable_publish() {
        log::info!("Running cargo publish on {}", crate_name);
        // feature list to release
        let features = &pkg.features;
        if !cargo::publish(dry_run, &pkg.manifest_path, features)? {
            return Ok(103);
        }
    }

    // STEP 4: upload doc
    if pkg.config.upload_doc() {
        log::info!("Building and exporting docs for {}", crate_name);
        log::warn!("Doc uploading support is deprecated, see docs.rs instead");
        cargo::doc(dry_run, &pkg.manifest_path)?;

        let doc_path = ws_meta.target_directory.join("doc");

        log::info!("Commit and push docs for {}", crate_name);
        git::init(&doc_path, dry_run)?;
        git::add_all(&doc_path, dry_run)?;
        git::commit_all(&doc_path, pkg.config.doc_commit_message(), sign, dry_run)?;
        let default_remote = git::origin_url(cwd)?;

        let refspec = format!("master:{}", pkg.config.doc_branch());
        git::force_push(&doc_path, default_remote.trim(), &refspec, dry_run)?;
    }

    // STEP 5: Tag
    if let Some(tag_name) = pkg.tag.as_ref() {
        let base = pkg.version.as_ref().unwrap_or_else(|| &pkg.prev_version);
        let template = Template {
            prev_version: Some(&pkg.prev_version.version_string),
            version: Some(&base.version_string),
            crate_name: Some(crate_name),
            tag_name: Some(&tag_name),
            date: Some(now.as_str()),
            ..Default::default()
        };
        let tag_message = template.render(pkg.config.tag_message());

        log::debug!("Creating git tag {}", tag_name);
        if !git::tag(cwd, &tag_name, &tag_message, sign, dry_run)? {
            // tag failed, abort release
            return Ok(104);
        }
    }

    // STEP 6: bump version
    if let Some(version) = pkg.post_version.as_ref() {
        let updated_version_string = version.version_string.as_ref();
        log::info!(
            "Starting {}'s next development iteration {}",
            crate_name,
            updated_version_string,
        );
        if !dry_run {
            cargo::set_package_version(&pkg.manifest_path, &updated_version_string)?;
            cargo::update_lock(&pkg.manifest_path)?;
        }
        let base = pkg.version.as_ref().unwrap_or_else(|| &pkg.prev_version);
        let template = Template {
            prev_version: Some(&pkg.prev_version.version_string),
            version: Some(&base.version_string),
            crate_name: Some(crate_name),
            date: Some(now.as_str()),
            next_version: Some(updated_version_string),
            ..Default::default()
        };
        let commit_msg = template.render(pkg.config.post_release_commit_message());

        if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
            return Ok(105);
        }
    }

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
    #[structopt(flatten)]
    manifest: clap_cargo::Manifest,

    #[structopt(flatten)]
    workspace: clap_cargo::Workspace,

    /// Release level: bumping specified version field or remove prerelease extensions by default
    #[structopt(
        possible_values(&version::BumpLevel::variants()),
        case_insensitive(true),
        default_value = "release"
    )]
    level: version::BumpLevel,

    #[structopt(short = "m")]
    /// Semver metadata
    metadata: Option<String>,

    #[structopt(short = "c", long = "config")]
    /// Custom config file
    custom_config: Option<String>,

    #[structopt(long)]
    /// Ignore implicit configuration files.
    isolated: bool,

    #[structopt(flatten)]
    config: ConfigArgs,

    #[structopt(short = "n", long)]
    /// Do not actually change anything, just log what are going to do
    dry_run: bool,

    #[structopt(long)]
    /// Skip release confirmation and version preview
    no_confirm: bool,

    #[structopt(flatten)]
    logging: Verbosity,
}

#[derive(StructOpt, Debug, Clone)]
pub struct Verbosity {
    /// Pass many times for less log output
    #[structopt(long, short = "q", parse(from_occurrences))]
    quiet: i8,

    /// Pass many times for more log output
    ///
    /// By default, it'll report info. Passing `-v` one time also prints
    /// warnings, `-vv` enables info logging, `-vvv` debug, and `-vvvv` trace.
    #[structopt(long, short = "v", parse(from_occurrences))]
    verbose: i8,
}

impl Verbosity {
    /// Get the log level.
    pub fn log_level(&self) -> log::Level {
        let verbosity = 2 - self.quiet + self.verbose;

        match verbosity {
            std::i8::MIN..=0 => log::Level::Error,
            1 => log::Level::Warn,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            4..=std::i8::MAX => log::Level::Trace,
        }
    }
}

#[derive(Debug, StructOpt)]
struct ConfigArgs {
    #[structopt(long)]
    /// Sign git commit and tag
    sign: bool,

    #[structopt(long, hidden(true))]
    /// Upload rust document to gh-pages branch
    upload_doc: bool,

    #[structopt(long)]
    /// Git remote to push
    push_remote: Option<String>,

    #[structopt(long)]
    /// Do not run cargo publish on release
    skip_publish: bool,

    #[structopt(long)]
    /// Do not run git push in the last step
    skip_push: bool,

    #[structopt(long)]
    /// Do not create git tag
    skip_tag: bool,

    #[structopt(
        long,
        possible_values(&config::DependentVersion::variants()),
        case_insensitive(true),
    )]
    /// Specify how workspace dependencies on this crate should be handed.
    dependent_version: Option<config::DependentVersion>,

    #[structopt(long, hidden(true))]
    /// Git branch to push documentation on
    doc_branch: Option<String>,

    #[structopt(long)]
    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    tag_prefix: Option<String>,

    #[structopt(long)]
    /// The name of the git tag.
    tag_name: Option<String>,

    #[structopt(long)]
    /// Pre-release identifier(s) to append to the next development version after release
    dev_version_ext: Option<String>,

    #[structopt(long)]
    /// Do not create dev version after release
    no_dev_version: bool,

    #[structopt(long)]
    /// Provide a set of features that need to be enabled
    features: Vec<String>,

    #[structopt(long)]
    /// Enable all features via `all-features`. Overrides `features`
    all_features: bool,
}

impl config::ConfigSource for ConfigArgs {
    fn sign_commit(&self) -> Option<bool> {
        self.sign.as_some(true)
    }

    fn upload_doc(&self) -> Option<bool> {
        self.upload_doc.as_some(true)
    }

    fn push_remote(&self) -> Option<&str> {
        self.push_remote.as_ref().map(|s| s.as_str())
    }

    fn doc_branch(&self) -> Option<&str> {
        self.doc_branch.as_ref().map(|s| s.as_str())
    }

    fn disable_publish(&self) -> Option<bool> {
        self.skip_publish.as_some(true)
    }

    fn disable_push(&self) -> Option<bool> {
        self.skip_push.as_some(true)
    }

    fn dev_version_ext(&self) -> Option<&str> {
        self.dev_version_ext.as_ref().map(|s| s.as_str())
    }

    fn no_dev_version(&self) -> Option<bool> {
        self.no_dev_version.as_some(true)
    }

    fn tag_prefix(&self) -> Option<&str> {
        self.tag_prefix.as_ref().map(|s| s.as_str())
    }

    fn tag_name(&self) -> Option<&str> {
        self.tag_name.as_ref().map(|s| s.as_str())
    }

    fn disable_tag(&self) -> Option<bool> {
        self.skip_tag.as_some(true)
    }

    fn enable_features(&self) -> Option<&[String]> {
        if !self.features.is_empty() {
            Some(self.features.as_slice())
        } else {
            None
        }
    }

    fn enable_all_features(&self) -> Option<bool> {
        self.all_features.as_some(true)
    }

    fn dependent_version(&self) -> Option<config::DependentVersion> {
        self.dependent_version
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo")]
#[structopt(
    setting = structopt::clap::AppSettings::UnifiedHelpMessage,
    setting = structopt::clap::AppSettings::DeriveDisplayOrder,
    setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
)]
enum Command {
    #[structopt(name = "release")]
    #[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
    Release(ReleaseOpt),
}

pub fn get_logging(level: log::Level) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();

    builder.filter(None, level.to_level_filter());

    builder
        .default_format_timestamp(false)
        .default_format_module_path(false);

    builder
}

fn main() {
    let Command::Release(ref release_matches) = Command::from_args();

    let mut builder = get_logging(release_matches.logging.log_level());
    builder.init();

    match release_workspace(release_matches) {
        Ok(code) => exit(code),
        Err(e) => {
            log::warn!("Fatal: {}", e);
            exit(128);
        }
    }
}
