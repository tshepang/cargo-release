use regex::Error as RegexError;
use semver::SemVerError;
use std::env::VarError as VarError;
use std::io::Error as IOError;
use std::string::FromUtf8Error;
use toml::de::Error as TomlError;
use toml_edit::TomlError as TomlEditError;
use cargo_metadata::Error as CargoMetaError;

quick_error! {
    #[derive(Debug)]
    pub enum FatalError {
        IOError(err: IOError) {
            from()
            cause(err)
            description(err.description())
            display("IO Error: {}", err)
        }
        InvalidCargoFileFormat(err: TomlError) {
            display("Invalid TOML file format: {}", err)
            description(err.description())
            from()
            cause(err)
        }
        InvalidCargoFileFormat2(err: TomlEditError) {
            display("Invalid TOML file format: {}", err)
            description(err.description())
            from()
            cause(err)
        }
        InvalidCargoFileFormat3(err: CargoMetaError) {
            display("Invalid TOML file format: {}", err)
            description(err.description())
            from()
            cause(err)
        }
        InvalidCargoConfigKeys {
            display("Invalid cargo-release config item found")
            description("Invalid cargo-release config item found")
        }
        SemVerError(err: SemVerError) {
            from()
            cause(err)
            display("SemVerError {}", err)
            description(err.description())
        }
        FromUtf8Error(err: FromUtf8Error) {
            from()
            cause(err)
            display("FromUtf8Error {}", err)
            description(err.description())
        }
        InvalidReleaseLevel(level: String) {
            display("Unsupported release level {}", level)
            description("Unsupported release level, only major, minor and patch are supported")
        }
        UnsupportedPrereleaseVersionScheme {
            display("This version scheme is not supported by cargo-release.")
            description("This version scheme is not supported by cargo-release. Use format like `pre`, `dev` or `alpha.1` for prerelease symbol")
        }
        ReplacerConfigError {
            display("Insuffient replacer config: file, search and replace are required.")
            description("Insuffient replacer config: file, search and replace are required.")
        }
        ReplacerRegexError(err: RegexError) {
            from()
            cause(err)
            display("RegexError {}", err)
            description(err.description())
        }
        VarError(err: VarError) {
            from()
            cause(err)
            description(err.description())
            display("Environment Variable Error: {}", err)
        }
    }
}
