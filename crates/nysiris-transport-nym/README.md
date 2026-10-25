# nysiris-transport-nym

[![crates.io](https://img.shields.io/crates/v/nysiris-transport-nym.svg)](https://crates.io/crates/nysiris-transport-nym)
[![docs.rs](https://docs.rs/nysiris-transport-nym/badge.svg)](https://docs.rs/nysiris-transport-nym)

[Nym](https://nymtech.net) transport adapter for Nysiris-native clients. It
implements the `Transport` trait from `nysiris-core`, so the chat engine does not
know whether it is talking over Nym or an in-memory queue.

Part of the [Nysiris-Core](https://github.com/TribeWarez/nysiris-core) workspace.

## Feature gate

The real adapter is behind the `nym` feature, which compiles the full `nym-sdk`
stack:

```toml
[dependencies]
nysiris-transport-nym = { version = "0.1", features = ["nym"] }
```

Without the feature, `NymTransport::connect` returns an explicit error instead
of pretending to deliver messages, so the default build stays light and offline.
The first build with `nym` enabled takes several minutes.

## API

- `NymTransport::connect(data_dir, gateway)` — start or connect to a Nym client.
- `NymTransport::is_available()` — whether the real adapter was compiled in.
- `address()` — the client's Nym address, to hand out in invites.
- `recv_timeout(timeout)` — receive one `InboundMessage`, including the optional
  sender tag.
- `shutdown()` — stop the client.
- `Transport` impl — `send` accepts a Nym address and opaque payload.

## Nym version pinning

The Nym 1.22.x patch line on crates.io is internally inconsistent:
`nym-client-core` 1.22.2 requires the yanked `tokio_with_wasm` 0.10.1, and the
1.22.1 fallback does not compile against the 1.22.2 siblings. This crate pins
every Nym crate to the known-good 1.22.0 set inside its `nym` feature, so both
`Cargo.lock` builds and fresh resolves use a graph that compiles.

Do not `cargo update` the Nym crates without re-testing `features = ["nym"]`.

## Security

Experimental protocol code, not independently audited. Nym hides who-talks-to-whom
from network observers, not all metadata from gateways: gateway queues and
timing remain observable. Report vulnerabilities privately through
[SECURITY.md](https://github.com/TribeWarez/nysiris-core/blob/main/SECURITY.md).

## License

Apache-2.0. See
[LICENSE](https://github.com/TribeWarez/nysiris-core/blob/main/LICENSE).
