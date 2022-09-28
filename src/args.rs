use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum Command {
    #[command(name = "release")]
    #[command(about, author, version)]
    Release(ReleaseOpt),
}

#[derive(Debug, Clone, clap::Args)]
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

    /// Sign git tag
    #[arg(long, overrides_with("no_sign_tag"))]
    sign_tag: bool,
    #[arg(long, overrides_with("sign_tag"), hide(true))]
    no_sign_tag: bool,

    /// Git remote to push
    #[arg(long)]
    push_remote: Option<String>,

    /// Cargo registry to upload to
    #[arg(long)]
    registry: Option<String>,

    #[arg(long, overrides_with("no_publish"), hide(true))]
    publish: bool,
    /// Do not run cargo publish on release
    #[arg(long, overrides_with("publish"))]
    no_publish: bool,

    #[arg(long, overrides_with("no_push"), hide(true))]
    push: bool,
    /// Do not run git push in the last step
    #[arg(long, overrides_with("push"))]
    no_push: bool,

    #[arg(long, overrides_with("no_tag"), hide(true))]
    tag: bool,
    /// Do not create git tag
    #[arg(long, overrides_with("tag"))]
    no_tag: bool,

    #[arg(long, overrides_with("no_verify"), hide(true))]
    verify: bool,
    /// Don't verify the contents by building them
    #[arg(long, overrides_with("verify"))]
    no_verify: bool,

    /// Specify how workspace dependencies on this crate should be handed.
    #[arg(long, value_enum)]
    dependent_version: Option<crate::config::DependentVersion>,

    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    #[arg(long)]
    tag_prefix: Option<String>,

    /// The name of the git tag.
    #[arg(long)]
    tag_name: Option<String>,

    /// Pre-release identifier(s) to append to the next development version after release
    #[arg(long)]
    dev_version_ext: Option<String>,

    /// Create dev version after release
    #[arg(long, overrides_with("no_dev_version"))]
    dev_version: bool,
    #[arg(long, overrides_with("dev_version"), hide(true))]
    no_dev_version: bool,

    /// Provide a set of features that need to be enabled
    #[arg(long)]
    features: Vec<String>,

    /// Enable all features via `all-features`. Overrides `features`
    #[arg(long)]
    all_features: bool,

    /// Build for the target triple
    #[arg(long)]
    target: Option<String>,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,
}

impl ConfigArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            allow_branch: self.allow_branch.clone(),
            sign_commit: resolve_bool_arg(self.sign_commit, self.no_sign_commit)
                .or_else(|| self.sign()),
            sign_tag: resolve_bool_arg(self.sign_tag, self.no_sign_tag).or_else(|| self.sign()),
            push_remote: self.push_remote.clone(),
            registry: self.registry.clone(),
            publish: resolve_bool_arg(self.publish, self.no_publish),
            verify: resolve_bool_arg(self.verify, self.no_verify),
            push: resolve_bool_arg(self.push, self.no_push),
            dev_version_ext: self.dev_version_ext.clone(),
            dev_version: resolve_bool_arg(self.dev_version, self.no_dev_version),
            tag_prefix: self.tag_prefix.clone(),
            tag_name: self.tag_name.clone(),
            tag: resolve_bool_arg(self.tag, self.no_tag),
            enable_features: (!self.features.is_empty()).then(|| self.features.clone()),
            enable_all_features: self.all_features.then(|| true),
            dependent_version: self.dependent_version,
            target: self.target.clone(),
            ..Default::default()
        }
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

fn resolve_bool_arg(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (_, _) => unreachable!("clap should make this impossible"),
    }
}

#[test]
fn verify_app() {
    use clap::CommandFactory;
    Command::command().debug_assert()
}
