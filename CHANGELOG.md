# Change Log

## [Unreleased] - ReleaseDate

### Added

* Notify users on unchanged crates when releasing workspace #148
* Strict check on replacements #187
* Trace replacement diff on dry-run #171
* Allow workspace release commits to be consolidated #181
* Releasing specific version

### Changed

* Renamed option "pro-release-commit-message" to
  "post-release-commit-message" #140
* Use logging for output #152
* Fixed issue when crate.io didn't update in time that causing
  workspace release failed #183

### Removed

* Doc upload removed #176

## [0.12.4] - 2019-08-03

### Changed

* Fixed commit message after release #136

## [0.12.3] - 2019-07-28

### Changed

* Only update dependents when needed #135

## [0.12.2] - 2019-07-24

### Changed

* Fixed issue when updating dependency version in workspace #130

## [0.12.1] - 2019-07-18

### Changed

* Fixed serde version as 1.0.95 was yanked

## [0.12.0] - 2019-07-17

### Added

* Workspace support #66
* Layered config support #111
* Support for tag name customization #125

### Changed

* Using `v` as default version tag prefix #127
* Improved cargo binary detection #88 #89
* Doc update
