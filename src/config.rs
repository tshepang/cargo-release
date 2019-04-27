use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::FatalError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub sign_commit: bool,
    pub upload_doc: bool,
    pub push_remote: String,
    pub doc_branch: String,
    pub disable_publish: bool,
    pub disable_push: bool,
    pub dev_version_ext: String,
    pub no_dev_version: bool,
    pub pre_release_commit_message: String,
    pub pro_release_commit_message: String,
    pub pre_release_replacements: Vec<Replace>,
    pub pre_release_hook: Option<Command>,
    pub tag_message: String,
    pub tag_prefix: Option<String>,
    pub doc_commit_message: String,
    pub disable_tag: bool,
    pub enable_features: Vec<String>,
    pub enable_all_features: bool,
    pub dependent_version: DependentVersion,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sign_commit: false,
            upload_doc: false,
            push_remote: "origin".into(),
            doc_branch: "gh-pages".into(),
            disable_publish: false,
            disable_push: false,
            dev_version_ext: "alpha.0".into(),
            no_dev_version: false,
            pre_release_commit_message: "(cargo-release) version {{version}}".into(),
            pro_release_commit_message:
                "(cargo-release) start next development iteration {{version}}".into(),
            pre_release_replacements: vec![],
            pre_release_hook: None,
            tag_message: "(cargo-release) {{crate_name}} version {{version}}".into(),
            tag_prefix: None,
            doc_commit_message: "(cargo-release) generate docs".into(),
            disable_tag: false,
            enable_features: vec![],
            enable_all_features: false,
            dependent_version: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replace {
    pub file: PathBuf,
    pub search: String,
    pub replace: String,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoPackage {
    metadata: Option<CargoMetadata>,
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
        Ok(c.package.and_then(|p| p.metadata).and_then(|m| m.release))
    } else {
        Ok(None)
    }
}

pub fn get_config_from_file(file_path: &Path) -> Result<Option<Config>, FatalError> {
    if file_path.exists() {
        let c = load_from_file(file_path).map_err(FatalError::from)?;
        let config = toml::from_str(&c).map_err(FatalError::from)?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

/// Try to resolve configuration source.
///
/// This tries the following sources in order, short-circuiting on the first one found:
/// 1. $(pwd)/release.toml
/// 2. $(pwd)/Cargo.toml `package.metadata.release` (with deprecation warning)
/// 3. $HOME/.release.toml
///
pub fn resolve_config(manifest_path: &Path) -> Result<Option<Config>, FatalError> {
    // Project release file.
    let default_config = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("release.toml");
    let current_dir_config = get_config_from_file(&default_config)?;
    if let Some(cfg) = current_dir_config {
        return Ok(Some(cfg));
    };

    // Crate manifest.
    let current_dir_config = get_config_from_manifest(manifest_path)?;
    if let Some(cfg) = current_dir_config {
        return Ok(Some(cfg));
    };

    // User-local configuration from home directory.
    let home_dir = dirs::home_dir();
    if let Some(mut home) = home_dir {
        home.push(".release.toml");
        return get_config_from_file(home.as_path());
    };

    // No project-wide configuration.
    Ok(None)
}

#[cfg(test)]
mod test {
    use super::*;

    mod resolve_config {
        use super::*;

        #[test]
        fn doesnt_panic() {
            let release_config = resolve_config(Path::new("Cargo.toml")).unwrap().unwrap();
            assert!(release_config.sign_commit);
        }
    }
}
