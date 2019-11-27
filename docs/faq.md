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
<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added
- feature 3

### Changed
- bug 1

## [1.0.0] - 2017-06-20
### Added
- feature 1
- feature 2

<!-- next-url -->
[Unreleased]: https://github.com/assert-rs/predicates-rs/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/assert-rs/predicates-rs/compare/v0.9.0...v1.0.0
```

In `Cargo.toml`, configure `cargo release` to do replacements while
bumping version:

```toml
pre-release-replacements = [
  {file="CHANGELOG.md", search="Unreleased", replace="{{version}}"},
  {file="CHANGELOG.md", search="\\.\\.\\.HEAD", replace="...{{tag_name}}"},
  {file="CHANGELOG.md", search="ReleaseDate", replace="{{date}}"},
  {file="CHANGELOG.md", search="<!-- next-header -->", replace="<!-- next-header -->\n\n## [Unreleased] - ReleaseDate"},
  {file="CHANGELOG.md", search="<!-- next-url -->", replace="<!-- next-url -->\n[Unreleased]: https://github.com/assert-rs/predicates-rs/compare/{{tag_name}}...HEAD"},
]
```

`{{version}}` and `{{date}}` are pre-defined variables with value of
current release version and date.

`predicates` is a real world example
- [`release.toml`](https://github.com/assert-rs/predicates-rs/blob/master/release.toml)
- [`CHANGELOG.md`](https://github.com/assert-rs/predicates-rs/blob/master/CHANGELOG.md)
