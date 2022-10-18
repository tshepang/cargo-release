use crate::error::FatalError;
use crate::error::ProcessError;
use crate::ops::git;
use crate::steps::plan;

/// Bump crate versions
#[derive(Debug, Clone, clap::Args)]
pub struct VersionStep {
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

    /// Either bump by LEVEL or set the VERSION for all selected packages
    #[arg(value_name = "LEVEL|VERSION", help_heading = "Version")]
    level_or_version: super::TargetVersion,

    /// Semver metadata
    #[arg(short, long, help_heading = "Version")]
    metadata: Option<String>,

    /// The name of tag for the previous release.
    #[arg(long, help_heading = "Version")]
    prev_tag_name: Option<String>,
}

impl VersionStep {
    pub fn run(&self) -> Result<(), ProcessError> {
        git::git_version()?;

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

        for pkg in pkgs.values_mut() {
            if let Some(prev_tag) = self.prev_tag_name.as_ref() {
                // Trust the user that the tag passed in is the latest tag for the workspace and that
                // they don't care about any changes from before this tag.
                pkg.set_prior_tag(prev_tag.to_owned());
            }
            pkg.bump(&self.level_or_version, self.metadata.as_deref())?;
        }

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            pkg.planned_version = None;
            pkg.config.release = Some(false);

            let crate_name = pkg.meta.name.as_str();
            if let Some(prior_tag_name) = &pkg.prior_tag {
                if let Some((changed, lock_changed)) = changed_since(&ws_meta, pkg, prior_tag_name)
                {
                    if !changed.is_empty() {
                        log::warn!(
                            "Disabled by user, skipping {} which has files changed since {}: {:#?}",
                            crate_name,
                            prior_tag_name,
                            changed
                        );
                    } else if lock_changed {
                        log::warn!(
                        "Disabled by user, skipping {} despite lock file being changed since {}",
                        crate_name,
                        prior_tag_name
                    );
                    } else {
                        log::trace!(
                            "Disabled by user, skipping {} (no changes since {})",
                            crate_name,
                            prior_tag_name
                        );
                    }
                } else {
                    log::debug!(
                        "Disabled by user, skipping {} (no {} tag)",
                        crate_name,
                        prior_tag_name
                    );
                }
            } else {
                log::debug!("Disabled by user, skipping {} (no tag found)", crate_name,);
            }
        }

        let pkgs = plan::plan(pkgs)?;

        let (selected_pkgs, _excluded_pkgs): (Vec<_>, Vec<_>) = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .partition(|p| p.config.release());
        if selected_pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(2.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Warn,
        )?;

        super::warn_changed(&ws_meta, &selected_pkgs)?;

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
        super::confirm("Bump", &selected_pkgs, self.no_confirm, dry_run)?;

        // STEP 2: update current version, save and commit
        let mut update_lock = false;
        for pkg in &selected_pkgs {
            if let Some(version) = pkg.planned_version.as_ref() {
                let crate_name = pkg.meta.name.as_str();
                log::info!(
                    "Update {} to version {}",
                    crate_name,
                    version.full_version_string
                );
                crate::ops::cargo::set_package_version(
                    &pkg.manifest_path,
                    version.full_version_string.as_str(),
                    dry_run,
                )?;
                update_dependent_versions(&ws_meta, pkg, version, dry_run)?;
                update_lock = true;
            }
        }
        if update_lock {
            log::debug!("Updating lock file");
            if !dry_run {
                let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
                crate::ops::cargo::update_lock(&workspace_path)?;
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

pub fn changed_since<'m>(
    ws_meta: &cargo_metadata::Metadata,
    pkg: &'m plan::PackageRelease,
    since_ref: &str,
) -> Option<(Vec<std::path::PathBuf>, bool)> {
    let lock_path = ws_meta.workspace_root.join("Cargo.lock");
    let changed_root = if pkg.bin {
        ws_meta.workspace_root.as_std_path()
    } else {
        // Limit our lookup since we don't need to check for `Cargo.lock`
        &pkg.package_root
    };
    let changed = git::changed_files(changed_root, since_ref).ok().flatten()?;
    let mut changed: Vec<_> = changed
        .into_iter()
        .filter(|p| pkg.package_content.contains(p))
        .collect();

    let mut lock_changed = false;
    if let Some(lock_index) =
        changed.iter().enumerate().find_map(
            |(idx, path)| {
                if path == &lock_path {
                    Some(idx)
                } else {
                    None
                }
            },
        )
    {
        let _ = changed.swap_remove(lock_index);
        if !pkg.bin {
            let crate_name = pkg.meta.name.as_str();
            log::trace!(
                "Ignoring lock file change since {}; {} has no [[bin]]",
                since_ref,
                crate_name
            );
        } else {
            lock_changed = true;
        }
    }

    Some((changed, lock_changed))
}

pub fn update_dependent_versions(
    ws_meta: &cargo_metadata::Metadata,
    pkg: &plan::PackageRelease,
    version: &plan::Version,
    dry_run: bool,
) -> Result<(), FatalError> {
    // This is redundant with iterating over `workspace_members`
    // - As `find_dependency_tables` returns workspace dependencies
    // - If there is a root package
    //
    // But split this out for
    // - Virtual manifests
    // - Nicer message to the user
    {
        let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
        crate::ops::cargo::upgrade_dependency_req(
            "workspace",
            &workspace_path,
            &pkg.package_root,
            &pkg.meta.name,
            &version.full_version,
            pkg.config.dependent_version(),
            dry_run,
        )?;
    }

    for dep in find_ws_members(ws_meta) {
        crate::ops::cargo::upgrade_dependency_req(
            &dep.name,
            dep.manifest_path.as_std_path(),
            &pkg.package_root,
            &pkg.meta.name,
            &version.full_version,
            pkg.config.dependent_version(),
            dry_run,
        )?;
    }

    Ok(())
}

fn find_ws_members(
    ws_meta: &cargo_metadata::Metadata,
) -> impl Iterator<Item = &cargo_metadata::Package> {
    let workspace_members: std::collections::HashSet<_> =
        ws_meta.workspace_members.iter().collect();
    ws_meta
        .packages
        .iter()
        .filter(move |p| workspace_members.contains(&p.id))
}
