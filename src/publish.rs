use crate::util::resolve_bool_arg;

#[derive(Clone, Default, Debug, clap::Args)]
#[command(next_help_heading = "Publish")]
pub struct PublishArgs {
    #[arg(long, overrides_with("no_publish"), hide(true))]
    publish: bool,
    /// Do not run cargo publish on release
    #[arg(long, overrides_with("publish"))]
    no_publish: bool,

    /// Cargo registry to upload to
    #[arg(long)]
    registry: Option<String>,

    #[arg(long, overrides_with("no_verify"), hide(true))]
    verify: bool,
    /// Don't verify the contents by building them
    #[arg(long, overrides_with("verify"))]
    no_verify: bool,

    /// Provide a set of features that need to be enabled
    #[arg(long)]
    features: Vec<String>,

    /// Enable all features via `all-features`. Overrides `features`
    #[arg(long)]
    all_features: bool,

    /// Build for the target triple
    #[arg(long)]
    target: Option<String>,
}

impl PublishArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            publish: resolve_bool_arg(self.publish, self.no_publish),
            registry: self.registry.clone(),
            verify: resolve_bool_arg(self.verify, self.no_verify),
            enable_features: (!self.features.is_empty()).then(|| self.features.clone()),
            enable_all_features: self.all_features.then(|| true),
            target: self.target.clone(),
            ..Default::default()
        }
    }
}
