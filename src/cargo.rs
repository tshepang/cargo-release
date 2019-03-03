use std::env;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

use regex::Regex;
use semver::Version;
use toml::Value;

use cmd::call;
use error::FatalError;
use Features;

fn cargo() -> String {
    env::var("CARGO").unwrap_or("cargo".to_owned())
}

pub fn publish(dry_run: bool, manifest_path: &Path, features: Features) -> Result<bool, FatalError> {
    let cargo = cargo();
    match features {
        Features::None => call(
            vec![
                &cargo,
                "publish",
                "--manifest-path",
                manifest_path.to_str().unwrap(),
            ],
            dry_run
        ),
        Features::Selective(vec) => call(
            vec![
                &cargo,
                "publish",
                "--features",
                &vec.join(" "),
                "--manifest-path",
                manifest_path.to_str().unwrap(),
            ],
            dry_run,
        ),
        Features::All => call(
            vec![
                &cargo,
                "publish",
                "--all-features",
                "--manifest-path",
                manifest_path.to_str().unwrap(),
            ],
            dry_run,
        ),
    }
}

pub fn doc(dry_run: bool, manifest_path: &Path) -> Result<bool, FatalError> {
    let cargo = cargo();
    call(vec![
        &cargo,
        "doc",
        "--no-deps",
        "--manifest-path",
        manifest_path.to_str().unwrap(),
    ], dry_run)
}

pub fn set_manifest_version(manifest_path: &Path, version: &str) -> Result<(), FatalError> {
    let temp_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");

    {
        let file_in = File::open(manifest_path).map_err(FatalError::from)?;
        let mut bufreader = BufReader::new(file_in);
        let mut line = String::new();

        let mut file_out = File::create(&temp_manifest_path).map_err(FatalError::from)?;

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
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

pub fn set_lock_version(lock_path: &Path, crate_name: &str, version: &str) -> Result<(), FatalError> {
    let temp_lock_path = lock_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.lock.work");

    {
        let file_in = File::open(lock_path).map_err(FatalError::from)?;
        let mut bufreader = BufReader::new(file_in);
        let mut line = String::new();

        let mut file_out = File::create(&temp_lock_path).map_err(FatalError::from)?;

        let section_matcher = Regex::new("^\\[\\[.+\\]\\]").unwrap();

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

    fs::rename(temp_lock_path, lock_path)?;

    Ok(())
}

pub fn parse_version(version: &str) -> Result<Version, FatalError> {
    Version::parse(version).map_err(|e| FatalError::from(e))
}

pub fn parse_cargo_config(manifest_path: &Path) -> Result<Value, FatalError> {
    let cargo_file_content = load_from_file(&manifest_path).map_err(FatalError::from)?;
    cargo_file_content.parse().map_err(FatalError::from)
}

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

#[cfg(test)]
mod test {
    use super::*;

    use assert_fs::prelude::*;
    use assert_fs;
    use cargo_metadata;

    #[test]
    fn test_parse_cargo_config() {
        parse_cargo_config(Path::new("Cargo.toml")).unwrap();
    }

    #[test]
    fn test_set_manifest_version() {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.copy_from("tests/fixtures/simple", &["*"]).unwrap();
        let manifest_path = temp.child("Cargo.toml");

        let meta = cargo_metadata::MetadataCommand::new()
            .manifest_path(manifest_path.path())
            .exec()
            .unwrap();
        assert_eq!(meta.packages[0].version.to_string(), "0.1.0");

        set_manifest_version(manifest_path.path(), "2.0.0").unwrap();

        let meta = cargo_metadata::MetadataCommand::new()
            .manifest_path(manifest_path.path())
            .exec()
            .unwrap();
        assert_eq!(meta.packages[0].version.to_string(), "2.0.0");

        temp.close().unwrap();
    }
}
