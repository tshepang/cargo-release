use crate::error::FatalError;
use crate::error::ProcessError;
use crate::ops::git;
use crate::ops::replace::{do_file_replacements, Template, NOW};
use crate::steps::plan;

/// Perform pre-release replacements
#[derive(Debug, Clone, clap::Args)]
pub struct ReplaceStep {
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
}

impl ReplaceStep {
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
            pkg.version = None;
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
            log::Level::Warn,
        )?;

        super::warn_changed(&ws_meta, &pkgs)?;

        failed |= !super::verify_git_branch(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Warn,
        )?;

        failed |= !super::verify_if_behind(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Warn,
        )?;

        // STEP 1: Release Confirmation
        super::confirm("Bump", &pkgs, self.no_confirm, dry_run)?;

        // STEP 2: update current version, save and commit
        for pkg in &pkgs {
            let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            if !pkg.config.pre_release_replacements().is_empty() {
                let cwd = &pkg.package_root;
                let crate_name = pkg.meta.name.as_str();
                let prev_version_var = pkg.prev_version.bare_version_string.as_str();
                let prev_metadata_var = pkg.prev_version.full_version.build.as_str();
                let version_var = version.bare_version_string.as_str();
                let metadata_var = version.full_version.build.as_str();
                // try replacing text in configured files
                let template = Template {
                    prev_version: Some(prev_version_var),
                    prev_metadata: Some(prev_metadata_var),
                    version: Some(version_var),
                    metadata: Some(metadata_var),
                    crate_name: Some(crate_name),
                    date: Some(NOW.as_str()),
                    tag_name: pkg.tag.as_deref(),
                    ..Default::default()
                };
                let prerelease = version.is_prerelease();
                let noisy = true;
                do_file_replacements(
                    pkg.config.pre_release_replacements(),
                    &template,
                    cwd,
                    prerelease,
                    noisy,
                    dry_run,
                )?;
            }
        }

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            allow_branch: self.allow_branch.clone(),
            ..Default::default()
        }
    }
}
