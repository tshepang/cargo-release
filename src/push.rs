use crate::util::resolve_bool_arg;

#[derive(Clone, Debug, clap::Args)]
#[command(next_help_heading = "Push")]
pub struct PushArgs {
    #[arg(long, overrides_with("no_push"), hide(true))]
    push: bool,
    /// Do not run git push in the last step
    #[arg(long, overrides_with("push"))]
    no_push: bool,

    /// Git remote to push
    #[arg(long)]
    push_remote: Option<String>,
}

impl PushArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            push: resolve_bool_arg(self.push, self.no_push),
            push_remote: self.push_remote.clone(),
            ..Default::default()
        }
    }
}
