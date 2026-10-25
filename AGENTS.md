# AGENTS.md — Nysiris-Core

Shared, UI-agnostic Rust layer for Nysiris-native clients and services.

```text
Nysiris PWA ──wire──► Nysiris services ──► Nysiris-Core
                                              ▲
NCC (PySide6 + ncc-daemon) ───────────────────┘
```

Nysiris-Core must not depend on Nysiris or NCC. Nysiris services and NCC are
consumers; they are not part of this repository.

## Crates

| Crate | Responsibility |
|---|---|
| `nysiris-core` | Identity, DM framing/crypto, contacts, messages, transport trait, sealed files |
| `nysiris-chat-history` | Encrypted append-only flat-file history; no SQLite |
| `nysiris-chat-core` | 1:1 chat engine over the transport trait |
| `nysiris-transport-nym` | Real `nym-sdk` adapter behind the `nym` feature; default builds use an explicit unavailable stub |
| `nysiris-testkit` | In-memory transport and test fixtures |

`nysiris-service` and `nysiris-social-protocol` are planned for the Nysiris
service side. Do not add them here until the extraction happens.

## Rules

- `nysiris-core` must stay Nym-free and UI-free. `nym-sdk` belongs only in
  `nysiris-transport-nym` (or a later transport crate).
- No Qt, Python, React, Nysiris service binaries, or NCC daemon code here.
- Chat history is encrypted flat files. No SQLite, no plaintext message bytes
  on disk.
- Chat protocol domains are `nysiris-chat-v1/*`. Do not reuse Nysiris social
  `fly-social-v1/*` domains or copy social provider routes.
- DM crypto: ed25519 identity → X25519, ephemeral DH, BLAKE2b-256 KDF,
  XChaCha20-Poly1305. The signed inner envelope binds `from`, `to`, `ts`,
  `msgId`, and body.
- Rooms are capability-key groups: a random room key is shared in a signed
  invite, the sender seals one message and fans it out to every member, and no
  relay or server roster exists. Cap rooms at 16 members. Removing a member
  requires a new room/key.
- Transport is a trait. Tests use `nysiris-testkit::MemoryTransport`; live Nym
  tests are manual/opt-in (see `docs` in the NCC repo and
  `client-py/tests/test_nym_live.py`).
- The Nym 1.22.x patch line is internally inconsistent. `Cargo.lock` pins the
  known-good 1.22.0 set; do not regenerate it without testing
  `cargo check -p nysiris-transport-nym --features nym`.
- The lockfile is load-bearing: a plain fresh resolve (`cargo update`, a new
  build directory, `cargo-semver-checks`) selects `nym-client-core` 1.22.1 with
  1.22.2 siblings, which does not compile (1.22.2 needs the yanked
  `tokio_with_wasm` 0.10.1). `nysiris-transport-nym` therefore pins every Nym
  crate to its locked version inside the `nym` feature, which is what makes
  downstream `features = ["nym"]` builds resolve. Cargo ignores optional deps
  that no enabled feature references, so the pins must stay in `nym` — a
  separate private pin feature would be silently dropped.
  `scripts/check-nym-pins.sh` (CI job `nym-pins`, resolution only, no build)
  fails if a from-scratch resolve drifts; keep the pins and `Cargo.lock` in
  step, and pass `--locked` for real builds.
- Crates are packaging-ready but not on crates.io yet; publication is gated on
  the `CORE_PUBLISH_ENABLED` repository variable. Do not publish a placeholder
  version; crates.io versions are immutable.
- Keep consumer compatibility: Nysiris must not depend on
  `nysiris-chat-core`; NCC must not depend on Nysiris. Publish crates in
  dependency order and pin consumers.

## Verify

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
bash scripts/check-nym-pins.sh   # from-scratch Nym resolve, no build
cargo check -p nysiris-transport-nym --features nym --locked   # heavy first build
```

For a consumer smoke test, build Nysiris and NCC against the changed crates.
That is a CI/test relationship, not a Cargo dependency.

## Security notes

- `crates/` is not production-audited crypto. Treat the DM construction as a
  protocol implementation, not a substitute for an audit.
- `dev-master.key` is an NCC-side development fallback, not part of this repo.
- Do not log message content, keys, or contact lists.
- `sealed_file` writes currently use a simple overwrite; add atomic rename +
  fsync before production.
