use semver::{Identifier, Version};
use error::FatalError;

static VERSION_ALPHA: &'static str = "alpha";
static VERSION_BETA: &'static str = "beta";
static VERSION_RC: &'static str = "rc";

pub fn is_pre_release(level: Option<&str>) -> bool {
    level
        .map(|l| l == VERSION_ALPHA || l == VERSION_BETA || l == VERSION_RC)
        .unwrap_or(false)
}

pub fn bump_version(
    version: &mut Version,
    level: Option<&str>,
    metadata: Option<&str>,
) -> Result<bool, FatalError> {
    let mut need_commit = false;
    match level {
        Some(level) => match level {
            "major" => {
                version.increment_major();
                need_commit = true;
            }
            "minor" => {
                version.increment_minor();
                need_commit = true;
            }
            "patch" => {
                if !version.is_prerelease() {
                    version.increment_patch();
                } else {
                    version.pre.clear();
                }
                need_commit = true;
            }
            "rc" => {
                version.increment_rc()?;
                need_commit = true;
            }
            "beta" => {
                version.increment_beta()?;
                need_commit = true;
            }
            "alpha" => {
                version.increment_alpha()?;
                need_commit = true;
            }
            _ => return Err(FatalError::InvalidReleaseLevel(level.to_owned())),
        },
        None => {
            if version.is_prerelease() {
                version.pre.clear();
                need_commit = true;
            }
        }
    };

    if let Some(metadata) = metadata {
        version.metadata(metadata)?;
    }

    Ok(need_commit)
}

trait VersionExt {
    fn increment_alpha(&mut self) -> Result<(), FatalError>;
    fn increment_beta(&mut self) -> Result<(), FatalError>;
    fn increment_rc(&mut self) -> Result<(), FatalError>;
    fn prerelease_id_version(&self) -> Result<Option<(String, Option<u64>)>, FatalError>;
    fn metadata(&mut self, metadata: &str) -> Result<(), FatalError>;
}

impl VersionExt for Version {
    fn prerelease_id_version(&self) -> Result<Option<(String, Option<u64>)>, FatalError> {
        if self.pre.len() > 0 {
            let e = match self.pre[0] {
                Identifier::AlphaNumeric(ref s) => s.to_owned(),
                Identifier::Numeric(_) => {
                    return Err(FatalError::UnsupportedPrereleaseVersionScheme)
                }
            };
            let v = if let Some(v) = self.pre.get(1) {
                if let Identifier::Numeric(v) = *v {
                    Some(v)
                } else {
                    return Err(FatalError::UnsupportedPrereleaseVersionScheme);
                }
            } else {
                None
            };
            Ok(Some((e, v)))
        } else {
            Ok(None)
        }
    }

    fn increment_alpha(&mut self) -> Result<(), FatalError> {
        if let Some((pre_ext, pre_ext_ver)) = try!(self.prerelease_id_version()) {
            if pre_ext == VERSION_BETA || pre_ext == VERSION_RC {
                Err(FatalError::InvalidReleaseLevel(VERSION_ALPHA.to_owned()))
            } else {
                let new_ext_ver = if pre_ext == VERSION_ALPHA {
                    pre_ext_ver.unwrap_or(0) + 1
                } else {
                    1
                };
                self.pre = vec![
                    Identifier::AlphaNumeric(VERSION_ALPHA.to_owned()),
                    Identifier::Numeric(new_ext_ver),
                ];
                Ok(())
            }
        } else {
            self.increment_patch();
            self.pre = vec![
                Identifier::AlphaNumeric(VERSION_ALPHA.to_owned()),
                Identifier::Numeric(1),
            ];
            Ok(())
        }
    }

    fn increment_beta(&mut self) -> Result<(), FatalError> {
        if let Some((pre_ext, pre_ext_ver)) = try!(self.prerelease_id_version()) {
            if pre_ext == VERSION_RC {
                Err(FatalError::InvalidReleaseLevel(VERSION_BETA.to_owned()))
            } else {
                let new_ext_ver = if pre_ext == VERSION_BETA {
                    pre_ext_ver.unwrap_or(0) + 1
                } else {
                    1
                };
                self.pre = vec![
                    Identifier::AlphaNumeric(VERSION_BETA.to_owned()),
                    Identifier::Numeric(new_ext_ver),
                ];
                Ok(())
            }
        } else {
            self.increment_patch();
            self.pre = vec![
                Identifier::AlphaNumeric(VERSION_BETA.to_owned()),
                Identifier::Numeric(1),
            ];
            Ok(())
        }
    }

