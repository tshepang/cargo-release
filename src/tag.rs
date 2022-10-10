use crate::util::resolve_bool_arg;

#[derive(Clone, Default, Debug, clap::Args)]
#[command(next_help_heading = "Tag")]
pub struct TagArgs {
    #[arg(long, overrides_with("no_tag"), hide(true))]
    tag: bool,
    /// Do not create git tag
    #[arg(long, overrides_with("tag"))]
    no_tag: bool,

    /// Sign git tag
    #[arg(long, overrides_with("no_sign_tag"))]
    sign_tag: bool,
    #[arg(long, overrides_with("sign_tag"), hide(true))]
    no_sign_tag: bool,

    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    #[arg(long)]
    tag_prefix: Option<String>,

    /// The name of the git tag.
    #[arg(long)]
    tag_name: Option<String>,
}

impl TagArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            tag: resolve_bool_arg(self.tag, self.no_tag),
            sign_tag: resolve_bool_arg(self.sign_tag, self.no_sign_tag),
            tag_prefix: self.tag_prefix.clone(),
            tag_name: self.tag_name.clone(),
            ..Default::default()
        }
    }
}
