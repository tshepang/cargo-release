use std::path::Path;
use std::path::PathBuf;

use crate::config;
use crate::error;
use crate::error::FatalError;
use crate::ops::cargo;
use crate::ops::git;
use crate::ops::replace::Template;
use crate::ops::version;

pub fn load(
    args: &config::ConfigArgs,
    ws_meta: &cargo_metadata::Metadata,
) -> Result<indexmap::IndexMap<cargo_metadata::PackageId, PackageRelease>, error::FatalError> {
    let root = git::top_level(ws_meta.workspace_root.as_std_path())?;

    let member_ids = cargo::sort_workspace(ws_meta);
    member_ids
        .iter()
        .filter_map(|p| PackageRelease::load(args, &root, ws_meta, &ws_meta[p]).transpose())
        .map(|p| p.map(|p| (p.meta.id.clone(), p)))
        .collect()
}

pub fn plan(
    mut pkgs: indexmap::IndexMap<cargo_metadata::PackageId, PackageRelease>,
) -> Result<indexmap::IndexMap<cargo_metadata::PackageId, PackageRelease>, error::FatalError> {
    let mut shared_max: Option<version::Version> = None;
    let mut shared_ids = indexmap::IndexSet::new();
    for (pkg_id, pkg) in pkgs.iter() {
        if pkg.config.shared_version() {
            shared_ids.insert(pkg_id.clone());
            let planned = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
            if shared_max
                .as_ref()
                .map(|max| max.full_version < planned.full_version)
                .unwrap_or(true)
            {
                shared_max = Some(planned.clone());
            }
        }
    }
    if let Some(shared_max) = shared_max {
        for shared_id in shared_ids {
            let shared_pkg = &mut pkgs[&shared_id];
            if shared_pkg.initial_version.bare_version != shared_max.bare_version {
                shared_pkg.planned_version = Some(shared_max.clone());
            }
        }
    }

    for pkg in pkgs.values_mut() {
        pkg.plan()?;
    }

    Ok(pkgs)
}

pub struct PackageRelease {
    pub meta: cargo_metadata::Package,
    pub manifest_path: PathBuf,
    pub package_root: PathBuf,
    pub is_root: bool,
    pub config: config::Config,

    pub package_content: Vec<PathBuf>,
    pub bin: bool,
    pub dependents: Vec<Dependency>,
    pub features: cargo::Features,

    pub initial_version: version::Version,
    pub initial_tag: String,
    pub prior_tag: Option<String>,

    pub planned_version: Option<version::Version>,
    pub planned_tag: Option<String>,
}

impl PackageRelease {
    pub fn load(
        args: &config::ConfigArgs,
        git_root: &Path,
        ws_meta: &cargo_metadata::Metadata,
        pkg_meta: &cargo_metadata::Package,
    ) -> Result<Option<Self>, error::FatalError> {
        let manifest_path = pkg_meta.manifest_path.as_std_path();
        let package_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let config = config::load_package_config(args, ws_meta, pkg_meta)?;
        if !config.release() {
            log::trace!("Disabled in config, skipping {}", manifest_path.display());
            return Ok(None);
        }

        let package_content = cargo::package_content(manifest_path)?;
        let bin = pkg_meta
            .targets
            .iter()
            .flat_map(|t| t.kind.iter())
            .any(|k| k == "bin");
        let features = config.features();
        let dependents = find_dependents(ws_meta, pkg_meta)
            .map(|(pkg, dep)| Dependency {
                pkg: pkg.clone(),
                req: dep.req.clone(),
            })
            .collect();

        let is_root = git_root == package_root;
        let initial_version = version::Version::from(pkg_meta.version.clone());
        let tag_name = config.tag_name();
        let tag_prefix = config.tag_prefix(is_root);
        let name = pkg_meta.name.as_str();
        let initial_tag = render_tag(
            tag_name,
            tag_prefix,
            name,
            &initial_version,
            &initial_version,
        );

        let prior_tag = None;

        let planned_version = None;
        let planned_tag = None;

        let pkg = PackageRelease {
            meta: pkg_meta.clone(),
            manifest_path: manifest_path.to_owned(),
            package_root: package_root.to_owned(),
            is_root,
            config,

            package_content,
            bin,
            dependents,
            features,

            initial_version,
            initial_tag,
            prior_tag,

            planned_version,
            planned_tag,
        };
        Ok(Some(pkg))
    }

