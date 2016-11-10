use semver::Version;
use error::FatalError;

pub fn bump_version(version: &mut Version, level: Option<&str>) -> Result<bool, FatalError> {
    let mut need_commit = false;
    match level {
        Some(level) => {
            match level {
                "major" => {
                    version.increment_major();
                    need_commit = true;
                }
                "minor" => {
                    version.increment_minor();
                    need_commit = true
                }
                "patch" => {
                    if !version.is_prerelease() {
                        version.increment_patch();
                    } else {
                        version.pre.clear();
                    }
                    need_commit = true
                }
                _ => return Err(FatalError::InvalidReleaseLevel(level.to_owned())),
            }
        }
        None => {
            if version.is_prerelease() {
                version.pre.clear();
                need_commit = true;
            }
        }
    };

    Ok(need_commit)
}
