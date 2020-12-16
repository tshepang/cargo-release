use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::FatalError;

pub trait ConfigSource {
    fn exclude_paths(&self) -> Option<&[String]> {
        None
    }

    fn sign_commit(&self) -> Option<bool> {
        None
    }

    fn sign_tag(&self) -> Option<bool> {
        None
    }

    fn push_remote(&self) -> Option<&str> {
        None
    }

    fn registry(&self) -> Option<&str> {
        None
    }

    fn disable_publish(&self) -> Option<bool> {
        None
    }

    fn disable_push(&self) -> Option<bool> {
        None
    }

    fn dev_version_ext(&self) -> Option<&str> {
        None
    }

    fn no_dev_version(&self) -> Option<bool> {
        None
    }

    fn consolidate_commits(&self) -> Option<bool> {
        None
    }

    fn pre_release_commit_message(&self) -> Option<&str> {
        None
    }

    // depreacted
    fn pro_release_commit_message(&self) -> Option<&str> {
        None
    }

    fn post_release_commit_message(&self) -> Option<&str> {
        None
    }

    fn pre_release_replacements(&self) -> Option<&[Replace]> {
        None
    }

    fn post_release_replacements(&self) -> Option<&[Replace]> {
        None
    }

    fn pre_release_hook(&self) -> Option<&Command> {
        None
    }

    fn tag_message(&self) -> Option<&str> {
        None
    }

    fn tag_prefix(&self) -> Option<&str> {
        None
    }

    fn tag_name(&self) -> Option<&str> {
        None
    }

    fn disable_tag(&self) -> Option<bool> {
        None
    }

    fn enable_features(&self) -> Option<&[String]> {
        None
    }

    fn enable_all_features(&self) -> Option<bool> {
        None
    }

    fn dependent_version(&self) -> Option<DependentVersion> {
        None
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub exclude_paths: Option<Vec<String>>,
    pub sign_commit: Option<bool>,
    pub sign_tag: Option<bool>,
    pub push_remote: Option<String>,
    pub registry: Option<String>,
    pub disable_publish: Option<bool>,
    pub disable_push: Option<bool>,
    pub dev_version_ext: Option<String>,
    pub no_dev_version: Option<bool>,
    pub consolidate_commits: Option<bool>,
    pub pre_release_commit_message: Option<String>,
    // depreacted
    pub pro_release_commit_message: Option<String>,
    pub post_release_commit_message: Option<String>,
    pub pre_release_replacements: Option<Vec<Replace>>,
    pub post_release_replacements: Option<Vec<Replace>>,
    pub pre_release_hook: Option<Command>,
    pub tag_message: Option<String>,
    pub tag_prefix: Option<String>,
    pub tag_name: Option<String>,
    pub disable_tag: Option<bool>,
    pub enable_features: Option<Vec<String>>,
    pub enable_all_features: Option<bool>,
    pub dependent_version: Option<DependentVersion>,
}

impl Config {
    pub fn update(&mut self, source: &dyn ConfigSource) {
        if let Some(exclude_paths) = source.exclude_paths() {
            self.exclude_paths = Some(exclude_paths.to_vec());
        }
        if let Some(sign_commit) = source.sign_commit() {
            self.sign_commit = Some(sign_commit);
        }
        if let Some(sign_tag) = source.sign_tag() {
            self.sign_tag = Some(sign_tag);
        }
        if let Some(push_remote) = source.push_remote() {
            self.push_remote = Some(push_remote.to_owned());
        }
        if let Some(registry) = source.registry() {
            self.registry = Some(registry.to_owned());
        }
        if let Some(disable_publish) = source.disable_publish() {
            self.disable_publish = Some(disable_publish);
        }
        if let Some(disable_push) = source.disable_push() {
            self.disable_push = Some(disable_push);
        }
        if let Some(dev_version_ext) = source.dev_version_ext() {
            self.dev_version_ext = Some(dev_version_ext.to_owned());
        }
        if let Some(no_dev_version) = source.no_dev_version() {
            self.no_dev_version = Some(no_dev_version);
        }
        if let Some(consolidate_commits) = source.consolidate_commits() {
            self.consolidate_commits = Some(consolidate_commits);
        }
        if let Some(pre_release_commit_message) = source.pre_release_commit_message() {
            self.pre_release_commit_message = Some(pre_release_commit_message.to_owned());
        }
        // depreacted
        if let Some(pro_release_commit_message) = source.pro_release_commit_message() {
            log::warn!(
                "pro_release_commit_message is deprecated, use post-release-commit-message instead"
            );
            self.post_release_commit_message = Some(pro_release_commit_message.to_owned());
        }
        if let Some(post_release_commit_message) = source.post_release_commit_message() {
            self.post_release_commit_message = Some(post_release_commit_message.to_owned());
        }
        if let Some(pre_release_replacements) = source.pre_release_replacements() {
            self.pre_release_replacements = Some(pre_release_replacements.to_owned());
        }
        if let Some(post_release_replacements) = source.post_release_replacements() {
            self.post_release_replacements = Some(post_release_replacements.to_owned());
        }
        if let Some(pre_release_hook) = source.pre_release_hook() {
            self.pre_release_hook = Some(pre_release_hook.to_owned());
        }
        if let Some(tag_message) = source.tag_message() {
            self.tag_message = Some(tag_message.to_owned());
        }
        if let Some(tag_prefix) = source.tag_prefix() {
            self.tag_prefix = Some(tag_prefix.to_owned());
        }
        if let Some(tag_name) = source.tag_name() {
            self.tag_name = Some(tag_name.to_owned());
        }
        if let Some(disable_tag) = source.disable_tag() {
            self.disable_tag = Some(disable_tag);
        }
        if let Some(enable_features) = source.enable_features() {
            self.enable_features = Some(enable_features.to_owned());
        }
        if let Some(enable_all_features) = source.enable_all_features() {
            self.enable_all_features = Some(enable_all_features);
        }
        if let Some(dependent_version) = source.dependent_version() {
            self.dependent_version = Some(dependent_version);
        }
    }

