#![allow(dead_code)]

extern crate ansi_term;
extern crate chrono;
#[macro_use]
extern crate maplit;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate structopt;
extern crate regex;
extern crate semver;
extern crate toml;

use std::io::{stdin, stdout, Write};
use std::path::Path;
use std::process::exit;

use ansi_term::Colour::{Green, Red};
use ansi_term::Style;
use semver::Identifier;
use structopt::StructOpt;
use toml::value::Table;
use toml::Value;

mod cargo;
mod cmd;
mod config;
mod error;
mod git;
mod replace;
mod version;

fn confirm(prompt: &str) -> bool {
    let mut input = String::new();

    print!(
        "{}",
        Style::new().bold().paint(format!("{} [y/N] ", prompt))
    );

    stdout().flush().unwrap();
    stdin().read_line(&mut input).expect("y/n required");

    input.trim().to_lowercase() == "y"
}

fn get_string_option(
    cli: &Option<String>,
    config_file: Option<&Table>,
    config_file_key: &str,
    default_value: &str,
) -> String {
    cli.clone()
        .or_else(|| {
            config::get_release_config(config_file, config_file_key)
                .and_then(|f| f.as_str())
                .map(|f| f.to_owned())
        })
        .unwrap_or(default_value.to_owned())
}

fn get_bool_option(cli: bool, config_file: Option<&Table>, config_file_key: &str) -> bool {
    cli || config::get_release_config(config_file, config_file_key)
        .and_then(|f| f.as_bool())
        .unwrap_or(false)
}

