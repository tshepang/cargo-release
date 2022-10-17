use std::str::FromStr;

pub mod config;
pub mod plan;
pub mod publish;
pub mod push;
pub mod release;
pub mod replace;
pub mod tag;
pub mod version;

use crate::error::FatalError;
use crate::ops::version::VersionExt as _;

pub fn verify_git_is_clean(
    path: &std::path::Path,
    dry_run: bool,
    level: log::Level,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;
    if crate::ops::git::is_dirty(path)? {
        log::log!(
            level,
            "Uncommitted changes detected, please commit before release."
        );
        if level == log::Level::Error {
            success = false;
            if !dry_run {
                return Err(101.into());
            }
        }
    }
    Ok(success)
}

pub fn verify_tags_missing(
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
    level: log::Level,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    let mut tag_exists = false;
    let mut seen_tags = std::collections::HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.planned_tag.as_ref() {
            if seen_tags.insert(tag_name) {
                let cwd = &pkg.package_root;
                if crate::ops::git::tag_exists(cwd, tag_name)? {
                    let crate_name = pkg.meta.name.as_str();
                    log::log!(
                        level,
                        "Tag `{}` already exists (for `{}`)",
                        tag_name,
                        crate_name
                    );
                    tag_exists = true;
                }
            }
        }
    }
    if tag_exists && level == log::Level::Error {
        success = false;
        if !dry_run {
            return Err(101.into());
        }
    }

    Ok(success)
}

pub fn verify_tags_exist(
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
    level: log::Level,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    let mut tag_missing = false;
    let mut seen_tags = std::collections::HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.planned_tag.as_ref() {
            if seen_tags.insert(tag_name) {
                let cwd = &pkg.package_root;
                if !crate::ops::git::tag_exists(cwd, tag_name)? {
                    let crate_name = pkg.meta.name.as_str();
                    log::log!(
                        level,
                        "Tag `{}` doesn't exist (for `{}`)",
                        tag_name,
                        crate_name
                    );
                    tag_missing = true;
                }
            }
        }
    }
    if tag_missing && level == log::Level::Error {
        success = false;
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
    level: log::Level,
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
        log::log!(
            level,
            "Cannot release from branch {:?}, instead switch to {:?}",
            branch,
            ws_config.allow_branch().join(", ")
        );
        log::trace!("Due to {:?}", good_branch_match);
        if level == log::Level::Error {
            success = false;
            if !dry_run {
                return Err(101.into());
            }
        }
    }

    Ok(success)
}

pub fn verify_if_behind(
    path: &std::path::Path,
    ws_config: &crate::config::Config,
    dry_run: bool,
    level: log::Level,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    let git_remote = ws_config.push_remote();
    let branch = crate::ops::git::current_branch(path)?;
    crate::ops::git::fetch(path, git_remote, &branch)?;
    if crate::ops::git::is_behind_remote(path, git_remote, &branch)? {
        log::log!(level, "{} is behind {}/{}", branch, git_remote, branch);
        if level == log::Level::Error {
            success = false;
            if !dry_run {
                return Err(101.into());
            }
        }
    }

    Ok(success)
}

pub fn verify_monotonically_increasing(
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
    level: log::Level,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    let mut downgrades_present = false;
    for pkg in pkgs {
        if let Some(version) = pkg.planned_version.as_ref() {
            if version.full_version < pkg.initial_version.full_version {
                let crate_name = pkg.meta.name.as_str();
                log::log!(
                    level,
                    "Cannot downgrade {} from {} to {}",
                    crate_name,
                    version.full_version,
                    pkg.initial_version.full_version
                );
                downgrades_present = true;
            }
        }
    }
    if downgrades_present && level == log::Level::Error {
        success = false;
        if !dry_run {
            return Err(101.into());
        }
    }

    Ok(success)
}

