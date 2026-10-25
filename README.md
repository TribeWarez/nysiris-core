# Nysiris-Core

Shared Rust primitives for Nysiris-native clients and services.

Repository: <https://github.com/TribeWarez/nysiris-core> · License: Apache-2.0

This repository is the middle of the dependency graph:

```text
Nysiris PWA ──wire──► Nysiris services ──► Nysiris-Core
                                              ▲
NCC (PySide6 + ncc-daemon) ───────────────────┘
```

It must not depend on Nysiris or NCC. Nysiris-side services consume the
service/social crates; NCC consumes the chat/transport/history crates.

## Status

Phase 0 walking skeleton:

- `nysiris-core`: chat identity, signed DM inner envelope, sealed DM crypto,
  contacts, message types, transport trait, encrypted sealed files.
- `nysiris-chat-history`: encrypted append-only flat-file history. No SQLite.
- `nysiris-chat-core`: 1:1 chat engine plus capability-key rooms with
  client-side fan-out over a transport trait.
- `nysiris-transport-nym`: real `nym-sdk` adapter behind the `nym` feature;
  default builds keep an explicit unavailable stub. Live mainnet DM and room
  delivery were verified with two native daemons.
- `nysiris-testkit`: in-memory transport.

OS keychain integration and crates.io publication are next. Crates are
`publish = false` until the API stabilizes.

## Releasing

Releases are cut by tagging the workspace version:

```bash
git tag v0.1.1 && git push origin v0.1.1
```

`.github/workflows/release.yml` verifies the tag against
`[workspace.package].version`, re-runs the full gate (fmt, clippy, tests, docs,
Nym feature compile), validates the packaged file lists, and creates a GitHub
release. crates.io publication runs in dependency order and is gated on the
`CORE_PUBLISH_ENABLED` repository variable plus a `CARGO_REGISTRY_TOKEN` secret
bound to the `crates-io` environment:

```bash
gh variable set CORE_PUBLISH_ENABLED --repo TribeWarez/nysiris-core --body true
gh secret set CARGO_REGISTRY_TOKEN --repo TribeWarez/nysiris-core
```

Or run `./scripts/configure-github.ps1 -EnablePublish` once; it also installs the
Nysiris Android signing secrets and creates the `crates-io` environment.

crates.io versions are immutable, so the publish job skips versions that already
exist and a failed release can be re-run. A dependent crate can only be
`cargo package`d after its dependency is on crates.io, so `cargo publish`
performs the verified build per crate, in dependency order.

The existing `v0.1.0` tag predates publish enablement and is already pinned by
NCC's submodule. For the first published release, bump the workspace version and
tag the new commit rather than moving `v0.1.0`.

## Verify

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/check-nym-pins.sh   # from-scratch Nym resolve, no build
cargo check -p nysiris-transport-nym --features nym --locked   # heavy first build
```

## Security notes

- DM crypto mirrors the Nysiris construction: ed25519 identity converted to
  X25519, ephemeral DH, BLAKE2b-256 KDF, XChaCha20-Poly1305.
- The DM inner envelope is signed and bound to `from`, `to`, `ts`, `msgId`,
  and body.
- History records are encrypted and bound to a hashed conversation filename.
- No message content is written unencrypted.
- The Nym dependency set is pinned in `Cargo.lock`; the current 1.22.x patch
  line is internally inconsistent. Do not run `cargo update` without testing
  `--features nym`.
- `nysiris-transport-nym` also pins every Nym crate to its locked version inside
  the `nym` feature, so consumers — and anything that resolves from scratch —
  get the buildable 1.22.0 set: `nym-client-core` 1.22.2 needs the yanked
  `tokio_with_wasm` 0.10.1, and the 1.22.1 fallback does not compile against
  the 1.22.2 `nym-crypto` / `nym-client-core-config-types` siblings.
  `scripts/check-nym-pins.sh` fails if that resolve ever drifts. CI still builds
  with `--locked`, and the API-compatibility job checks default features only
  (cargo-semver-checks resolves in a throwaway project; the public API surface
  is the same with or without `nym`).

## Security

See `SECURITY.md` for private reporting and scope reminders.

## License

Apache-2.0. See `LICENSE`.