    pub fn set_prior_tag(&mut self, prior_tag: String) {
        self.prior_tag = Some(prior_tag);
    }

    pub fn bump(
        &mut self,
        level_or_version: &version::TargetVersion,
        metadata: Option<&str>,
    ) -> Result<(), FatalError> {
        self.planned_version =
            level_or_version.bump(&self.initial_version.full_version, metadata)?;
        Ok(())
    }

    pub fn plan(&mut self) -> Result<(), FatalError> {
        if !self.config.release() {
            return Ok(());
        }

        if self.planned_version.is_some()
            && crate::ops::git::tag_exists(&self.package_root, &self.initial_tag)?
        {
            self.prior_tag
                .get_or_insert_with(|| self.initial_tag.clone());
        }
        if self.prior_tag.is_none() {
            let tag_name = self.config.tag_name();
            let tag_prefix = self.config.tag_prefix(self.is_root);
            let name = self.meta.name.as_str();
            let tag_glob = render_tag_glob(tag_name, tag_prefix, name);
            match globset::Glob::new(&tag_glob) {
                Ok(tag_glob) => {
                    let tag_glob = tag_glob.compile_matcher();
                    self.prior_tag = crate::ops::git::find_last_tag(&self.package_root, &tag_glob);
                }
                Err(err) => {
                    log::debug!("Failed to find tag with glob `{}`: {}", tag_glob, err);
                }
            }
        }

        let base = self
            .planned_version
            .as_ref()
            .unwrap_or(&self.initial_version);
        let tag = if self.config.tag() {
            let tag_name = self.config.tag_name();
            let tag_prefix = self.config.tag_prefix(self.is_root);
            let name = self.meta.name.as_str();
            Some(render_tag(
                tag_name,
                tag_prefix,
                name,
                &self.initial_version,
                base,
            ))
        } else {
            None
        };

        self.planned_tag = tag;

        Ok(())
    }
}

fn render_tag(
    tag_name: &str,
    tag_prefix: &str,
    name: &str,
    prev: &version::Version,
    base: &version::Version,
) -> String {
    let initial_version_var = prev.bare_version_string.as_str();
    let existing_metadata_var = prev.full_version.build.as_str();
    let version_var = base.bare_version_string.as_str();
    let metadata_var = base.full_version.build.as_str();
    let mut template = Template {
        prev_version: Some(initial_version_var),
        prev_metadata: Some(existing_metadata_var),
        version: Some(version_var),
        metadata: Some(metadata_var),
        crate_name: Some(name),
        ..Default::default()
    };

    let tag_prefix = template.render(tag_prefix);
    template.prefix = Some(&tag_prefix);
    template.render(tag_name)
}

fn render_tag_glob(tag_name: &str, tag_prefix: &str, name: &str) -> String {
    let initial_version_var = "*";
    let existing_metadata_var = "*";
    let version_var = "*";
    let metadata_var = "*";
    let mut template = Template {
        prev_version: Some(initial_version_var),
        prev_metadata: Some(existing_metadata_var),
        version: Some(version_var),
        metadata: Some(metadata_var),
        crate_name: Some(name),
        ..Default::default()
    };

    let tag_prefix = template.render(tag_prefix);
    template.prefix = Some(&tag_prefix);
    template.render(tag_name)
}

fn find_dependents<'w>(
    ws_meta: &'w cargo_metadata::Metadata,
    pkg_meta: &'w cargo_metadata::Package,
) -> impl Iterator<Item = (&'w cargo_metadata::Package, &'w cargo_metadata::Dependency)> {
    ws_meta.packages.iter().filter_map(move |p| {
        if ws_meta.workspace_members.iter().any(|m| *m == p.id) {
            p.dependencies
                .iter()
                .find(|d| d.name == pkg_meta.name)
                .map(|d| (p, d))
        } else {
            None
        }
    })
}

pub struct Dependency {
    pub pkg: cargo_metadata::Package,
    pub req: semver::VersionReq,
}
