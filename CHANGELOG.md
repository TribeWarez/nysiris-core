# Changelog

Notable changes to the Nysiris-Core crates. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions are the workspace
version and the matching annotated tag.

## [0.2.0] - 2026-11-22

### Changed (breaking)

- DM wire format is now byte-exact with the Nysiris browser: sealed boxes are
  `{epubHex, nonceHex, ciphertextB64}` (base64 ciphertext, no `v`, no AAD), and
  signed inner envelopes use `fly-social-v1/dm-inner` v2 with `bodyB64` plus
  optional `atts`. The old `nysiris-chat-v1/dm` domain, AAD, `to` binding, and
  hex ciphertext are gone.
- `seal_to`/`open_sealed` no longer take an AAD argument; `pack_dm_inner` takes
  body bytes plus attachment refs; `unpack_dm_inner` no longer takes the local
  public key. `DM_AAD` is removed.

### Added

- `attach::{AttachmentRef, canonical_attachments}` mirrors Nysiris' encrypted
  attachment refs, including the fail-closed validation rules.
- Shared conformance fixture (`tests/fixtures/dm-v2.json`), a generating
  example, and a cross-repository test. Nysiris CI checks out this pinned
  submodule, runs its tests, and verifies the browser copy is identical.
- `ChatEngine::clear_conversation` for deleting one conversation's encrypted
  journal, and Nym address shape validation for contacts and room members.

## [0.1.4] - 2026-10-25

### Added

- Atomic sealed-file replacement with a previous-generation `.bak` file and
  automatic recovery when the primary file is missing or corrupt.
- Inbound sender authorization: DMs require a saved contact and room messages
  require a key in the local roster.
- Regression tests for sealed-file recovery, truncated history repair, and
  inbound sender checks.

### Changed

- History journals quarantine a torn trailing record as `*.corrupt` and keep
  every complete record before it.
- `dev-master.key` is created with `create_new` and is never overwritten when
  unreadable or malformed.

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
