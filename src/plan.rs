use std::path::Path;
use std::path::PathBuf;

use crate::error::FatalError;
use crate::replace::Template;
use crate::version::VersionExt as _;
use crate::*;

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

    pub prev_version: version::Version,
    pub prev_tag: String,

    pub version: Option<version::Version>,
    pub tag: Option<String>,
    pub post_version: Option<version::Version>,
}

impl PackageRelease {
    pub fn load(
        args: &args::ReleaseOpt,
        git_root: &Path,
        ws_meta: &cargo_metadata::Metadata,
        pkg_meta: &cargo_metadata::Package,
    ) -> Result<Option<Self>, error::FatalError> {
        let manifest_path = pkg_meta.manifest_path.as_std_path();
        let package_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let config = config::load_package_config(&args.config, ws_meta, pkg_meta)?;
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
        let prev_version = version::Version::from(pkg_meta.version.clone());
        let prev_tag = if let Some(prev_tag) = args.prev_tag_name.as_ref() {
            // Trust the user that the tag passed in is the latest tag for the workspace and that
            // they don't care about any changes from before this tag.
            prev_tag.to_owned()
        } else {
            let prev_version_var = prev_version.bare_version_string.as_str();
            let prev_metadata_var = prev_version.full_version.build.as_str();
            let version_var = prev_version.bare_version_string.as_str();
            let metadata_var = prev_version.full_version.build.as_str();
            let mut template = Template {
                prev_version: Some(prev_version_var),
                prev_metadata: Some(prev_metadata_var),
                version: Some(version_var),
                metadata: Some(metadata_var),
                crate_name: Some(pkg_meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = config.tag_prefix(is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            template.render(config.tag_name())
        };

        let version = args
            .level_or_version
            .bump(&prev_version.full_version, args.metadata.as_deref())?;
        let tag = None;
        let post_version = None;

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

            prev_version,
            prev_tag,

            version,
            tag,
            post_version,
        };
        Ok(Some(pkg))
    }

    pub fn plan(&mut self) -> Result<(), FatalError> {
        let base = self.version.as_ref().unwrap_or(&self.prev_version);
        let tag = if self.config.tag() {
            let prev_version_var = self.prev_version.bare_version_string.as_str();
            let prev_metadata_var = self.prev_version.full_version.build.as_str();
            let version_var = base.bare_version_string.as_str();
            let metadata_var = base.full_version.build.as_str();
            let mut template = Template {
                prev_version: Some(prev_version_var),
                prev_metadata: Some(prev_metadata_var),
                version: Some(version_var),
                metadata: Some(metadata_var),
                crate_name: Some(self.meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = self.config.tag_prefix(self.is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            Some(template.render(self.config.tag_name()))
        } else {
            None
        };

        let is_pre_release = base.is_prerelease();
        let post_version = if !is_pre_release && self.config.dev_version() {
            let mut post = base.full_version.clone();
            post.increment_patch();
            post.pre = semver::Prerelease::new(self.config.dev_version_ext())?;

            Some(version::Version::from(post))
        } else {
            None
        };

        self.tag = tag;
        self.post_version = post_version;

        Ok(())
    }
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
