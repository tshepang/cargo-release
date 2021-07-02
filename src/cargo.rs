use std::env;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::path::Path;

use toml::Value;

use crate::cmd::call;
use crate::error::FatalError;
use crate::Features;

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

pub fn publish(
    dry_run: bool,
    manifest_path: &Path,
    features: &Features,
    registry: Option<&str>,
    token: Option<&str>,
) -> Result<bool, FatalError> {
    let cargo = cargo();

    let mut command: Vec<&str> = vec![
        &cargo,
        "publish",
        "--manifest-path",
        manifest_path.to_str().unwrap(),
    ];

    if let Some(registry) = registry {
        command.push("--registry");
        command.push(registry);
    }

    if let Some(token) = token {
        command.push("--token");
        command.push(token);
    }

    let feature_arg;
    match features {
        Features::None => (),
        Features::Selective(vec) => {
            feature_arg = vec.join(" ");
            command.push("--features");
            command.push(&feature_arg);
        }
        Features::All => {
            command.push("--all-features");
        }
    };

    call(command, dry_run)
}

pub fn wait_for_publish(
    name: &str,
    version: &str,
    timeout: std::time::Duration,
    dry_run: bool,
) -> Result<(), FatalError> {
    if !dry_run {
        let now = std::time::Instant::now();
        let sleep_time = std::time::Duration::from_secs(1);
        let index = crates_index::Index::new_cargo_default();
        let mut logged = false;
        loop {
            if let Err(e) = index.update() {
                log::debug!("Crate index update failed with {}", e);
            }
            let crate_data = index.crate_(name);
            let published = crate_data
                .iter()
                .flat_map(|c| c.versions().iter())
                .any(|v| v.version() == version);

            if published {
                break;
            } else if timeout < now.elapsed() {
                return Err(FatalError::PublishTimeoutError);
            }

            if !logged {
                log::info!("Waiting for publish to complete...");
                logged = true;
            }
            std::thread::sleep(sleep_time);
        }
    }

    Ok(())
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
        file_out
            .write(manifest.to_string_in_original_order().as_bytes())
            .map_err(FatalError::from)?;
    }
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

pub fn set_dependency_version(
    manifest_path: &Path,
    name: &str,
    version: &str,
) -> Result<(), FatalError> {
    let temp_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");

    {
        let manifest = load_from_file(manifest_path)?;
        let mut manifest: toml_edit::Document = manifest.parse().map_err(FatalError::from)?;

        let dep_table_names = &["dependencies", "dev-dependencies", "build-dependencies"];
        for key in dep_table_names {
            if let Some(deps_table) = manifest
                .as_table_mut()
                .get_mut(key)
                .and_then(|i| i.as_table_mut())
            {
                set_version(deps_table, name, version)?;
            }
        }

        if let Some(target_table) = manifest
            .as_table_mut()
            .get_mut("target")
            .and_then(|i| i.as_table_mut())
        {
            for (_target_name, target_specific_item) in target_table.iter_mut() {
                for key in dep_table_names {
                    if let Some(deps_table) = target_specific_item
                        .as_table_mut()
                        .and_then(|t| t.get_mut(key))
                        .and_then(|i| i.as_table_mut())
                    {
                        set_version(deps_table, name, version)?;
                    }
                }
            }
        }

        let mut file_out = File::create(&temp_manifest_path).map_err(FatalError::from)?;
        file_out
            .write(manifest.to_string_in_original_order().as_bytes())
            .map_err(FatalError::from)?;
    }
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

fn set_version(
    deps_table: &mut toml_edit::Table,
    name: &str,
    version: &str,
) -> Result<(), FatalError> {
    let dep_item = match deps_table.get_mut(name) {
        Some(item) => item,
        None => {
            return Ok(());
        }
    };
    if dep_item.is_table_like() {
        dep_item["version"] = toml_edit::value(version);
    } else {
        return Err(FatalError::InvalidCargoFileFormat(
            "Intra-workspace dependencies should use both version and path".into(),
        ));
    }

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
    let cargo_file_content = load_from_file(manifest_path).map_err(FatalError::from)?;
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
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
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
        fn preserve_table_order() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "1.0", path = "../" }

    [package.metadata.release]
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::similar(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "2.0", path = "../" }

    [package.metadata.release]
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::similar(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn dev_dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]

    [dev-dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::similar(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]

    [dev-dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn build_dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dev-dependencies]

    [build-dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::similar(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dev-dependencies]

    [build-dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn all_dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "1.0", path = "../" }

    [build-dependencies]
    foo = { version = "1.0", path = "../" }

    [dev-dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::similar(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "2.0", path = "../" }

    [build-dependencies]
    foo = { version = "2.0", path = "../" }

    [dev-dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn no_path() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = "1.0"
    "#,
                )
                .unwrap();

            let err = set_dependency_version(manifest_path.path(), "foo", "2.0");
            assert!(err.is_err());

            temp.close().unwrap();
        }

        #[test]
        fn out_of_line_table() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies.foo]
    version = "1.0"
    path = "../"
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::similar(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies.foo]
    version = "2.0"
    path = "../"
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }
    }

    mod update_lock {
        use super::*;

        #[test]
        fn in_pkg() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new(
                "tests/fixtures/simple/Cargo.lock",
            )));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(
                predicate::path::eq_file(Path::new("tests/fixtures/simple/Cargo.lock")).not(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn in_pure_workspace() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/pure_ws", &["**"]).unwrap();
            let manifest_path = temp.child("b/Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new(
                "tests/fixtures/pure_ws/Cargo.lock",
            )));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(
                predicate::path::eq_file(Path::new("tests/fixtures/pure_ws/Cargo.lock")).not(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn in_mixed_workspace() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/mixed_ws", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new(
                "tests/fixtures/mixed_ws/Cargo.lock",
            )));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(
                predicate::path::eq_file(Path::new("tests/fixtures/mixed_ws/Cargo.lock")).not(),
            );

            temp.close().unwrap();
        }
    }
}
