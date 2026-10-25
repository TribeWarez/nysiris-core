# Changelog

Notable changes to the Nysiris-Core crates. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions are the workspace
version and the matching annotated tag.

## [0.1.3] - 2026-10-25

### Added

- Per-crate README files, referenced through the `readme` manifest field so
  crates.io and docs.rs show crate-specific documentation.
- `keywords` and `categories` metadata for discoverability on crates.io.
- Rustdoc coverage: `#![warn(missing_docs)]` in every crate and doc comments on
  all public modules, types, traits, fields, and methods.

### Changed

- `README.md`, `AGENTS.md`, and `CONTRIBUTING.md` describe the published `0.1.x`
  line, annotated tags, and the `--locked` release gate.
- Doc builds run with `RUSTDOCFLAGS=-D warnings` in CI and release, so broken
  intra-doc links and missing public docs fail the gate.

### Notes

- crates.io versions are immutable: this documentation and metadata reach the
  registry with this release, not by editing `0.1.2`.

## [0.1.2] - 2026-10-25

First public release: all five crates published to crates.io in dependency
order.

### Added

- `scripts/check-nym-pins.sh` and the `nym-pins` CI job: reproduce a
  from-scratch consumer resolve and fail if it drifts from `Cargo.lock`.
- Exact pins for every Nym crate inside `nysiris-transport-nym`'s `nym` feature,
  so downstream builds resolve the buildable 1.22.0 set.

### Changed

- CI and release builds pass `--locked`; a stale lock fails loudly instead of
  silently re-resolving an unbuildable Nym graph.
- The release workflow verifies the tag against `[workspace.package].version`,
  validates the packaged file lists, and publishes behind
  `CORE_PUBLISH_ENABLED` plus `CARGO_REGISTRY_TOKEN` in the `crates-io`
  environment.

### Notes

- `v0.1.2` is a lightweight tag; cut future releases with an annotated tag so
  `git submodule status` and `git describe` show the version.

## [0.1.1] - 2026-10-25

### Changed

- Prepared the workspace for crates.io: workspace-inherited versions, registry
  versions alongside every internal path dependency, and the publish gate in
  release CI.

## [0.1.0] - 2026-10-24

### Added

- Initial walking skeleton: identity, signed DM envelope, sealed DM crypto,
  contacts, messages, encrypted flat-file history, chat engine with
  capability-key rooms, transport trait, in-memory testkit, and the `nym-sdk`
  adapter behind the `nym` feature.
