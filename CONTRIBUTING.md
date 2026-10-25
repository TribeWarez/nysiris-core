# Contributing to Nysiris-Core

Thanks for helping. This repository is shared protocol code: changes here affect
the Nysiris services and NCC, so the bar is "small, explained, and tested"
rather than "large and clever".

Read [`AGENTS.md`](AGENTS.md) first. It lists the invariants (Nym-free core, no
SQLite, no chat service, `nysiris-chat-v1/*` domains, room caps) that pull
requests must not relax. It also explains how the crates are consumed.

## Before you start

- Rust 1.85 or newer, stable toolchain.
- The default build needs no system packages and no network.
- Large or protocol-affecting changes are easier to review after a discussion:
  open a
  [Discussions](https://github.com/TribeWarez/nysiris-core/discussions) thread or
  an issue first, and describe the wire format, the failure modes, and what you
  deliberately did not change.

Do not open a public issue or pull request for a vulnerability. Use
[`SECURITY.md`](SECURITY.md) instead.

## Set up

```bash
git clone https://github.com/TribeWarez/nysiris-core
cd nysiris-core
cargo test --workspace --locked
```

## Gates

Run these before pushing; CI runs the same set on Linux and Windows:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
bash scripts/check-nym-pins.sh
cargo check -p nysiris-transport-nym --features nym --locked   # heavy first build
```

- Always pass `--locked`. `Cargo.lock` pins the only Nym set that compiles; a
  fresh resolve selects a broken 1.22.x mix. If you must change dependencies,
  regenerate deliberately and re-run the Nym check and the pin script.
- `scripts/check-nym-pins.sh` proves that a downstream consumer without our
  lockfile still resolves the pinned Nym set. It is cheap (resolution only).
- The `nym` build is slow. It is still required for changes that touch
  `nysiris-transport-nym`, the `Transport` trait, or dependency versions.

## Tests

- Core tests must stay offline and use `nysiris-testkit`'s in-memory transport.
  No test may contact mainnet.
- Live Nym checks are manual and opt-in: two native clients plus
  `NCC_LIVE_NYM=1`. They are not part of CI.
- History tests use temporary directories and assert that no SQLite file and no
  plaintext message content appears on disk.
- When fixing a bug, add a test that fails without your change. Protocol
  changes need vectors or round-trip tests for both peers.

## Code rules

- Keep `nysiris-core` Nym-free and UI-free. `nym-sdk` belongs only in
  `nysiris-transport-nym`.
- No Qt, Python, React, Nysiris service binaries, or NCC daemon code here.
- No SQLite, no plaintext message bytes on disk, no logging of message content,
  keys, or contact lists.
- New dependencies need a reason in the pull request. Prefer the standard
  library and existing crates.
- Chat protocol domains are `nysiris-chat-v1/*`. Never reuse Nysiris social
  `fly-social-v1/*` domains or copy social provider routes.
- Public items are API. Nothing is published to crates.io yet, but
  `cargo semver-checks` still runs against the previous tag, so renames and
  signature changes need to be intentional.

## Pull requests

- Keep each pull request focused; unrelated refactors make protocol review
  harder.
- Use an imperative subject line ("Fix room key rotation on member removal"),
  and explain *why* in the body: the failure being fixed, the trade-off chosen,
  and anything you deliberately left out.
- Update `README.md`, `AGENTS.md`, and the crate docs when behaviour or
  invariants change.
- Fill in the pull request template checklist; CI must be green before review.
- Never commit `target/`, `.venv/`, Nym `sp-storage`, identity keys, history
  journals, SQLite files, or tokens.

## Review

- Expect questions about wire compatibility, key handling, and what happens on
  replay, reordering, or a lost message.
- Security-sensitive changes may be asked for a threat-model paragraph before
  review.
- Maintainers may push small fixes to your branch; larger changes are requested
  as review comments.

## Community

Ask questions and talk through design in the repository
[Discussions](https://github.com/TribeWarez/nysiris-core/discussions); keep
issues for reproducible bugs and concrete proposals. Questions about the client
UI belong in [TribeWarez/ncc](https://github.com/TribeWarez/ncc); Nym network and
SDK questions belong upstream at [nymtech.net](https://nymtech.net). If this
work is useful to you, you can [sponsor it](https://github.com/sponsors/TribeWarez).
