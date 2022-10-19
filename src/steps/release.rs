use crate::config;
use crate::error::CliError;
use crate::ops::cargo;
use crate::ops::git;
use crate::ops::replace::{Template, NOW};
use crate::steps::plan;

#[derive(Debug, Clone, clap::Args)]
pub struct ReleaseStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    #[command(flatten)]
    workspace: clap_cargo::Workspace,

    /// Process all packages whose current version is unpublished
    #[arg(long, conflicts_with = "level_or_version")]
    unpublished: bool,

    /// Either bump by LEVEL or set the VERSION for all selected packages
    #[arg(value_name = "LEVEL|VERSION")]
    level_or_version: Option<super::TargetVersion>,

    /// Semver metadata
    #[arg(short, long, requires = "level_or_version")]
    metadata: Option<String>,

    #[command(flatten)]
    config: crate::config::ConfigArgs,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,

    /// The name of tag for the previous release.
    #[arg(long)]
    prev_tag_name: Option<String>,
}

impl ReleaseStep {
    pub fn run(&self) -> Result<(), CliError> {
        git::git_version()?;
        let mut index = crates_index::Index::new_cargo_default()?;

        let ws_meta = self
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional dependencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()?;
        let ws_config = config::load_workspace_config(&self.config, &ws_meta)?;
        let mut pkgs = plan::load(&self.config, &ws_meta)?;

        for pkg in pkgs.values_mut() {
            if let Some(prev_tag) = self.prev_tag_name.as_ref() {
                // Trust the user that the tag passed in is the latest tag for the workspace and that
                // they don't care about any changes from before this tag.
                pkg.set_prior_tag(prev_tag.to_owned());
            }
            if let Some(level_or_version) = &self.level_or_version {
                pkg.bump(level_or_version, self.metadata.as_deref())?;
            }
        }

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in &excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };

            let crate_name = pkg.meta.name.as_str();
            let explicitly_excluded = self.workspace.exclude.contains(&excluded_pkg.name);
            // 1. Don't show this message if already not releasing in config
            // 2. Still respect `--exclude`
            if pkg.config.release() && pkg.config.publish() && !explicitly_excluded {
                let version = &pkg.initial_version;
                if !cargo::is_published(&index, crate_name, &version.full_version_string) {
                    log::debug!(
                        "Enabled {}, v{} is unpublished",
                        crate_name,
                        version.full_version_string
                    );
                    continue;
                }
            }

            pkg.config.release = Some(false);
            pkg.planned_version = None;

            if let Some(prior_tag_name) = &pkg.prior_tag {
                if let Some((changed, lock_changed)) =
                    crate::steps::version::changed_since(&ws_meta, pkg, prior_tag_name)
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

        for excluded_pkg in &excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };

