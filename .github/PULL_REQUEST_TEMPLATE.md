## What and why

<!-- What changes, and what failure or requirement drives it? -->

## How it was tested

<!-- Commands you ran, tests added, and whether the `nym` feature was involved. -->

## Checklist

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo doc --workspace --no-deps --locked`
- [ ] `bash scripts/check-nym-pins.sh` (if dependencies changed)
- [ ] `cargo check -p nysiris-transport-nym --features nym --locked` (if transport, the `Transport` trait, or dependencies changed)
- [ ] No message content, keys, identities, contact lists, or Nym `sp-storage` in the diff, logs, or fixtures
- [ ] `README.md`, `AGENTS.md`, or crate docs updated if behaviour or invariants changed
- [ ] Public API changes are intentional and noted in the description

## Notes for reviewers

<!-- Trade-offs, follow-ups, or anything deliberately left out. -->
