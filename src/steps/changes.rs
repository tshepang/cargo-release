use crate::error::CargoResult;
use crate::error::CliError;
use crate::ops::git;
use crate::ops::shell::Color;
use crate::ops::shell::ColorSpec;
use crate::steps::plan;

/// Bump crate versions
#[derive(Debug, Clone, clap::Args)]
pub struct ChangesStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    /// Custom config file
    #[arg(short, long = "config")]
    custom_config: Option<String>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    isolated: bool,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,
}

impl ChangesStep {
    pub fn run(&self) -> Result<(), CliError> {
        git::git_version()?;

        let ws_meta = self
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional dependencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()?;
        let config = self.to_config();
        let ws_config = crate::config::load_workspace_config(&config, &ws_meta)?;
        let pkgs = plan::load(&config, &ws_meta)?;
        let pkgs = plan::plan(pkgs)?;

        let (selected_pkgs, _excluded_pkgs): (Vec<_>, Vec<_>) = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .partition(|p| p.config.release());
        if selected_pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(2.into());
        }

        let dry_run = false;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Warn,
        )?;

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

        changes(&ws_meta, &selected_pkgs)?;

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

pub fn changes(
    ws_meta: &cargo_metadata::Metadata,
    selected_pkgs: &[plan::PackageRelease],
) -> CargoResult<()> {
    for pkg in selected_pkgs {
        let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
        let crate_name = pkg.meta.name.as_str();
        if let Some(prior_tag_name) = &pkg.prior_tag {
            let workspace_root = ws_meta.workspace_root.as_std_path();
            let repo = git2::Repository::discover(workspace_root)?;

            let mut tag_id = None;
            let fq_prior_tag_name = format!("refs/tags/{}", prior_tag_name);
            repo.tag_foreach(|id, name| {
                if name == fq_prior_tag_name.as_bytes() {
                    tag_id = Some(id);
                    false
                } else {
                    true
                }
            })?;
            let tag_id = tag_id
                .ok_or_else(|| anyhow::format_err!("could not find tag {}", prior_tag_name))?;

            let head_id = repo.head()?.peel_to_commit()?.id();

            let mut revwalk = repo.revwalk()?;
            revwalk.push_range(&format!("{tag_id}..{head_id}"))?;

            let mut commits = Vec::new();
            for commit_id in revwalk {
                let commit_id = commit_id?;
                let commit = repo.find_commit(commit_id)?;
                if 1 < commit.parent_count() {
                    // Assuming merge commits can be ignored
                    continue;
                }
                let parent_tree = commit.parent(0).ok().map(|c| c.tree()).transpose()?;
                let tree = commit.tree()?;
                let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

                let mut changed_paths = std::collections::BTreeSet::new();
                for delta in diff.deltas() {
                    let old_path = delta.old_file().path();
                    let new_path = delta.new_file().path();
                    for entry_relpath in [old_path, new_path].into_iter().flatten() {
                        for path in pkg
                            .package_content
                            .iter()
                            .filter_map(|p| p.strip_prefix(&workspace_root).ok())
                        {
                            if path == entry_relpath {
                                changed_paths.insert(path.to_owned());
                            }
                        }
                    }
                }

                if !changed_paths.is_empty() {
                    let short_id =
                        String::from_utf8_lossy(&repo.find_object(commit_id, None)?.short_id()?)
                            .into_owned();
                    commits.push(PackageCommit {
                        id: commit_id,
                        short_id,
                        summary: String::from_utf8_lossy(commit.summary_bytes().unwrap_or(b""))
                            .into_owned(),
                        message: String::from_utf8_lossy(commit.message_bytes()).into_owned(),
                        paths: changed_paths,
                    });
                }
            }

            if !commits.is_empty() {
                crate::ops::shell::status(
                    "Changes",
                    format!(
                        "for {} from {} to {}",
                        crate_name, prior_tag_name, version.full_version_string
                    ),
                )?;
                let prefix = format!("{:>13}", " ");
                for commit in &commits {
                    let _ = crate::ops::shell::write_stderr(&prefix, &ColorSpec::new());
                    let _ = crate::ops::shell::write_stderr(
                        &commit.short_id,
                        ColorSpec::new().set_fg(Some(Color::Yellow)),
                    );
                    let _ = crate::ops::shell::write_stderr(" ", &ColorSpec::new());
                    let _ = crate::ops::shell::write_stderr(&commit.summary, &ColorSpec::new());

                    write_status(commit.status());
                    let _ = crate::ops::shell::write_stderr("\n", &ColorSpec::new());
                }
            }
        } else {
            log::debug!(
                    "Cannot detect changes for {} because no tag was found. Try setting `--prev-tag-name <TAG>`.",
                    crate_name,
                );
        }
    }

    Ok(())
}

fn write_status(status: Option<CommitStatus>) {
    if let Some(status) = status {
        let suffix;
        let mut color = ColorSpec::new();
        match status {
            CommitStatus::Breaking => {
                suffix = " (breaking)";
                color.set_fg(Some(Color::Red));
            }
            CommitStatus::Feature => {
                suffix = " (feature)";
                color.set_fg(Some(Color::Yellow));
            }
            CommitStatus::Fix => {
                suffix = " (fix)";
                color.set_fg(Some(Color::Green));
            }
            CommitStatus::Ignore => {
                suffix = "";
            }
        }
        let _ = crate::ops::shell::write_stderr(suffix, &color);
    }
}

#[derive(Clone, Debug)]
pub struct PackageCommit {
    pub id: git2::Oid,
    pub short_id: String,
    pub summary: String,
    pub message: String,
    pub paths: std::collections::BTreeSet<std::path::PathBuf>,
}

impl PackageCommit {
    pub fn status(&self) -> Option<CommitStatus> {
        if let Some(status) = self.conventional_status() {
            return status;
        }

        None
    }

    fn conventional_status(&self) -> Option<Option<CommitStatus>> {
        let parts = git_conventional::Commit::parse(&self.message).ok()?;
        if parts.breaking() {
            return Some(Some(CommitStatus::Breaking));
        }

        if [
            git_conventional::Type::CHORE,
            git_conventional::Type::TEST,
            git_conventional::Type::STYLE,
            git_conventional::Type::REFACTOR,
            git_conventional::Type::REVERT,
        ]
        .contains(&parts.type_())
        {
            Some(Some(CommitStatus::Ignore))
        } else if [
            git_conventional::Type::DOCS,
            git_conventional::Type::PERF,
            git_conventional::Type::FIX,
        ]
        .contains(&parts.type_())
        {
            Some(Some(CommitStatus::Fix))
        } else if [git_conventional::Type::FEAT].contains(&parts.type_()) {
            Some(Some(CommitStatus::Feature))
        } else {
            Some(None)
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommitStatus {
    Breaking,
    Feature,
    Fix,
    Ignore,
}
