use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo")]
#[structopt(
    setting = structopt::clap::AppSettings::UnifiedHelpMessage,
    setting = structopt::clap::AppSettings::DeriveDisplayOrder,
    setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
)]
pub enum Command {
    #[structopt(name = "release")]
    #[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
    Release(ReleaseOpt),
}

#[derive(Debug, StructOpt)]
pub struct ReleaseOpt {
    #[structopt(flatten)]
    pub manifest: clap_cargo::Manifest,

    #[structopt(flatten)]
    pub workspace: clap_cargo::Workspace,

    /// Release level or version: bumping specified version field or remove prerelease extensions by default. Possible level value: major, minor, patch, release, rc, beta, alpha or any valid semver version that is greater than current version
    #[structopt(default_value)]
    pub level_or_version: crate::version::TargetVersion,

    #[structopt(short = "m")]
    /// Semver metadata
    pub metadata: Option<String>,

    #[structopt(short = "c", long = "config")]
    /// Custom config file
    pub custom_config: Option<String>,

    #[structopt(long)]
    /// Ignore implicit configuration files.
    pub isolated: bool,

    #[structopt(flatten)]
    pub config: ConfigArgs,

    /// Actually perform a release. Dry-run mode is the default
    #[structopt(short = "x", long)]
    pub execute: bool,

    #[structopt(long)]
    /// Skip release confirmation and version preview
    pub no_confirm: bool,

    #[structopt(long)]
    /// The name of tag for the previous release.
    pub prev_tag_name: Option<String>,

    #[structopt(flatten)]
    pub logging: Verbosity,
}

impl ReleaseOpt {
    pub fn dry_run(&self) -> bool {
        !self.execute
    }
}

#[derive(Debug, StructOpt)]
pub struct ConfigArgs {
    #[structopt(long)]
    /// Sign both git commit and tag,
    pub sign: bool,

    #[structopt(long)]
    /// Sign git commit
    pub sign_commit: bool,

    #[structopt(long)]
    /// Sign git tag
    pub sign_tag: bool,

    #[structopt(long)]
    /// Git remote to push
    pub push_remote: Option<String>,

    #[structopt(long)]
    /// Cargo registry to upload to
    pub registry: Option<String>,

    #[structopt(long)]
    /// Do not run cargo publish on release
    pub skip_publish: bool,

    #[structopt(long)]
    /// Do not run git push in the last step
    pub skip_push: bool,

    #[structopt(long)]
    /// Do not create git tag
    pub skip_tag: bool,

    #[structopt(long)]
    /// Don't verify the contents by building them
    pub no_verify: bool,

    #[structopt(
        long,
        possible_values(&crate::config::DependentVersion::variants()),
        case_insensitive(true),
    )]
    /// Specify how workspace dependencies on this crate should be handed.
    pub dependent_version: Option<crate::config::DependentVersion>,

    #[structopt(long)]
    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    pub tag_prefix: Option<String>,

    #[structopt(long)]
    /// The name of the git tag.
    pub tag_name: Option<String>,

    #[structopt(long)]
    /// Pre-release identifier(s) to append to the next development version after release
    pub dev_version_ext: Option<String>,

    #[structopt(long)]
    /// Do not create dev version after release
    pub no_dev_version: bool,

    #[structopt(long)]
    /// Provide a set of features that need to be enabled
    pub features: Vec<String>,

    #[structopt(long)]
    /// Enable all features via `all-features`. Overrides `features`
    pub all_features: bool,

    #[structopt(long)]
    /// Token to use when uploading
    pub token: Option<String>,
}

impl ConfigArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            sign_commit: self
                .sign
                .then(|| true)
                .or_else(|| self.sign_commit.then(|| true)),
            sign_tag: self
                .sign
                .then(|| true)
                .or_else(|| self.sign_tag.then(|| true)),
            push_remote: self.push_remote.clone(),
            registry: self.registry.clone(),
            disable_publish: self.skip_publish.then(|| true),
            no_verify: self.no_verify.then(|| true),
            disable_push: self.skip_push.then(|| true),
            dev_version_ext: self.dev_version_ext.clone(),
            no_dev_version: self.no_dev_version.then(|| true),
            tag_prefix: self.tag_prefix.clone(),
            tag_name: self.tag_name.clone(),
            disable_tag: self.skip_tag.then(|| true),
            enable_features: (!self.features.is_empty()).then(|| self.features.clone()),
            enable_all_features: self.all_features.then(|| true),
            dependent_version: self.dependent_version,
            ..Default::default()
        }
    }
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
