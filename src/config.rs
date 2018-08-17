use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

use regex::Regex;
use semver::Version;
use toml::value::Table;
use toml::{self, Value};
use dirs;

use error::FatalError;

pub static SIGN_COMMIT: &'static str = "sign-commit";
pub static UPLOAD_DOC: &'static str = "upload-doc";
pub static PUSH_REMOTE: &'static str = "push-remote";
pub static DOC_BRANCH: &'static str = "doc-branch";
pub static DISABLE_PUBLISH: &'static str = "disable-publish";
pub static DISABLE_PUSH: &'static str = "disable-push";
pub static DEV_VERSION_EXT: &'static str = "dev-version-ext";
pub static NO_DEV_VERSION: &'static str = "no-dev-version";
pub static PRE_RELEASE_COMMIT_MESSAGE: &'static str = "pre-release-commit-message";
pub static PRO_RELEASE_COMMIT_MESSAGE: &'static str = "pro-release-commit-message";
pub static PRE_RELEASE_REPLACEMENTS: &'static str = "pre-release-replacements";
pub static PRE_RELEASE_HOOK: &'static str = "pre-release-hook";
pub static TAG_MESSAGE: &'static str = "tag-message";
pub static TAG_PREFIX: &'static str = "tag-prefix";
pub static DOC_COMMIT_MESSAGE: &'static str = "doc-commit-message";

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = try!(File::open(path));
    let mut s = String::new();
    try!(file.read_to_string(&mut s));
    Ok(s)
}

fn save_to_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = try!(File::create(path));
    try!(file.write_all(&content.as_bytes()));
    Ok(())
}

pub fn parse_cargo_config() -> Result<Value, FatalError> {
    let cargo_file_path = Path::new("Cargo.toml");

    let cargo_file_content = try!(load_from_file(&cargo_file_path).map_err(FatalError::from));
    cargo_file_content.parse().map_err(FatalError::from)
}

fn get_release_config_table_from_cargo<'a>(cargo_config: &'a Value) -> Option<&'a Table> {
    cargo_config
        .get("package")
        .and_then(|f| f.as_table())
        .and_then(|f| f.get("metadata"))
        .and_then(|f| f.as_table())
        .and_then(|f| f.get("release"))
        .and_then(|f| f.as_table())
}

pub fn get_release_config<'a>(config: Option<&'a Table>, key: &str) -> Option<&'a Value> {
    config.and_then(|c| c.get(key))
}

pub fn get_release_config_table_from_file(file_path: &Path) -> Result<Option<Table>, FatalError> {
    if file_path.exists() {
        load_from_file(file_path)
            .map_err(FatalError::from)
            .and_then(|c| c.parse::<Value>().map_err(FatalError::from))
            .map(|v| v.as_table().map(|t| t.clone()))
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
pub fn resolve_release_config_table(cargo_config: &Value) -> Result<Option<Table>, FatalError> {
    // Project release file.
    let current_dir_config = get_release_config_table_from_file(Path::new("release.toml"))?;
    if let Some(cfg) = current_dir_config {
        return Ok(Some(cfg));
    };

    // Crate manifest.
    let cargo_file_config = get_release_config_table_from_cargo(cargo_config);
    if let Some(cfg) = cargo_file_config.cloned() {
        return Ok(Some(cfg));
    };

    // User-local configuration from home directory.
    let home_dir = dirs::home_dir();
    if let Some(mut home) = home_dir {
        home.push(".release.toml");
        return get_release_config_table_from_file(home.as_path());
    };

    // No project-wide configuration.
    Ok(None)
}

pub fn verify_release_config(config: &Table) -> Option<Vec<&str>> {
    let valid_keys = vec![
        SIGN_COMMIT,
        UPLOAD_DOC,
        PUSH_REMOTE,
        DOC_BRANCH,
        DISABLE_PUBLISH,
        DISABLE_PUSH,
        DEV_VERSION_EXT,
        NO_DEV_VERSION,
        PRE_RELEASE_COMMIT_MESSAGE,
        PRO_RELEASE_COMMIT_MESSAGE,
        PRE_RELEASE_REPLACEMENTS,
        PRE_RELEASE_HOOK,
        TAG_MESSAGE,
        TAG_PREFIX,
        DOC_COMMIT_MESSAGE,
    ];
    let mut invalid_keys = Vec::new();
    for i in config.keys() {
        if !valid_keys.contains(&i.as_ref()) {
            invalid_keys.push(i.as_ref());
        }
    }
    if invalid_keys.is_empty() {
        None
    } else {
        Some(invalid_keys)
    }
}

pub fn save_cargo_config(config: &Value) -> Result<(), FatalError> {
    let cargo_file_path = Path::new("Cargo.toml");

    let serialized_data = toml::to_string(config).unwrap();

    try!(save_to_file(&cargo_file_path, &serialized_data).map_err(FatalError::from));
    Ok(())
}

pub fn rewrite_cargo_version(version: &str) -> Result<(), FatalError> {
    {
        let file_in = try!(File::open("Cargo.toml").map_err(FatalError::from));
        let mut bufreader = BufReader::new(file_in);
        let mut line = String::new();

        let mut file_out = try!(File::create("Cargo.toml.work").map_err(FatalError::from));

        let section_matcher = Regex::new("^\\[.+\\]").unwrap();

        let mut in_package = false;

        loop {
            let b = try!(bufreader.read_line(&mut line).map_err(FatalError::from));
            if b <= 0 {
                break;
            }

            if section_matcher.is_match(&line) {
                in_package = line.trim() == "[package]";
            }

            if in_package && line.starts_with("version") {
                line = format!("version = \"{}\"\n", version);
            }

            try!(
                file_out
                    .write_all(line.as_bytes())
                    .map_err(FatalError::from)
            );
            line.clear();
        }
    }
    try!(fs::rename("Cargo.toml.work", "Cargo.toml"));

    if Path::new("Cargo.lock").exists() {
        let file_in = try!(File::open("Cargo.lock").map_err(FatalError::from));
        let mut bufreader = BufReader::new(file_in);
        let mut line = String::new();

        let mut file_out = try!(File::create("Cargo.lock.work").map_err(FatalError::from));

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
            let b = try!(bufreader.read_line(&mut line).map_err(FatalError::from));
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

            try!(
                file_out
                    .write_all(line.as_bytes())
                    .map_err(FatalError::from)
            );
            line.clear();
        }
        try!(fs::rename("Cargo.lock.work", "Cargo.lock"));
    }

    Ok(())
}

pub fn parse_version(version: &str) -> Result<Version, FatalError> {
    Version::parse(version).map_err(|e| FatalError::from(e))
}

#[test]
fn test_release_config() {
    if let Ok(cargo_file) = parse_cargo_config() {
        let release_config = resolve_release_config_table(&cargo_file).unwrap();
        assert!(
            get_release_config(release_config.as_ref(), "sign-commit")
                .and_then(|f| f.as_bool())
                .unwrap_or(false)
        );
    } else {
        panic!("paser cargo file failed");
    }
}