    pub fn exclude_paths(&self) -> Option<&[String]> {
        self.exclude_paths.as_ref().map(|v| v.as_ref())
    }

    pub fn sign_commit(&self) -> bool {
        self.sign_commit.unwrap_or(false)
    }

    pub fn sign_tag(&self) -> bool {
        self.sign_tag.unwrap_or(false)
    }

    pub fn push_remote(&self) -> &str {
        self.push_remote
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("origin")
    }

    pub fn registry(&self) -> Option<&str> {
        self.registry.as_ref().map(|s| s.as_str())
    }

    pub fn disable_publish(&self) -> bool {
        self.disable_publish.unwrap_or(false)
    }

    pub fn disable_push(&self) -> bool {
        self.disable_push.unwrap_or(false)
    }

    pub fn dev_version_ext(&self) -> &str {
        self.dev_version_ext
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("alpha.0")
    }

    pub fn no_dev_version(&self) -> bool {
        self.no_dev_version.unwrap_or(false)
    }

    pub fn consolidate_commits(&self) -> bool {
        self.consolidate_commits.unwrap_or(false)
    }

    pub fn pre_release_commit_message(&self) -> &str {
        self.pre_release_commit_message
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(cargo-release) version {{version}}")
    }

    pub fn post_release_commit_message(&self) -> &str {
        self.post_release_commit_message
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(cargo-release) start next development iteration {{next_version}}")
    }

