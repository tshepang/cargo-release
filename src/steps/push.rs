use std::collections::HashSet;

use crate::error::FatalError;
use crate::error::ProcessError;
use crate::ops::git;
use crate::steps::plan;

/// Push tags/commits to remote
#[derive(Debug, Clone, clap::Args)]
pub struct PushStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    #[command(flatten)]
    workspace: clap_cargo::Workspace,

    /// Custom config file
    #[arg(short, long = "config")]
    custom_config: Option<String>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    isolated: bool,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,

    #[command(flatten)]
    tag: crate::config::TagArgs,

    #[command(flatten)]
    push: crate::config::PushArgs,
}

impl PushStep {
    pub fn run(&self) -> Result<(), ProcessError> {
        let ws_meta = self
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional dependencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()
            .map_err(FatalError::from)?;
        let config = self.to_config();
        let ws_config = crate::config::load_workspace_config(&config, &ws_meta)?;
        let mut pkgs = plan::load(&config, &ws_meta)?;

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            pkg.config.release = Some(false);

            let crate_name = pkg.meta.name.as_str();
            log::debug!("Disabled by user, skipping {}", crate_name,);
        }

        let pkgs = plan::plan(pkgs)?;

        let pkgs: Vec<_> = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .filter(|p| p.config.release())
            .collect();
        if pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(0.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        git::git_version()?;

        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Error,
        )?;

        failed |= !super::verify_tags_exist(&pkgs, dry_run, log::Level::Error)?;

        failed |= !super::verify_git_branch(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Error,
        )?;

        failed |= !super::verify_if_behind(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Warn,
        )?;

        // STEP 1: Release Confirmation
        super::confirm("Push", &pkgs, self.no_confirm, dry_run)?;

        // STEP 7: git push
        push(&ws_config, &ws_meta, &pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            allow_branch: self.allow_branch.clone(),
            tag: self.tag.clone(),
            push: self.push.clone(),
            ..Default::default()
        }
    }
}

pub fn push(
    ws_config: &crate::config::Config,
    ws_meta: &cargo_metadata::Metadata,
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
) -> Result<(), ProcessError> {
    if ws_config.push() {
        let git_remote = ws_config.push_remote();
        let branch = crate::ops::git::current_branch(ws_meta.workspace_root.as_std_path())?;

        let mut shared_refs = HashSet::new();
        for pkg in pkgs {
            if !pkg.config.push() {
                continue;
            }

            if pkg.config.consolidate_pushes() {
                shared_refs.insert(branch.as_str());
                if let Some(tag_name) = pkg.tag.as_deref() {
                    shared_refs.insert(tag_name);
                }
            } else {
                let mut refs = vec![branch.as_str()];
                if let Some(tag_name) = pkg.tag.as_deref() {
                    refs.push(tag_name)
                }
                log::info!("Pushing {} to {}", refs.join(", "), git_remote);
                let cwd = &pkg.package_root;
                if !git::push(cwd, git_remote, refs, pkg.config.push_options(), dry_run)? {
                    return Err(106.into());
                }
            }
        }
        if !shared_refs.is_empty() {
            let mut shared_refs = shared_refs.into_iter().collect::<Vec<_>>();
            shared_refs.sort_unstable();
            log::info!("Pushing {} to {}", shared_refs.join(", "), git_remote);
            if !git::push(
                ws_meta.workspace_root.as_std_path(),
                git_remote,
                shared_refs,
                ws_config.push_options(),
                dry_run,
            )? {
                return Err(106.into());
            }
        }
    }

    Ok(())
}
