use std::env;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::path::Path;

use cargo_metadata;
use toml::Value;
use toml_edit;

use cmd::call;
use error::FatalError;
use Features;

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
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

pub fn set_package_version(manifest_path: &Path, version: &str) -> Result<(), FatalError> {
    let temp_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");

    {
        let manifest = load_from_file(manifest_path)?;
        let mut manifest: toml_edit::Document = manifest.parse().map_err(FatalError::from)?;
        manifest["package"]["version"] = toml_edit::value(version);

        let mut file_out = File::create(&temp_manifest_path).map_err(FatalError::from)?;
        file_out.write(manifest.to_string().as_bytes())
                .map_err(FatalError::from)?;
    }
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

pub fn set_dependency_version(manifest_path: &Path, name: &str, version: &str) -> Result<(), FatalError> {
    let temp_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");

    {
        let manifest = load_from_file(manifest_path)?;
        let mut manifest: toml_edit::Document = manifest.parse().map_err(FatalError::from)?;
        manifest["dependencies"][name]["version"] = toml_edit::value(version);

        let mut file_out = File::create(&temp_manifest_path).map_err(FatalError::from)?;
        file_out.write(manifest.to_string().as_bytes())
                .map_err(FatalError::from)?;
    }
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

pub fn update_lock(manifest_path: &Path) -> Result<(), FatalError> {
    cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(FatalError::from)?;

    Ok(())
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

    #[allow(unused_imports)] // Not being detected
    use assert_fs::prelude::*;
    use assert_fs;
    use predicates::prelude::*;

    mod parse_cargo_config {
        use super::*;

        #[test]
        fn doesnt_panic() {
            parse_cargo_config(Path::new("Cargo.toml")).unwrap();
        }
    }

    mod set_package_version {
        use super::*;

        #[test]
        fn succeeds() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["*"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");

            let meta = cargo_metadata::MetadataCommand::new()
                .manifest_path(manifest_path.path())
                .exec()
                .unwrap();
            assert_eq!(meta.packages[0].version.to_string(), "0.1.0");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();

            let meta = cargo_metadata::MetadataCommand::new()
                .manifest_path(manifest_path.path())
                .exec()
                .unwrap();
            assert_eq!(meta.packages[0].version.to_string(), "2.0.0");

            temp.close().unwrap();
        }
    }

    mod set_dependency_version {
        use super::*;

        #[test]
        fn succeeds() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["*"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path.write_str(r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "1.0", path = "../" }
    "#).unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(predicate::str::similar(r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "2.0", path = "../" }
    "#).from_utf8().from_file_path());

            temp.close().unwrap();
        }
    }

    mod update_lock {
        use super::*;

        #[test]
        fn in_pkg() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["*"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new("tests/fixtures/simple/Cargo.lock")));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new("tests/fixtures/simple/Cargo.lock")).not());

            temp.close().unwrap();
        }

        #[test]
        fn in_pure_workspace() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/pure_ws", &["*"]).unwrap();
            let manifest_path = temp.child("b/Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new("tests/fixtures/pure_ws/Cargo.lock")));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new("tests/fixtures/pure_ws/Cargo.lock")).not());

            temp.close().unwrap();
        }

        #[test]
        fn in_mixed_workspace() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/mixed_ws", &["*"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new("tests/fixtures/mixed_ws/Cargo.lock")));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new("tests/fixtures/mixed_ws/Cargo.lock")).not());

            temp.close().unwrap();
        }
    }
}
