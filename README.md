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

## Verify

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p nysiris-transport-nym --features nym   # heavy first build
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

## License

Apache-2.0. See `LICENSE`.
