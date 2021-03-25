# cargo release

[![](http://meritbadge.herokuapp.com/cargo-release)](https://crates.io/crates/cargo-release)
[![Build Status](https://travis-ci.org/sunng87/cargo-release.svg?branch=master)](https://travis-ci.org/sunng87/cargo-release)
[![Donate](https://img.shields.io/badge/donate-liberapay-yellow.svg)](https://liberapay.com/Sunng/donate)

Performs release best-practices, including:

1. Ensure the git working directory is clean.
2. Bump the version in Cargo.toml
3. Create a git tag for this version
4. Run `cargo publish` ([if not disabled](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish--field-optional))
5. Bump version for next development cycle
6. `git push`

Features for workspaces include:
- Report which crates might be able to be skipped
- Update version ranges in dependent crates
- Optionally using a single commit for all version bumps

## Install

Current release: 0.13.11

`cargo install cargo-release`

## Usage

`cargo release [level]`

* See the [reference](docs/reference.md) for more on `level`, other CLI
  arguments, and configuration file format.
* See also the [FAQ](docs/faq.md) for help in figuring out how to adapt
  cargo-release to your workflow.

### Prerequisite

* Your project should be managed by git.

### Dry run

We recommend calling `cargo release --dry-run -vv` with your custom options before
actually executing it. The dry-run mode with verbose output will print all commands to
execute during the release process. And you will get an overview of what's going on.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
  at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

### Donation

I'm now accepting donation on [liberapay](https://liberapay.com/Sunng/donate),
if you find my work helpful and want to keep it going.