fn execute(args: &ReleaseOpt) -> Result<i32, error::FatalError> {
    let cargo_file = config::parse_cargo_config()?;
    let custom_config_path_option = args.config.as_ref();
    // FIXME:
    let release_config = if let Some(custom_config_path) = custom_config_path_option {
        // when calling with -c option
        config::get_release_config_table_from_file(Path::new(custom_config_path))?
    } else {
        config::resolve_release_config_table(&cargo_file)?
    };

    // step -1
    if let Some(ref release_config_table) = release_config {
        if let Some(invalid_keys) = config::verify_release_config(release_config_table) {
            for i in invalid_keys {
                println!(
                    "{}",
                    Red.bold()
                        .paint(format!("Unknown config key \"{}\" found", i))
                );
            }
            return Ok(109);
        }
    }

    let dry_run = args.dry_run;
    let level = args.level.as_ref();
    let sign = get_bool_option(args.sign, release_config.as_ref(), config::SIGN_COMMIT);
    let upload_doc = get_bool_option(args.upload_doc, release_config.as_ref(), config::UPLOAD_DOC);
    let git_remote = get_string_option(
        &args.push_remote,
        release_config.as_ref(),
        config::PUSH_REMOTE,
        "origin",
    );
    let doc_branch = get_string_option(
        &args.doc_branch,
        release_config.as_ref(),
        config::DOC_BRANCH,
        "gh-pages",
    );
    let skip_push = get_bool_option(
        args.skip_push,
        release_config.as_ref(),
        config::DISABLE_PUSH,
    );
    let dev_version_ext = get_string_option(
        &args.dev_version_ext,
        release_config.as_ref(),
        config::DEV_VERSION_EXT,
        "alpha.0",
    );
    let no_dev_version = get_bool_option(
        args.no_dev_version,
        release_config.as_ref(),
        config::NO_DEV_VERSION,
    );
    let pre_release_commit_msg =
        config::get_release_config(release_config.as_ref(), config::PRE_RELEASE_COMMIT_MESSAGE)
            .and_then(|f| f.as_str())
            .unwrap_or("(cargo-release) version {{version}}");
    let pro_release_commit_msg =
        config::get_release_config(release_config.as_ref(), config::PRO_RELEASE_COMMIT_MESSAGE)
            .and_then(|f| f.as_str())
            .unwrap_or("(cargo-release) start next development iteration {{version}}");
    let pre_release_replacements =
        config::get_release_config(release_config.as_ref(), config::PRE_RELEASE_REPLACEMENTS);
    let pre_release_hook = config::get_release_config(
        release_config.as_ref(),
        config::PRE_RELEASE_HOOK,
    ).and_then(|h| match h {
        &Value::String(ref s) => Some(vec![s.as_ref()]),
        &Value::Array(ref a) => Some(
            a.iter()
                .map(|v| v.as_str())
                .filter(|o| o.is_some())
                .map(|s| s.unwrap())
                .collect(),
        ),
        _ => None,
    });
    let tag_msg = config::get_release_config(release_config.as_ref(), config::TAG_MESSAGE)
        .and_then(|f| f.as_str())
        .unwrap_or("(cargo-release) {{prefix}} version {{version}}");
    let doc_commit_msg =
        config::get_release_config(release_config.as_ref(), config::DOC_COMMIT_MESSAGE)
            .and_then(|f| f.as_str())
            .unwrap_or("(cargo-release) generate docs");
    let no_confirm = args.no_confirm;
    let publish = cargo_file
        .get("package")
        .and_then(|f| f.as_table())
        .and_then(|f| f.get("publish"))
        .and_then(|f| f.as_bool())
        .unwrap_or(true);
    let metadata = args.metadata.as_ref();

    // STEP 0: Check if working directory is clean
    if !try!(git::status()) {
        println!(
            "{}",
            Red.bold()
                .paint("Uncommitted changes detected, please commit before release.",)
        );
        if !dry_run {
            return Ok(101);
        }
    }

    // STEP 1: Read version from Cargo.toml and remove
    let mut version = cargo_file
        .get("package")
        .and_then(|f| f.as_table())
        .and_then(|f| f.get("version"))
        .and_then(|f| f.as_str())
        .and_then(|f| config::parse_version(f).ok())
        .unwrap();
    let prev_version_string = version.to_string();

    // STEP 2: update current version, save and commit
    if try!(version::bump_version(&mut version, level, metadata)) {
        let new_version_string = version.to_string();
        // Release Confirmation
        if !dry_run {
            if !no_confirm {
                if !confirm(&format!("Release version {} ?", new_version_string)) {
                    return Ok(0);
                }
            }
        }

        // pre-release hook
        if let Some(pre_rel_hook) = pre_release_hook {
            println!(
                "{}",
                Green.paint(format!("Calling pre-release hook: {:?}", pre_rel_hook))
            );
            let envs = btreemap!{
                "PREV_VERSION" => prev_version_string.as_ref(),
                "NEW_VERSION" => new_version_string.as_ref(),
                "DRY_RUN" => if dry_run { "true" } else { "false" }
            };
            // we use dry_run environmental variable to run the script
            // so here we set dry_run=false and always execute the command.
            if !try!(cmd::call_with_env(pre_rel_hook, envs, false)) {
                println!(
                    "{}",
                    Red.paint("Release aborted by non-zero return of prerelease hook.")
                );
                return Ok(107);
            }
        }

        println!(
            "{}",
            Green.paint(format!(
                "Update to version {} and commit",
                new_version_string
            ))
        );
        if !dry_run {
            try!(config::rewrite_cargo_version(&new_version_string));
        }

        if let Some(pre_rel_rep) = pre_release_replacements {
            // try update version number in configured files
            try!(replace::do_replace_versions(
                pre_rel_rep,
                &new_version_string,
                dry_run,
            ));
        }

        let commit_msg =
            String::from(pre_release_commit_msg).replace("{{version}}", &new_version_string);
        if !try!(git::commit_all(".", &commit_msg, sign, dry_run)) {
            // commit failed, abort release
            return Ok(102);
        }
    }

    // STEP 3: cargo publish
    if publish {
        println!("{}", Green.paint("Running cargo publish"));
        if !try!(cargo::publish(dry_run)) {
            return Ok(103);
        }
    }

    // STEP 4: upload doc
    if upload_doc {
        println!("{}", Green.paint("Building and exporting docs."));
        try!(cargo::doc(dry_run));

        let doc_path = "target/doc/";

        println!("{}", Green.paint("Commit and push docs."));
        try!(git::init(doc_path, dry_run));
        try!(git::add_all(doc_path, dry_run));
        try!(git::commit_all(doc_path, doc_commit_msg, sign, dry_run));
        let default_remote = try!(git::origin_url());

        let mut refspec = String::from("master:");
        refspec.push_str(&doc_branch);

        try!(git::force_push(
            doc_path,
            default_remote.trim(),
            &refspec,
            dry_run,
        ));
    }

    // STEP 5: Tag
    let root = try!(git::top_level());
    let rel_path = try!(cmd::relative_path_for(&root));
    let tag_prefix = args.tag_prefix
        .clone()
        .or_else(|| {
            config::get_release_config(release_config.as_ref(), config::TAG_PREFIX)
                .and_then(|f| f.as_str())
                .map(|f| f.to_string())
        })
        .or_else(|| rel_path.as_ref().map(|t| format!("{}-", t)));

    let current_version = version.to_string();
    let tag_name = tag_prefix.as_ref().map_or_else(
        || current_version.clone(),
        |x| format!("{}{}", x, current_version),
    );

    let tag_message = String::from(tag_msg)
        .replace("{{prefix}}", tag_prefix.as_ref().unwrap_or(&"".to_owned()))
        .replace("{{version}}", &current_version);

    println!("{}", Green.paint(format!("Creating git tag {}", tag_name)));
    if !try!(git::tag(&tag_name, &tag_message, sign, dry_run)) {
        // tag failed, abort release
        return Ok(104);
    }

    // STEP 6: bump version
    if !version::is_pre_release(level) && !no_dev_version {
        version.increment_patch();
        version
            .pre
            .push(Identifier::AlphaNumeric(dev_version_ext.to_owned()));
        println!(
            "{}",
            Green.paint(format!("Starting next development iteration {}", version))
        );
        let updated_version_string = version.to_string();
        if !dry_run {
            try!(config::rewrite_cargo_version(&updated_version_string));
        }
        let commit_msg =
            String::from(pro_release_commit_msg).replace("{{version}}", &updated_version_string);

        if !try!(git::commit_all(".", &commit_msg, sign, dry_run)) {
            return Ok(105);
        }
    }

    // STEP 7: git push
    if !skip_push {
        println!("{}", Green.paint("Pushing to git remote"));
        if !try!(git::push(&git_remote, dry_run)) {
            return Ok(106);
        }
        if !try!(git::push_tag(&git_remote, &tag_name, dry_run)) {
            return Ok(106);
        }
    }

    println!("{}", Green.paint("Finished"));
    Ok(0)
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

    #[structopt(long = "skip-push")]
    /// Do not run git push in the last step
    skip_push: bool,

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
            println!("{}", Red.bold().paint(format!("Fatal: {}", e)));
            exit(128);
        }
    }
}