pub fn verify_rate_limit(
    pkgs: &[plan::PackageRelease],
    index: &crates_index::Index,
    dry_run: bool,
    level: log::Level,
) -> Result<bool, crate::error::ProcessError> {
    let mut success = true;

    // "It's not particularly secret, we just don't publish it other than in the code because
    // it's subject to change. The responses from the rate limited requests on when to try
    // again contain the most accurate information."
    let mut new = 0;
    let mut existing = 0;
    for pkg in pkgs {
        if pkg.config.registry().is_none() {
            let crate_name = pkg.meta.name.as_str();
            if index.crate_(crate_name).is_some() {
                existing += 1;
            } else {
                new += 1;
            }
        }
    }

    if 5 < new {
        // "The rate limit for creating new crates is 1 crate every 10 minutes, with a burst of 5 crates."
        success = false;
        log::log!(
            level,
            "Attempting to publish {} new crates which is above the crates.io rate limit",
            new
        );
    }

    if 30 < existing {
        // "The rate limit for new versions of existing crates is 1 per minute, with a burst of 30 crates, so when releasing new versions of these crates, you shouldn't hit the limit."
        success = false;
        log::log!(
            level,
            "Attempting to publish {} existing crates which is above the crates.io rate limit",
            existing
        );
    }

    if !success && level == log::Level::Error && !dry_run {
        return Err(101.into());
    }

    Ok(success)
}

pub fn warn_changed(
    ws_meta: &cargo_metadata::Metadata,
    pkgs: &[plan::PackageRelease],
) -> Result<(), crate::error::ProcessError> {
    let mut changed_pkgs = std::collections::HashSet::new();
    for pkg in pkgs {
        let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
        let crate_name = pkg.meta.name.as_str();
        if let Some(prior_tag_name) = &pkg.prior_tag {
            if let Some((changed, lock_changed)) =
                crate::steps::version::changed_since(ws_meta, pkg, prior_tag_name)
            {
                if !changed.is_empty() {
                    log::debug!(
                        "Files changed in {} since {}: {:#?}",
                        crate_name,
                        prior_tag_name,
                        changed
                    );
                    changed_pkgs.insert(&pkg.meta.id);
                    changed_pkgs.extend(pkg.dependents.iter().map(|d| &d.pkg.id));
                } else if changed_pkgs.contains(&pkg.meta.id) {
                    log::debug!(
                        "Dependency changed for {} since {}",
                        crate_name,
                        prior_tag_name,
                    );
                    changed_pkgs.insert(&pkg.meta.id);
                    changed_pkgs.extend(pkg.dependents.iter().map(|d| &d.pkg.id));
                } else if lock_changed {
                    log::debug!(
                        "Lock file changed for {} since {}, assuming its relevant",
                        crate_name,
                        prior_tag_name
                    );
                    changed_pkgs.insert(&pkg.meta.id);
                    // Lock file changes don't invalidate dependents, which is why this check is
                    // after the transitive check, so that can invalidate dependents
                } else {
                    log::warn!(
                        "Updating {} to {} despite no changes made since tag {}",
                        crate_name,
                        version.full_version_string,
                        prior_tag_name
                    );
                }
            } else {
                log::debug!(
                        "Cannot detect changes for {} because tag {} is missing. Try setting `--prev-tag-name <TAG>`.",
                        crate_name,
                        prior_tag_name
                    );
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

pub fn find_shared_versions(
    pkgs: &[plan::PackageRelease],
) -> Result<Option<plan::Version>, crate::error::ProcessError> {
    let mut is_shared = true;
    let mut shared_versions: std::collections::HashMap<&str, &plan::Version> = Default::default();
    for pkg in pkgs {
        let group_name = if let Some(group_name) = pkg.config.shared_version() {
            group_name
        } else {
            continue;
        };
        let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
        match shared_versions.entry(group_name) {
            std::collections::hash_map::Entry::Occupied(existing) => {
                if version.bare_version != existing.get().bare_version {
                    is_shared = false;
                    log::error!(
                        "{} has version {}, should be {}",
                        pkg.meta.name,
                        version.bare_version_string,
                        existing.get().bare_version_string
                    );
                }
            }
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(version);
            }
        }
    }
    if !is_shared {
        log::error!("Crate versions deviated, aborting");
        return Err(101.into());
    }

    if shared_versions.len() == 1 {
        Ok(shared_versions.values().next().map(|s| (*s).clone()))
    } else {
        Ok(None)
    }
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
            let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
            format!("{} {} {}?", step, crate_name, version.full_version_string)
        } else {
            use std::io::Write;

            let mut buffer: Vec<u8> = vec![];
            writeln!(&mut buffer, "{}", step).unwrap();
            for pkg in pkgs {
                let crate_name = pkg.meta.name.as_str();
                let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
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
            Err(101.into())
        } else {
            log::warn!("Ran a `dry-run`, re-run with `--execute` if all looked good.");
            Ok(())
        }
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum TargetVersion {
    Relative(BumpLevel),
    Absolute(semver::Version),
}

impl TargetVersion {
    pub fn bump(
        &self,
        current: &semver::Version,
        metadata: Option<&str>,
    ) -> Result<Option<plan::Version>, FatalError> {
        let bumped = match self {
            TargetVersion::Relative(bump_level) => {
                let mut potential_version = current.to_owned();
                if bump_level.bump_version(&mut potential_version, metadata)? {
                    let full_version = potential_version;
                    let version = plan::Version::from(full_version);
                    Some(version)
                } else {
                    None
                }
            }
            TargetVersion::Absolute(version) => {
                let mut full_version = version.to_owned();
                if full_version.build.is_empty() {
                    if let Some(metadata) = metadata {
                        full_version.build = semver::BuildMetadata::new(metadata)?;
                    } else {
                        full_version.build = current.build.clone();
                    }
                }
                let version = plan::Version::from(full_version);
                if version.bare_version != plan::Version::from(current.clone()).bare_version {
                    Some(version)
                } else {
                    None
                }
            }
        };
        Ok(bumped)
    }
}

impl Default for TargetVersion {
    fn default() -> Self {
        TargetVersion::Relative(BumpLevel::Release)
    }
}

impl std::fmt::Display for TargetVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            TargetVersion::Relative(bump_level) => {
                write!(f, "{}", bump_level)
            }
            TargetVersion::Absolute(version) => {
                write!(f, "{}", version)
            }
        }
    }
}

impl std::str::FromStr for TargetVersion {
    type Err = FatalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(bump_level) = BumpLevel::from_str(s) {
            Ok(TargetVersion::Relative(bump_level))
        } else {
            Ok(TargetVersion::Absolute(semver::Version::parse(s)?))
        }
    }
}

