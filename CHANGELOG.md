# Change Log

<!-- next-header -->
## [Unreleased] - ReleaseDate

### Fixed

- Don't error out when excluding workspace packages that would be downgraded by the current run

## [0.18.6] - 2021-12-08

### Fixed

- Consider optional dependencies when evaluating release order

## [0.18.5] - 2021-11-16

### Features

- Warn users when skipping crates in a workspace that have no changes

### Fixed

- Use static crt on Windows
- Tweak log levels to avoid needing to show all traces to see whats happening

## [0.18.4] - 2021-10-30

### Fixed

- Vendor libgit2 for a more consistent experience

## [0.18.3] - 2021-10-26

### Fixed

- Fix bug where we ignored `push` config

## [0.18.2] - 2021-10-11

### Fixed

- Crash when setting `publish = false` in `Cargo.toml` **and**` in a config file / commandline

## [0.18.1] - 2021-10-09

### Fixed

- Introduced `cargo publish` fix for workspaces that was meant to be in before 0.18

## [0.18.0] - 2021-10-08

### Breaking Changes

`dev-version` is now disabled by default.  This to encourage people to not use
it as it makes it harder for dependent crates to `[patch]` in a version from
git.

### Features

- Opt-in shared crate version.  For now, it just errors on mismatch.  This lets you reference the version in consolidated commits.

### Fixed

- In dev-version commit, render `{{next_version}}`
- Disable always-sleep after publish.  We believe the underlying problem
  preventing us from detecting the crate from being released is fixed.  If you
  run into problems, you can set the env variable `PUBLISH_GRACE_SLEEP`.
- Let packages override consolidated actions
- Switched command-line to more common `no-` prefixes
- Added positive and negative version of each command-line flag to allow overriding the config, and not just defaults.
- Added positive versions of each negative (`disable_`, `no_`) config field
- **Deprecated** all negative (`disable_`, `no_`) config fields

## [0.17.1] - 2021-08-24

### Fixed

- Correctly detect changes for crates outside of the root

## [0.17.0] - 2021-08-23

### Features

- Support `~/.config/cargo-release/release.toml`
- Run `cargo publish` during dry-runs to help catch publish-specific errors
- Add `allow-branch` config setting to limit what branches a release can happen from
- Support `Cargo.toml`s `workspace.metadata`

### Fixed

- In theory, finally fixed it so we properly wait between publishing of crates in a workspace
- Don't warn a user about releasing a crate without changes if a dependency changed
- Notify for all `[[bin]]` crates on `Cargo.lock` change, rather than just the root crate
- Made clearer what are fatal errors during dry-run (since dry-run doesn't stop for them)
- Gracefully handle path-only dependencies which are especially important for cycles.
- Correctly update dependents on post-release version bump.
- Log what was dirty about a repo to make it easier for people to report problems
- Allow pushing even when there isn't a tracking branch
- Specifying `--package` should switch us to opt-in

### Breaking Changes

- `--dry-run` is now the default.   Pass `--execute` to perform the release.
- `exclude-paths` config setting was removed; we now rely on `cargo package --list` to know which files to check for changes.

## [0.16.3] - 2021-08-01

## [0.16.2] - 2021-07-15

### Fixed

* Respect `disable_push` flag at package level.

## [0.16.1] - 2021-07-04

### Fixed

* Submodule operation dir issue

## [0.16.0] - 2021-07-03

### Added

* Git dirty check for submodules

### Changed

* Prior sharing of pushes between workspace crates is now behind the flag `consolidate-pushes`

### Fixed

* Avoid panic on invalid Cargo.toml entry

## [0.15.1] - 2021-06-24

### Fixed

* Fixed issue where the versions of cfg specific dependencies wouldn't be properly bumped.

## [0.15.0] - 2021-06-19

### Added

* `push-options` to send flags to the server, on push.  Current limitations include:
  * Only on branch and not tag push
  * Operates at the workspace level
  * No placeholders are supported

### Changed

* `disable-push`, `push-remote` now only apply at the workspace level, when in a workspace.
* tags are pushed before branch as requested in #250

## [0.14.0] - 2021-06-16

### Added

* Add `PUBLISH_GRACE_SLEEP` environment variable that allows to set the number of seconds to sleep between
  two invocations of `cargo publish`. Default is `5`
* Do not sleep between publishes on dry runs

### Changed

* New `disable-release` config flag to skip crates in a workspace
* Warn on detached HEADs and being behind the remote
  * **Note:** This means we are now doing a `git fetch` at the beginning, even with `--dry-run`

## [0.13.11] - 2021-03-25

## [0.13.10] - 2020-12-28

### Changed

* Dependencies updated
* Add sleep between publish [#247]

## [0.13.9] - 2020-11-29

### Changed

* Improved diff view in dry-run mode

## [0.13.8] - 2020-09-29

### Added

* New option `post-release-replacements` support [#228]

## [0.13.7] - 2020-09-27

### Changed

* Upgraded crate-index and minimum rust version [#227]

## [0.13.6] - 2020-08-31

### Fixed

* `min`/`max` bug with replacement settings [#225]
* Better error message for IOError [#226]

## [0.13.5] - 2020-07-04

### Added

* Added new option `sign-tag` for tag signing only

## [0.13.4] - 2020-05-10

### Changed

* Changed default timeout on waiting crate to land on crates.io [#207]
* Changed change detection log to debug [#208]

## [0.13.3] - 2020-03-13

### Added

* Ability to upload to alternate registries (though wait-for-publish
  is skipped) [#203]

### Fixed

* Prerelease check for replacement

## [0.13.1] - 2020-03-01

### Added

* Config: `exclude_paths` list of globs to get more accurate listing of files-changed [#149]
* CLI: `--token` can be used to specify the token used by `cargo publish`

### Fixed

* Take 2 on waiting for a crate to be published before publishing the next one [#194]

## [0.13.0] - 2019-12-09

### Added

* Notify users on unchanged crates when releasing workspace [#148]
* Strict check on replacements [#187]
* Trace replacement diff on dry-run [#171]
* Allow workspace release commits to be consolidated [#181]
* Releasing specific version [#191]
* `tag_name` is now available in replacements and can be useful for
  changelog generation in multi-crate workspace [#168]

### Changed

* Renamed option "pro-release-commit-message" to
  "post-release-commit-message" [#140]
* Use logging for output [#152]
* Also check untracked files in initial dirty check [#146]
* `[package.metadata.release]` in `$CRATE/Cargo.toml` now has a higher
  priority than `$CRATE/release.toml` [7cc9890] [#181]
* Confirmation is prompted for even when there is no version bump
  [47bf645] [#175]

### Fixed

* Fixed issue when crate.io didn't update in time that causing
  workspace release failed [#183]

### Removed

* Doc upload removed as the community has moved to [docs.rs](https://docs.rs) [#176]

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

<!-- next-url -->
[Unreleased]: https://github.com/crate-ci/cargo-release/compare/v0.18.6...HEAD
[0.18.6]: https://github.com/crate-ci/cargo-release/compare/v0.18.5...v0.18.6
[0.18.5]: https://github.com/crate-ci/cargo-release/compare/v0.18.4...v0.18.5
[0.18.4]: https://github.com/crate-ci/cargo-release/compare/v0.18.3...v0.18.4
[0.18.3]: https://github.com/crate-ci/cargo-release/compare/v0.18.2...v0.18.3
[0.18.2]: https://github.com/crate-ci/cargo-release/compare/v0.18.1...v0.18.2
[0.18.1]: https://github.com/crate-ci/cargo-release/compare/v0.18.0...v0.18.1
[0.18.0]: https://github.com/crate-ci/cargo-release/compare/v0.17.1...v0.18.0
[0.17.1]: https://github.com/crate-ci/cargo-release/compare/v0.17.0...v0.17.1
[0.17.0]: https://github.com/crate-ci/cargo-release/compare/v0.16.6...v0.17.0
[0.16.6]: https://github.com/crate-ci/cargo-release/compare/v0.16.5...v0.16.6
[0.16.5]: https://github.com/crate-ci/cargo-release/compare/v0.16.4...v0.16.5
[0.16.4]: https://github.com/crate-ci/cargo-release/compare/v0.16.3...v0.16.4
