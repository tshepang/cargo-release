use clap::Parser;

use crate::util::resolve_bool_arg;

#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum Command {
    #[command(name = "release")]
    #[command(about, author, version)]
    Release(ReleaseOpt),
}

#[derive(Debug, Clone, clap::Args)]
#[command(help_template = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

Arguments:
{positionals}

Options:
{options}

Steps:
{subcommands}{after-help}
")]
#[command(subcommand_value_name = "STEP")]
#[command(subcommand_help_heading = "Steps")]
pub struct ReleaseOpt {
    #[command(flatten)]
    pub manifest: clap_cargo::Manifest,

    #[command(flatten)]
    pub workspace: clap_cargo::Workspace,

    /// Release level or version: bumping specified version field or remove prerelease extensions by default. Possible level value: major, minor, patch, release, rc, beta, alpha or any valid semver version that is greater than current version
    #[arg(default_value_t)]
    pub level_or_version: crate::version::TargetVersion,

    /// Semver metadata
    #[arg(short, long)]
    pub metadata: Option<String>,

    /// Custom config file
    #[arg(short, long = "config")]
    pub custom_config: Option<String>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    pub isolated: bool,

    #[command(flatten)]
    pub config: ConfigArgs,

    /// Token to use when uploading
    #[arg(long)]
    pub token: Option<String>,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    pub execute: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    pub no_confirm: bool,

    /// The name of tag for the previous release.
    #[arg(long)]
    pub prev_tag_name: Option<String>,

    /// Write the current configuration to file with `-` for stdout
    #[arg(long)]
    pub dump_config: Option<std::path::PathBuf>,

    #[command(flatten)]
    pub logging: Verbosity,
}

impl ReleaseOpt {
    pub fn dry_run(&self) -> bool {
        !self.execute
    }
}

#[derive(Debug, Clone, clap::Args)]
pub struct ConfigArgs {
    /// Sign both git commit and tag
    #[arg(long, overrides_with("no_sign"))]
    sign: bool,
    #[arg(long, overrides_with("sign"), hide(true))]
    no_sign: bool,

    /// Sign git commit
    #[arg(long, overrides_with("no_sign_commit"))]
    sign_commit: bool,
    #[arg(long, overrides_with("sign_commit"), hide(true))]
    no_sign_commit: bool,

    /// Specify how workspace dependencies on this crate should be handed.
    #[arg(long, value_enum)]
    dependent_version: Option<crate::config::DependentVersion>,

    /// Pre-release identifier(s) to append to the next development version after release
    #[arg(long)]
    dev_version_ext: Option<String>,

    /// Create dev version after release
    #[arg(long, overrides_with("no_dev_version"))]
    dev_version: bool,
    #[arg(long, overrides_with("dev_version"), hide(true))]
    no_dev_version: bool,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,

    #[command(flatten)]
    publish: crate::publish::PublishArgs,

    #[command(flatten)]
    tag: crate::tag::TagArgs,

    #[command(flatten)]
    push: crate::push::PushArgs,
}

impl ConfigArgs {
    pub fn to_config(&self) -> crate::config::Config {
        let mut config = crate::config::Config {
            allow_branch: self.allow_branch.clone(),
            sign_commit: resolve_bool_arg(self.sign_commit, self.no_sign_commit)
                .or_else(|| self.sign()),
            sign_tag: self.sign(),
            dev_version_ext: self.dev_version_ext.clone(),
            dev_version: resolve_bool_arg(self.dev_version, self.no_dev_version),
            dependent_version: self.dependent_version,
            ..Default::default()
        };
        config.update(&self.publish.to_config());
        config.update(&self.tag.to_config());
        config.update(&self.push.to_config());
        config
    }

    fn sign(&self) -> Option<bool> {
        resolve_bool_arg(self.sign, self.no_sign)
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct Verbosity {
    /// Pass many times for less log output
    #[arg(long, short, action = clap::ArgAction::Count)]
    quiet: u8,

    /// Pass many times for more log output
    ///
    /// By default, it'll report info. Passing `-v` one time adds debug
    /// logs, `-vv` adds trace logs.
    #[arg(long, short, action = clap::ArgAction::Count)]
    verbose: u8,
}

impl Verbosity {
    /// Get the log level.
    pub fn log_level(&self) -> log::Level {
        let verbosity = 2 - (self.quiet as i8) + (self.verbose as i8);

        match verbosity {
            i8::MIN..=0 => log::Level::Error,
            1 => log::Level::Warn,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            4..=i8::MAX => log::Level::Trace,
        }
    }
}

#[test]
fn verify_app() {
    use clap::CommandFactory;
    Command::command().debug_assert()
}