impl clap::builder::ValueParserFactory for TargetVersion {
    type Parser = TargetVersionParser;

    fn value_parser() -> Self::Parser {
        TargetVersionParser
    }
}

#[derive(Copy, Clone)]
pub struct TargetVersionParser;

impl clap::builder::TypedValueParser for TargetVersionParser {
    type Value = TargetVersion;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let inner_parser = TargetVersion::from_str;
        inner_parser.parse_ref(cmd, arg, value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        let inner_parser = clap::builder::EnumValueParser::<BumpLevel>::new();
        #[allow(clippy::needless_collect)] // Erasing a lifetime
        inner_parser.possible_values().map(|ps| {
            let ps = ps.collect::<Vec<_>>();
            let ps: Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_> =
                Box::new(ps.into_iter());
            ps
        })
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BumpLevel {
    /// Increase the major version (x.0.0)
    Major,
    /// Increase the minor version (x.y.0)
    Minor,
    /// Increase the patch version (x.y.z)
    Patch,
    /// Increase the rc pre-version (x.y.z-rc.M)
    Rc,
    /// Increase the beta pre-version (x.y.z-beta.M)
    Beta,
    /// Increase the alpha pre-version (x.y.z-alpha.M)
    Alpha,
    /// Remove the pre-version (x.y.z)
    Release,
}

impl std::fmt::Display for BumpLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl std::str::FromStr for BumpLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::ValueEnum;

        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {}", s))
    }
}

impl BumpLevel {
    pub fn bump_version(
        self,
        version: &mut semver::Version,
        metadata: Option<&str>,
    ) -> Result<bool, FatalError> {
        let mut need_commit = false;
        match self {
            BumpLevel::Major => {
                version.increment_major();
                need_commit = true;
            }
            BumpLevel::Minor => {
                version.increment_minor();
                need_commit = true;
            }
            BumpLevel::Patch => {
                if !version.is_prerelease() {
                    version.increment_patch();
                } else {
                    version.pre = semver::Prerelease::EMPTY;
                }
                need_commit = true;
            }
            BumpLevel::Rc => {
                version.increment_rc()?;
                need_commit = true;
            }
            BumpLevel::Beta => {
                version.increment_beta()?;
                need_commit = true;
            }
            BumpLevel::Alpha => {
                version.increment_alpha()?;
                need_commit = true;
            }
            BumpLevel::Release => {
                if version.is_prerelease() {
                    version.pre = semver::Prerelease::EMPTY;
                    need_commit = true;
                }
            }
        };

        if let Some(metadata) = metadata {
            version.metadata(metadata)?;
        }

        Ok(need_commit)
    }
}