            if pkg.config.publish() && pkg.config.registry().is_none() {
                let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
                let crate_name = pkg.meta.name.as_str();
                if !cargo::is_published(&index, crate_name, &version.full_version_string) {
                    log::warn!(
                        "Disabled by user, skipping {} v{} despite being unpublished",
                        crate_name,
                        version.full_version_string,
                    );
                }
            }
        }

        let (selected_pkgs, excluded_pkgs): (Vec<_>, Vec<_>) = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .partition(|p| p.config.release());
        if selected_pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(2.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        let consolidate_commits = super::consolidate_commits(&selected_pkgs, &excluded_pkgs)?;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Error,
        )?;

        failed |= !super::verify_tags_missing(&selected_pkgs, dry_run, log::Level::Error)?;

        failed |=
            !super::verify_monotonically_increasing(&selected_pkgs, dry_run, log::Level::Error)?;

        let mut double_publish = false;
        for pkg in &selected_pkgs {
            if !pkg.config.publish() {
                continue;
            }
            if pkg.config.registry().is_none() {
                let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
                let crate_name = pkg.meta.name.as_str();
                if cargo::is_published(&index, crate_name, &version.full_version_string) {
                    log::error!(
                        "{} {} is already published",
                        crate_name,
                        version.full_version_string
                    );
                    double_publish = true;
                }
            }
        }
        if double_publish {
            failed = true;
            if !dry_run {
                return Err(101.into());
            }
        }

        super::warn_changed(&ws_meta, &selected_pkgs)?;

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

        failed |= !super::verify_rate_limit(&selected_pkgs, &index, dry_run, log::Level::Error)?;

        // STEP 1: Release Confirmation
        super::confirm("Release", &selected_pkgs, self.no_confirm, dry_run)?;

        // STEP 2: update current version, save and commit
        if consolidate_commits {
            let update_lock =
                super::version::update_versions(&ws_meta, &selected_pkgs, &excluded_pkgs, dry_run)?;
            if update_lock {
                log::debug!("Updating lock file");
                if !dry_run {
                    let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
                    crate::ops::cargo::update_lock(&workspace_path)?;
                }
            }

            for pkg in &selected_pkgs {
                super::replace::replace(pkg, dry_run)?;

                // pre-release hook
                super::hook::hook(&ws_meta, pkg, dry_run)?;
            }

            workspace_commit(&ws_meta, &ws_config, &selected_pkgs, dry_run)?;
        } else {
            for pkg in &selected_pkgs {
                if let Some(version) = pkg.planned_version.as_ref() {
                    let crate_name = pkg.meta.name.as_str();
                    log::info!(
                        "Update {} to version {}",
                        crate_name,
                        version.full_version_string
                    );
                    cargo::set_package_version(
                        &pkg.manifest_path,
                        version.full_version_string.as_str(),
                        dry_run,
                    )?;
                    crate::steps::version::update_dependent_versions(
                        &ws_meta, pkg, version, dry_run,
                    )?;
                    if dry_run {
                        log::debug!("Updating lock file");
                    } else {
                        cargo::update_lock(&pkg.manifest_path)?;
                    }
                }

                super::replace::replace(pkg, dry_run)?;

                // pre-release hook
                super::hook::hook(&ws_meta, pkg, dry_run)?;

                pkg_commit(pkg, dry_run)?;
            }
        }

        // STEP 3: cargo publish
        super::publish::publish(&ws_meta, &selected_pkgs, &mut index, dry_run)?;

        // STEP 5: Tag
        super::tag::tag(&selected_pkgs, dry_run)?;

        // STEP 6: git push
        super::push::push(&ws_config, &ws_meta, &selected_pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }
}

pub fn pkg_commit(pkg: &plan::PackageRelease, dry_run: bool) -> Result<(), CliError> {
    let cwd = &pkg.package_root;
    let crate_name = pkg.meta.name.as_str();
    let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
    let prev_version_var = pkg.initial_version.bare_version_string.as_str();
    let prev_metadata_var = pkg.initial_version.full_version.build.as_str();
    let version_var = version.bare_version_string.as_str();
    let metadata_var = version.full_version.build.as_str();
    let template = Template {
        prev_version: Some(prev_version_var),
        prev_metadata: Some(prev_metadata_var),
        version: Some(version_var),
        metadata: Some(metadata_var),
        crate_name: Some(crate_name),
        date: Some(NOW.as_str()),
        ..Default::default()
    };
    let commit_msg = template.render(pkg.config.pre_release_commit_message());
    let sign = pkg.config.sign_commit();
    if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
        // commit failed, abort release
        return Err(101.into());
    }

    Ok(())
}

pub fn workspace_commit(
    ws_meta: &cargo_metadata::Metadata,
    ws_config: &config::Config,
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
) -> Result<(), CliError> {
    let shared_version = super::find_shared_versions(pkgs)?;

    let shared_commit_msg = {
        let version_var = shared_version
            .as_ref()
            .map(|v| v.bare_version_string.as_str());
        let metadata_var = shared_version
            .as_ref()
            .map(|v| v.full_version.build.as_str());
        let template = Template {
            version: version_var,
            metadata: metadata_var,
            date: Some(NOW.as_str()),
            ..Default::default()
        };
        template.render(ws_config.pre_release_commit_message())
    };
    if !git::commit_all(
        ws_meta.workspace_root.as_std_path(),
        &shared_commit_msg,
        ws_config.sign_commit(),
        dry_run,
    )? {
        // commit failed, abort release
        return Err(101.into());
    }

    Ok(())
}
