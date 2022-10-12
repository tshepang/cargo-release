use std::collections::HashSet;
use std::io::Write;

use itertools::Itertools;

use crate::error::FatalError;
use crate::error::ProcessError;
use crate::git;
use crate::shell;

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

    #[command(flatten)]
    tag: crate::tag::TagArgs,

    #[command(flatten)]
    push: crate::config::PushArgs,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,
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
        let mut pkgs = crate::plan::load(&config, &ws_meta)?;

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

        let pkgs = crate::plan::plan(pkgs)?;

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

        if git::is_dirty(ws_meta.workspace_root.as_std_path())? {
            log::error!("Uncommitted changes detected, please commit before release.");
            failed = true;
            if !dry_run {
                return Err(101.into());
            }
        }

        let mut tag_missing = false;
        let mut seen_tags = HashSet::new();
        for pkg in &pkgs {
            if let Some(tag_name) = pkg.tag.as_ref() {
                if seen_tags.insert(tag_name) {
                    let cwd = &pkg.package_root;
                    if !git::tag_exists(cwd, tag_name)? {
                        let crate_name = pkg.meta.name.as_str();
                        log::error!("Tag `{}` doesn't exist (for `{}`)", tag_name, crate_name);
                        tag_missing = true;
                    }
                }
            }
        }
        if tag_missing {
            failed = true;
            if !dry_run {
                return Err(101.into());
            }
        }

        let git_remote = ws_config.push_remote();
        let branch = git::current_branch(ws_meta.workspace_root.as_std_path())?;
        let mut good_branches = ignore::gitignore::GitignoreBuilder::new(".");
        for pattern in ws_config.allow_branch() {
            good_branches.add_line(None, pattern)?;
        }
        let good_branches = good_branches.build()?;
        let good_branch_match = good_branches.matched_path_or_any_parents(&branch, false);
        if !good_branch_match.is_ignore() {
            log::error!(
                "Cannot release from branch {:?}, instead switch to {:?}",
                branch,
                ws_config.allow_branch().join(", ")
            );
            log::trace!("Due to {:?}", good_branch_match);
            failed = true;
            if !dry_run {
                return Err(101.into());
            }
        }
        git::fetch(ws_meta.workspace_root.as_std_path(), git_remote, &branch)?;
        if git::is_behind_remote(ws_meta.workspace_root.as_std_path(), git_remote, &branch)? {
            log::warn!("{} is behind {}/{}", branch, git_remote, branch);
        }

        // STEP 1: Release Confirmation
        if !dry_run && !self.no_confirm {
            let prompt = if pkgs.len() == 1 {
                let pkg = &pkgs[0];
                let crate_name = pkg.meta.name.as_str();
                let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
                format!("Push {} {}?", crate_name, version.full_version_string)
            } else {
                let mut buffer: Vec<u8> = vec![];
                writeln!(&mut buffer, "Push").unwrap();
                for pkg in &pkgs {
                    let crate_name = pkg.meta.name.as_str();
                    let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
                    writeln!(
                        &mut buffer,
                        "  {} {}",
                        crate_name, version.full_version_string
                    )
                    .unwrap();
                }
                write!(&mut buffer, "?").unwrap();
                String::from_utf8(buffer).expect("Only valid UTF-8 has been written")
            };

            let confirmed = shell::confirm(&prompt);
            if !confirmed {
                return Err(0.into());
            }
        }

        // STEP 7: git push
        if ws_config.push() {
            let mut shared_refs = HashSet::new();
            for pkg in &pkgs {
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

        if dry_run {
            if failed {
                log::error!("Dry-run failed, resolve the above errors and try again.");
                Err(107.into())
            } else {
                log::warn!("Ran a `dry-run`, re-run with `--execute` if all looked good.");
                Ok(())
            }
        } else {
            Ok(())
        }
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