    fn increment_rc(&mut self) -> Result<(), FatalError> {
        if let Some((pre_ext, pre_ext_ver)) = try!(self.prerelease_id_version()) {
            let new_ext_ver = if pre_ext == VERSION_RC {
                pre_ext_ver.unwrap_or(0) + 1
            } else {
                1
            };
            self.pre = vec![
                Identifier::AlphaNumeric(VERSION_RC.to_owned()),
                Identifier::Numeric(new_ext_ver),
            ];
            Ok(())
        } else {
            self.increment_patch();
            self.pre = vec![
                Identifier::AlphaNumeric(VERSION_RC.to_owned()),
                Identifier::Numeric(1),
            ];
            Ok(())
        }
    }

    fn metadata(&mut self, build: &str) -> Result<(), FatalError> {
        self.build = vec![Identifier::AlphaNumeric(build.to_owned())];
        Ok(())
    }
}

#[test]
fn test_increment_alpha() {
    let mut v = Version::parse("1.0.0").unwrap();
    let _ = v.increment_alpha();
    assert_eq!(v, Version::parse("1.0.1-alpha.1").unwrap());

    let mut v2 = Version::parse("1.0.1-dev").unwrap();
    let _ = v2.increment_alpha();
    assert_eq!(v2, Version::parse("1.0.1-alpha.1").unwrap());

    let mut v3 = Version::parse("1.0.1-alpha.1").unwrap();
    let _ = v3.increment_alpha();
    assert_eq!(v3, Version::parse("1.0.1-alpha.2").unwrap());

    let mut v4 = Version::parse("1.0.1-beta.1").unwrap();
    assert!(v4.increment_alpha().is_err());

    let mut v5 = Version::parse("1.0.1-1").unwrap();
    assert!(v5.increment_alpha().is_err());
}

#[test]
fn test_increment_beta() {
    let mut v = Version::parse("1.0.0").unwrap();
    let _ = v.increment_beta();
    assert_eq!(v, Version::parse("1.0.1-beta.1").unwrap());

    let mut v2 = Version::parse("1.0.1-dev").unwrap();
    let _ = v2.increment_beta();
    assert_eq!(v2, Version::parse("1.0.1-beta.1").unwrap());

    let mut v2 = Version::parse("1.0.1-alpha.1").unwrap();
    let _ = v2.increment_beta();
    assert_eq!(v2, Version::parse("1.0.1-beta.1").unwrap());

    let mut v3 = Version::parse("1.0.1-beta.1").unwrap();
    let _ = v3.increment_beta();
    assert_eq!(v3, Version::parse("1.0.1-beta.2").unwrap());

    let mut v4 = Version::parse("1.0.1-rc.1").unwrap();
    assert!(v4.increment_beta().is_err());

    let mut v5 = Version::parse("1.0.1-1").unwrap();
    assert!(v5.increment_beta().is_err());
}

#[test]
fn test_increment_rc() {
    let mut v = Version::parse("1.0.0").unwrap();
    let _ = v.increment_rc();
    assert_eq!(v, Version::parse("1.0.1-rc.1").unwrap());

    let mut v2 = Version::parse("1.0.1-dev").unwrap();
    let _ = v2.increment_rc();
    assert_eq!(v2, Version::parse("1.0.1-rc.1").unwrap());

    let mut v3 = Version::parse("1.0.1-rc.1").unwrap();
    let _ = v3.increment_rc();
    assert_eq!(v3, Version::parse("1.0.1-rc.2").unwrap());
}

#[test]
fn test_build() {
    let mut v = Version::parse("1.0.0").unwrap();
    let _ = v.metadata("git.123456");
    assert_eq!(v, Version::parse("1.0.0+git.123456").unwrap());
}
