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
