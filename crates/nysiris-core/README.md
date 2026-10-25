# nysiris-core

[![crates.io](https://img.shields.io/crates/v/nysiris-core.svg)](https://crates.io/crates/nysiris-core)
[![docs.rs](https://docs.rs/nysiris-core/badge.svg)](https://docs.rs/nysiris-core)

Shared primitives for Nysiris-native chat clients and services: identities,
signed direct-message envelopes, message framing, contacts, capability-key
rooms, transport traits, and sealed files.

Part of the [Nysiris-Core](https://github.com/TribeWarez/nysiris-core) workspace.
That repository depends on neither Nysiris nor NCC; both consume it. There is no
chat service, no server-side message store, and no SQLite.

## Install

```toml
[dependencies]
nysiris-core = "0.1"
```

## What you get

- `ChatIdentity` — ed25519 identity with X25519 key agreement.
- `SealedDm` and `DM_AAD` — ephemeral DH, BLAKE2b-256 KDF,
  XChaCha20-Poly1305.
- `DmInner` and `pack_dm_inner` — signed inner envelope bound to `from`, `to`,
  `ts`, `msgId`, and body.
- `Group` and `GroupMember` — capability-key rooms with signed invites, a
  16-member cap, and `MAX_GROUP_NAME_CHARS`.
- `Contact`, `ChatMessage`, `Direction`, `MessageState`, conversation summaries,
  and `MAX_BODY_BYTES`.
- `Transport` and `SentPayload` — the trait every transport implements.
  `nysiris-testkit` ships an in-memory fake; `nysiris-transport-nym` ships the
  real Nym adapter.
- `sealed_file` — encrypted local file helpers, used by the history crate.
- `ids` and `error` — conversation ids and the shared `CoreError`/`Result`.

## Related crates

| Crate | Purpose |
|---|---|
| [`nysiris-chat-core`](https://crates.io/crates/nysiris-chat-core) | 1:1 and room chat engine built on this crate |
| [`nysiris-chat-history`](https://crates.io/crates/nysiris-chat-history) | Encrypted append-only local history |
| [`nysiris-transport-nym`](https://crates.io/crates/nysiris-transport-nym) | Nym transport behind a `nym` feature |
| [`nysiris-testkit`](https://crates.io/crates/nysiris-testkit) | In-memory transport for tests |

## Security

Experimental protocol code that has not had an independent audit. Do not treat
the DM, room, or history constructions as audited production cryptography.
Report vulnerabilities privately through
[SECURITY.md](https://github.com/TribeWarez/nysiris-core/blob/main/SECURITY.md),
never in a public issue. Never log or persist message content, keys, or contact
lists in the clear.

## License

Apache-2.0. See
[LICENSE](https://github.com/TribeWarez/nysiris-core/blob/main/LICENSE).
