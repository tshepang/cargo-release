# FAQ

## How do I update my README or other files

Cargo-release 0.8 allows you to search and replace version string in
any project or source file. See [`pre-release-replacements`](reference.md).

## Maintaining Changelog

At the moment, `cargo release` won't try to generate a changelog from
git history or anything. Because I think changelog is an important
communication between developer and users, which requires careful maintenance.

However, you can still use [`pre-release-replacements`](reference.md) to smooth your
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


## How do I upload my rustdoc documentation?

Most of the time, [docs.rs](https://docs.rs/about) service will be good enough
for managing your documentation.  It will automatically generate and post your
documentation when publishing your crate.

However, if you wish to maintain your own copy (like for your unpublished
`master`), you can use `--upload-doc` option, cargo-release will generate
rustdoc during release process, and commit the doc directory to `gh-pages`
branch. So you can access your rust doc at
https://YOUR-GITHUB-USERNAME.github.io/YOUR-REPOSITORY-NAME/YOUR-CRATE-NAME

If your hosting service uses different branch for pages, you can use
`--doc-branch` to customize the branch we push docs to.

#### WARNING

This option will override your existed doc branch,
use it at your own risk.