    pub fn pre_release_replacements(&self) -> &[Replace] {
        self.pre_release_replacements
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn post_release_replacements(&self) -> &[Replace] {
        self.post_release_replacements
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn pre_release_hook(&self) -> Option<&Command> {
        self.pre_release_hook.as_ref()
    }

    pub fn tag_message(&self) -> &str {
        self.tag_message
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(cargo-release) {{crate_name}} version {{version}}")
    }

    pub fn tag_prefix(&self, is_root: bool) -> &str {
        // crate_name as default tag prefix for multi-crate project
        self.tag_prefix
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| if !is_root { "{{crate_name}}-" } else { "" })
    }

    pub fn tag_name(&self) -> &str {
        self.tag_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("{{prefix}}v{{version}}")
    }

    pub fn disable_tag(&self) -> bool {
        self.disable_tag.unwrap_or(false)
    }

    pub fn enable_features(&self) -> &[String] {
        self.enable_features
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn enable_all_features(&self) -> bool {
        self.enable_all_features.unwrap_or(false)
    }

    pub fn dependent_version(&self) -> DependentVersion {
        self.dependent_version.unwrap_or_default()
    }
}

impl ConfigSource for Config {
    fn exclude_paths(&self) -> Option<&[String]> {
        self.exclude_paths.as_ref().map(|v| v.as_ref())
    }

    fn sign_commit(&self) -> Option<bool> {
        self.sign_commit
    }

    fn sign_tag(&self) -> Option<bool> {
        self.sign_tag
    }

    fn push_remote(&self) -> Option<&str> {
        self.push_remote.as_ref().map(|s| s.as_str())
    }

    fn registry(&self) -> Option<&str> {
        self.registry.as_ref().map(|s| s.as_str())
    }

    fn disable_publish(&self) -> Option<bool> {
        self.disable_publish
    }

    fn disable_push(&self) -> Option<bool> {
        self.disable_push
    }

    fn dev_version_ext(&self) -> Option<&str> {
        self.dev_version_ext.as_ref().map(|s| s.as_str())
    }

    fn no_dev_version(&self) -> Option<bool> {
        self.no_dev_version
    }

    fn consolidate_commits(&self) -> Option<bool> {
        self.consolidate_commits
    }

    fn pre_release_commit_message(&self) -> Option<&str> {
        self.pre_release_commit_message.as_ref().map(|s| s.as_str())
    }

    // deprecated
    fn pro_release_commit_message(&self) -> Option<&str> {
        self.pro_release_commit_message.as_ref().map(|s| s.as_str())
    }

    fn post_release_commit_message(&self) -> Option<&str> {
        self.post_release_commit_message
            .as_ref()
            .map(|s| s.as_str())
    }

    fn pre_release_replacements(&self) -> Option<&[Replace]> {
        self.pre_release_replacements.as_ref().map(|v| v.as_ref())
    }

    fn post_release_replacements(&self) -> Option<&[Replace]> {
        self.post_release_replacements.as_ref().map(|v| v.as_ref())
    }

    fn pre_release_hook(&self) -> Option<&Command> {
        self.pre_release_hook.as_ref()
    }

    fn tag_message(&self) -> Option<&str> {
        self.tag_message.as_ref().map(|s| s.as_str())
    }

    fn tag_prefix(&self) -> Option<&str> {
        self.tag_prefix.as_ref().map(|s| s.as_str())
    }

    fn tag_name(&self) -> Option<&str> {
        self.tag_name.as_ref().map(|s| s.as_str())
    }

    fn disable_tag(&self) -> Option<bool> {
        self.disable_tag
    }

    fn enable_features(&self) -> Option<&[String]> {
        self.enable_features.as_ref().map(|v| v.as_ref())
    }

    fn enable_all_features(&self) -> Option<bool> {
        self.enable_all_features
    }

    fn dependent_version(&self) -> Option<DependentVersion> {
        self.dependent_version
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replace {
    pub file: PathBuf,
    pub search: String,
    pub replace: String,
    pub min: Option<usize>,
    pub max: Option<usize>,
    pub exactly: Option<usize>,
    #[serde(default)]
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Command {
    Line(String),
    Args(Vec<String>),
}

impl Command {
    pub fn args(&self) -> Vec<&str> {
        match self {
            Command::Line(ref s) => vec![s.as_str()],
            Command::Args(ref a) => a.iter().map(|s| s.as_str()).collect(),
        }
    }
}

arg_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum DependentVersion {
        Upgrade,
        Fix,
        Error,
        Warn,
        Ignore,
    }
}

impl Default for DependentVersion {
    fn default() -> Self {
        DependentVersion::Fix
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoManifest {
    package: Option<CargoPackage>,
}

impl CargoManifest {
    fn into_config(self) -> Option<Config> {
        self.package.and_then(|p| p.into_config())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoPackage {
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    metadata: Option<CargoMetadata>,
}

impl CargoPackage {
    fn into_config(self) -> Option<Config> {
        if self.include.is_none() && self.exclude.is_none() && self.metadata.is_none() {
            return None;
        }
        let CargoPackage {
            include,
            exclude,
            metadata,
        } = self;
        let mut config = metadata.and_then(|m| m.release).unwrap_or_default();
        if config.exclude_paths.is_none() {
            if let Some(_include) = include {
                log::trace!("Ignoring `include` from Cargo.toml");
            } else if let Some(exclude) = exclude {
                config.exclude_paths = Some(exclude);
            }
        }
        Some(config)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoMetadata {
    release: Option<Config>,
}

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

fn get_config_from_manifest(manifest_path: &Path) -> Result<Option<Config>, FatalError> {
    if manifest_path.exists() {
        let m = load_from_file(manifest_path).map_err(FatalError::from)?;
        let c: CargoManifest = toml::from_str(&m).map_err(FatalError::from)?;
        Ok(c.into_config())
    } else {
        Ok(None)
    }
}

fn get_config_from_file(file_path: &Path) -> Result<Option<Config>, FatalError> {
    if file_path.exists() {
        let c = load_from_file(file_path).map_err(FatalError::from)?;
        let config = toml::from_str(&c).map_err(FatalError::from)?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

pub fn resolve_custom_config(file_path: &Path) -> Result<Option<Config>, FatalError> {
    get_config_from_file(file_path)
}

/// Try to resolve workspace configuration source.
///
/// This tries the following sources in order, merging the results:
/// 1. $HOME/.release.toml
/// 2. $(workspace)/release.toml
pub fn resolve_workspace_config(workspace_root: &Path) -> Result<Config, FatalError> {
    let mut config = Config::default();

    // User-local configuration from home directory.
    let home_dir = dirs_next::home_dir();
    if let Some(mut home) = home_dir {
        home.push(".release.toml");
        if let Some(cfg) = get_config_from_file(&home)? {
            config.update(&cfg);
        }
    };

    let default_config = workspace_root.join("release.toml");
    let current_dir_config = get_config_from_file(&default_config)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    Ok(config)
}

/// Try to resolve configuration source.
///
/// This tries the following sources in order, merging the results:
/// 1. $HOME/.release.toml
/// 2. $(workspace)/release.toml
/// 4. $(crate)/release.toml
/// 3. $(crate)/Cargo.toml `package.metadata.release`
///
/// `$(crate)/Cargo.toml` is a way to differentiate configuration for the root crate and the
/// workspace.
pub fn resolve_config(workspace_root: &Path, manifest_path: &Path) -> Result<Config, FatalError> {
    let mut config = Config::default();

    // User-local configuration from home directory.
    let home_dir = dirs_next::home_dir();
    if let Some(mut home) = home_dir {
        home.push(".release.toml");
        if let Some(cfg) = get_config_from_file(&home)? {
            config.update(&cfg);
        }
    };

    let crate_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    if crate_root != workspace_root {
        let default_config = workspace_root.join("release.toml");
        let current_dir_config = get_config_from_file(&default_config)?;
        if let Some(cfg) = current_dir_config {
            config.update(&cfg);
        };
    }

    // Project release file.
    let default_config = crate_root.join("release.toml");
    let current_dir_config = get_config_from_file(&default_config)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    // Crate manifest.
    let current_dir_config = get_config_from_manifest(manifest_path)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    Ok(config)
}

#[cfg(test)]
mod test {
    use super::*;

    mod resolve_config {
        use super::*;

        #[test]
        fn doesnt_panic() {
            let release_config = resolve_config(Path::new("."), Path::new("Cargo.toml")).unwrap();
            assert!(release_config.sign_commit());
        }
    }
}
