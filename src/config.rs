use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{PathBuf, Path};

use dirs;
use regex::Regex;
use semver::Version;
use toml::{self, Value};
use serde::{Deserialize, Serialize};

use error::FatalError;

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
            pro_release_commit_message: "(cargo-release) start next development iteration {{version}}".into(),
            pre_release_replacements: vec![],
            pre_release_hook: None,
            tag_message: "(cargo-release) {{crate_name}} version {{version}}".into(),
            tag_prefix: None,
            doc_commit_message: "(cargo-release) generate docs".into(),
            disable_tag: false,
            enable_features: vec![],
            enable_all_features: false,
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

fn save_to_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(&content.as_bytes())?;
    Ok(())
}

pub fn parse_cargo_config() -> Result<Value, FatalError> {
    let cargo_file_path = Path::new("Cargo.toml");

    let cargo_file_content = load_from_file(&cargo_file_path).map_err(FatalError::from)?;
    cargo_file_content.parse().map_err(FatalError::from)
}

pub fn get_config_from_manifest(manifest_path: &Path) -> Result<Option<Config>, FatalError> {
    if manifest_path.exists() {
        let m = load_from_file(manifest_path)
            .map_err(FatalError::from)?;
        let c: CargoManifest = toml::from_str(&m).map_err(FatalError::from)?;
        Ok(c.package.and_then(|p| p.metadata).and_then(|m| m.release))
    } else {
        Ok(None)
    }
}

pub fn get_config_from_file(file_path: &Path) -> Result<Option<Config>, FatalError> {
    if file_path.exists() {
        let c = load_from_file(file_path)
            .map_err(FatalError::from)?;
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
pub fn resolve_config() -> Result<Option<Config>, FatalError> {
    // Project release file.
    let current_dir_config = get_config_from_file(Path::new("release.toml"))?;
    if let Some(cfg) = current_dir_config {
        return Ok(Some(cfg));
    };

    // Crate manifest.
    let current_dir_config = get_config_from_manifest(Path::new("Cargo.toml"))?;
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

pub fn rewrite_cargo_version(version: &str) -> Result<(), FatalError> {
    {
        let file_in = File::open("Cargo.toml").map_err(FatalError::from)?;
        let mut bufreader = BufReader::new(file_in);
        let mut line = String::new();

        let mut file_out = File::create("Cargo.toml.work").map_err(FatalError::from)?;

        let section_matcher = Regex::new("^\\[.+\\]").unwrap();

        let mut in_package = false;

        loop {
            let b = bufreader.read_line(&mut line).map_err(FatalError::from)?;
            if b <= 0 {
                break;
            }

            if section_matcher.is_match(&line) {
                in_package = line.trim() == "[package]";
            }

            if in_package && line.starts_with("version") {
                line = format!("version = \"{}\"\n", version);
            }

            file_out
                .write_all(line.as_bytes())
                .map_err(FatalError::from)?;
            line.clear();
        }
    }
    fs::rename("Cargo.toml.work", "Cargo.toml")?;

    if Path::new("Cargo.lock").exists() {
        {
            let file_in = File::open("Cargo.lock").map_err(FatalError::from)?;
            let mut bufreader = BufReader::new(file_in);
            let mut line = String::new();

            let mut file_out = File::create("Cargo.lock.work").map_err(FatalError::from)?;

            let section_matcher = Regex::new("^\\[\\[.+\\]\\]").unwrap();

            let config = parse_cargo_config()?;
            let crate_name = config
                .get("package")
                .and_then(|f| f.as_table())
                .and_then(|f| f.get("name"))
                .and_then(|f| f.as_str())
                .unwrap();

            let mut in_package = false;

            loop {
                let b = bufreader.read_line(&mut line).map_err(FatalError::from)?;
                if b <= 0 {
                    break;
                }

                if section_matcher.is_match(&line) {
                    in_package = line.trim() == "[[package]]";
                }

                if in_package && line.starts_with("name") {
                    in_package = line == format!("name = \"{}\"\n", crate_name);
                }

                if in_package && line.starts_with("version") {
                    line = format!("version = \"{}\"\n", version);
                }

                file_out
                    .write_all(line.as_bytes())
                    .map_err(FatalError::from)?;
                line.clear();
            }
        }

        fs::rename("Cargo.lock.work", "Cargo.lock")?;
    }

    Ok(())
}

pub fn parse_version(version: &str) -> Result<Version, FatalError> {
    Version::parse(version).map_err(|e| FatalError::from(e))
}

#[test]
fn test_parse_cargo_config() {
    parse_cargo_config().unwrap();
}

#[test]
fn test_release_config() {
    let release_config = resolve_config().unwrap().unwrap();
    assert!(release_config.sign_commit);
}
