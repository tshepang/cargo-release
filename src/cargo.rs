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

pub fn publish(dry_run: bool, features: Features) -> Result<bool, FatalError> {
    match features {
        Features::None => call(vec![env!("CARGO"), "publish"], dry_run),
        Features::Selective(vec) => call(
            vec![env!("CARGO"), "publish", "--features", &vec.join(" ")],
            dry_run,
        ),
        Features::All => call(vec![env!("CARGO"), "publish", "--all-features"], dry_run),
    }
}

pub fn update(dry_run: bool) -> Result<bool, FatalError> {
    call(vec![env!("CARGO"), "update"], dry_run)
}

pub fn doc(dry_run: bool) -> Result<bool, FatalError> {
    call(vec![env!("CARGO"), "doc", "--no-deps"], dry_run)
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

pub fn parse_cargo_config() -> Result<Value, FatalError> {
    let cargo_file_path = Path::new("Cargo.toml");

    let cargo_file_content = load_from_file(&cargo_file_path).map_err(FatalError::from)?;
    cargo_file_content.parse().map_err(FatalError::from)
}

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}


#[test]
fn test_parse_cargo_config() {
    parse_cargo_config().unwrap();
}
