# cargo release

[![](http://meritbadge.herokuapp.com/cargo-release)](https://crates.io/crates/cargo-release)
[![Build Status](https://travis-ci.org/sunng87/cargo-release.svg?branch=master)](https://travis-ci.org/sunng87/cargo-release)
[![Donate](https://img.shields.io/badge/donate-liberapay-yellow.svg)](https://liberapay.com/Sunng/donate)

This a script standardize release process of cargo project for you.

Basically it runs following tasks:

* Check if current working directory is git clean
* Read version from Cargo.toml, remove pre-release extension, bump
  version and commit if necessary
* Run `cargo publish` ([if not disabled](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish--field-optional))
* Generate rustdoc and push to gh-pages optionally
* Create a git tag for this version
* Bump version for next development cycle
* `git push`

## Install

Current release: 0.10.5

`cargo install cargo-release`

## Usage

`cargo release [level]`

### Prerequisite

* Your project should be managed by git.

### Release level

Release level is to tell cargo-release how to bump version.

* By default, cargo release removes pre-release extension; if there is
  no pre-release extension, the current version will be used (0.1.0-pre
  -> 0.1.0, 0.1.0 -> 0.1.0)
* If level is `patch` and current version is a pre-release, it behaves
  like default; if current version has no extension, it bumps patch
  version (0.1.0 -> 0.1.1)
* If level is `minor`, it bumps minor version (0.1.0-pre -> 0.2.0)
* If level is `major`, it bumps major version (0.1.0-pre -> 1.0.0)

From 0.7, you can also use `alpha`, `beta` and `rc` for `level`. It
adds pre-release to your version. You can have multiple `alpha`
version as it goes to `alpha.1`, `alpha.2`…

Releasing `alpha` version on top of a `beta` or `rc` version is not
allowed and will be resulted in error. So does `beta` on `rc`. It is
recommended to use `--dry-run` if you are not sure about the behavior
of specific `level`.

### Signing your git commit and tag

Use `--sign` option to GPG sign your release commits and
tags. [Further
information](https://git-scm.com/book/en/v2/Git-Tools-Signing-Your-Work)

### Upload rust doc to github pages

By using `--upload-doc` option, cargo-release will generate rustdoc
during release process, and commit the doc directory to `gh-pages`
branch. So you can access your rust doc at
https://YOUR-GITHUB-USERNAME.github.io/YOUR-REPOSITORY-NAME/YOUR-CRATE-NAME

If your hosting service uses different branch for pages, you can use
`--doc-branch` to customize the branch we push docs to.

#### WARNING

This option will override your existed doc branch,
use it at your own risk.

### Tag prefix

For single-crate repository, we will use version number as git tag
name.

For multi-crate repository, the subdirectory name will be used as tag
name. For example, when releasing serde_macros 0.7.0 in serde-rs/serde
repo, a tag named as `serde_macros-0.7.0` will be created.

You can always override this behavior by using `--tag-prefix <prefix>`
option.

### Custom remote to push

In case your `origin` is not writable, you can specify custom remote
by `--push-remote` to set the remote to push.

Use `--skip-push` if you do not plan to push to anywhere for now.

### Specifying dev pre-release extension

After release, the version in Cargo.toml will be incremented and have
a pre-release extension added, defaulting to `alpha.0`.

You can specify a different extension by using the
`--dev-version-ext <ext>` option. To disable version bump after
release, use `--no-dev-version` option.

### Update version in README or other files

Cargo-release 0.8 allows you to search and replace version string in
any project or source file. See `pre-release-replacements` in
Cargo.toml configuration below.

### Pre-release hook

Since 0.9, you can configure `pre-release-hook` command in
`Cargo.toml`, for example:

```toml
pre-release-hook = ["echo", "ok"]
```

If the return code of hook command is greater than 0, the release
process will be aborted.

### Maintaining Changelog

At the moment, `cargo release` won't try to generate a changelog from
git history or anything. Because I think changelog is an important
communication between developer and users, which requires careful maintenance.

However, you can still use `pre-release-replacements` to smooth your
process of releasing a changelog, along with your crate. You need to
keep your changelog arranged during feature development, in an `Unreleased`
section (recommended by [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)):

```markdown
## [Unreleased] - ReleaseDate
### Added
- feature 3

### Changed
- bug 1

## [1.0.0] - 2017-06-20
### Added
- feature 1
- feature 2
```

In `Cargo.toml`, configure `cargo release` to do replacements while
bumping version:

```toml
pre-release-replacements = [ {file="CHANGELOG.md", search="Unreleased", replace="{{version}}"}, {file="CHANGELOG.md", search="ReleaseDate", replace="{{date}}"} ]
```

`{{version}}` and `{{date}}` are pre-defined variables with value of
current release version and date.

You can find a real world example in a [handlebars-rust release commit](https://github.com/sunng87/handlebars-rust/commit/ca60fce3e1fce68f427d097d0706a7194600b982#diff-80398c5faae3c069e4e6aa2ed11b28c0)

### Configuration in release.toml

You can persist some options in `release.toml` under your project home
directory, or `.release.toml` at your home directory. Available keys are:

* `sign-commit`: bool, use GPG to sign git commits and tag generated by
  cargo-release
* `upload-doc`: bool, generate doc and push to remote branch
* `doc-branch`: string, default branch to push docs
* `push-remote`: string, default git remote to push
* `disable-push`: bool, don't do git push
* `disable-tag`: bool, don't do git tag
* `dev-version-ext`: string, pre-release extension to use on the next
  development version.
* `pre-release-commit-message`: string, a commit message template for
  release. For example: `"release {{version}}"`, where `{{version}}`
  will be replaced by actual version.
* `pro-release-commit-message`: string, a commit message template for
  bumping version after release. For example: `starting next iteration
  {{version}}`, where `{{version}}` will be replaced by actual
  version.
* `tag-message`: string, a message template for tag. Available
  variables: `{{version}}`, `{{prefix}}` (the tag prefix)
* `tag-prefix`: string, prefix of git tag, note that this will
  override default prefix based on sub-directory.
* `doc-commit-message`: string, a commit message template for doc
  import.
* `no-dev-version`: bool, disable version bump after release.
* `pre-release-replacements`: array of tables, specify files that
  cargo-release will search and replace with new version, check
  [Cargo.toml](https://github.com/sunng87/cargo-release/blob/master/Cargo.toml)
  for example. The table contains three keys:
  * `file`: the file to search and replace
  * `search`: regex that matches string you want to replace
  * `replace`: the replacement string; you can use the following variables:
    * `{{version}}`: the current (bumped) crate version
    * `{{date}}`: the release date
    * `{{prev_version}}`: the version before `cargo-relase` was executed (before any version bump)
* `pre-release-hook`: provide a command to run before `cargo-release`
  commits version change

```toml
[package.metadata.release]
sign-commit = true
upload-doc = true
pre-release-commit-message = "Release {{version}} 🎉🎉"
pre-release-replacements = [ {file="README.md", search="Current release: [a-z0-9\\.-]+", replace="Current release: {{version}}"} , {file ="Cargo.toml", search="branch=\"[a-z0-9\\.-]+\"", replace="branch=\"{{version}}\""} ]
```

### Dry run

Always call `cargo release --dry-run` with your custom options before
actually executing it. The dry-run mode will print all commands to
execute during the release process. And you will get an overview of
what's going on.

Here is an example.

```
 $ cargo release --dry-run
cd .
git commit -S -am (cargo-release) version 0.18.3
cd -
cargo publish
Building and exporting docs.
cargo doc --no-deps
cd target/doc/
git init
cd -
cd target/doc/
git add .
cd -
cd target/doc/
git commit -S -am (cargo-release) generate docs
cd -
cd target/doc/
git push -f git@github.com:sunng87/handlebars-rust.git master:gh-pages
cd -
git tag -a 0.18.3 -m (cargo-release)  version 0.18.3 -s
Starting next development iteration 0.18.4-pre
cd .
git commit -S -am (cargo-release) start next development iteration 0.18.4-pre
cd -
git push origin --follow-tags
```

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
