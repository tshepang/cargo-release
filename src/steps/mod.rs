pub mod config;
pub mod plan;
pub mod push;
pub mod release;

pub fn verify_git_is_clean(
    path: &std::path::Path,
    dry_run: bool,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;
    if crate::ops::git::is_dirty(path)? {
        log::error!("Uncommitted changes detected, please commit before release.");
        success = false;
        if !dry_run {
            return Err(101.into());
        }
    }
    Ok(success)
}

pub fn verify_tags_missing(
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    let mut tag_exists = false;
    let mut seen_tags = std::collections::HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.tag.as_ref() {
            if seen_tags.insert(tag_name) {
                let cwd = &pkg.package_root;
                if crate::ops::git::tag_exists(cwd, tag_name)? {
                    let crate_name = pkg.meta.name.as_str();
                    log::error!("Tag `{}` already exists (for `{}`)", tag_name, crate_name);
                    tag_exists = true;
                }
            }
        }
    }
    if tag_exists {
        success = true;
        if !dry_run {
            return Err(101.into());
        }
    }

    Ok(success)
}

pub fn verify_tags_exist(
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    let mut tag_missing = false;
    let mut seen_tags = std::collections::HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.tag.as_ref() {
            if seen_tags.insert(tag_name) {
                let cwd = &pkg.package_root;
                if !crate::ops::git::tag_exists(cwd, tag_name)? {
                    let crate_name = pkg.meta.name.as_str();
                    log::error!("Tag `{}` doesn't exist (for `{}`)", tag_name, crate_name);
                    tag_missing = true;
                }
            }
        }
    }
    if tag_missing {
        success = true;
        if !dry_run {
            return Err(101.into());
        }
    }

    Ok(success)
}

pub fn verify_git_branch(
    path: &std::path::Path,
    ws_config: &crate::config::Config,
    dry_run: bool,
) -> Result<bool, crate::error::ProcessError> {
    use itertools::Itertools;

    let mut success = true;

    let branch = crate::ops::git::current_branch(path)?;
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
        success = true;
        if !dry_run {
            return Err(101.into());
        }
    }

    Ok(success)
}

pub fn warn_if_behind(
    path: &std::path::Path,
    ws_config: &crate::config::Config,
) -> Result<(), crate::error::ProcessError> {
    let git_remote = ws_config.push_remote();
    let branch = crate::ops::git::current_branch(path)?;
    crate::ops::git::fetch(path, git_remote, &branch)?;
    if crate::ops::git::is_behind_remote(path, git_remote, &branch)? {
        log::warn!("{} is behind {}/{}", branch, git_remote, branch);
    }

    Ok(())
}

pub fn find_shared_versions(
    pkgs: &[plan::PackageRelease],
) -> Result<Option<crate::ops::version::Version>, crate::error::ProcessError> {
    let mut is_shared = true;
    let mut shared_version: Option<crate::ops::version::Version> = None;
    for pkg in pkgs {
        if let Some(version) = pkg.version.as_ref() {
            if pkg.config.shared_version() && pkg.version.is_some() {
                if let Some(shared_version) = shared_version.as_ref() {
                    if shared_version.bare_version != version.bare_version {
                        is_shared = false;
                        log::error!(
                            "{} has version {}, should be {}",
                            pkg.meta.name,
                            version.bare_version,
                            shared_version.bare_version_string
                        );
                    }
                } else {
                    shared_version = Some(version.clone());
                }
            }
        }
    }
    if !is_shared {
        log::error!("Crate versions deviated, aborting");
        return Err(110.into());
    }

    Ok(shared_version)
}

pub fn confirm(
    step: &str,
    pkgs: &[plan::PackageRelease],
    no_confirm: bool,
    dry_run: bool,
) -> Result<(), crate::error::ProcessError> {
    if !dry_run && !no_confirm {
        let prompt = if pkgs.len() == 1 {
            let pkg = &pkgs[0];
            let crate_name = pkg.meta.name.as_str();
            let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            format!("{} {} {}?", step, crate_name, version.full_version_string)
        } else {
            use std::io::Write;

            let mut buffer: Vec<u8> = vec![];
            writeln!(&mut buffer, "{}", step).unwrap();
            for pkg in pkgs {
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

        let confirmed = crate::ops::shell::confirm(&prompt);
        if !confirmed {
            return Err(0.into());
        }
    }

    Ok(())
}

pub fn finish(failed: bool, dry_run: bool) -> Result<(), crate::error::ProcessError> {
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
