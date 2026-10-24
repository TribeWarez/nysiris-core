# Security

Nysiris-Core is experimental protocol code and has not had an independent
security audit. Do not treat the DM, room, or history constructions as audited
production cryptography.

## Reporting

Do not open a public issue for a vulnerability. Use GitHub's private security
advisory flow for `TribeWarez/nysiris-core`:

<https://github.com/TribeWarez/nysiris-core/security/advisories/new>

Include the affected crate, a reproduction, and the impact.

## Scope reminders

- There is no chat service and no server-side message store in this repo.
- History is local encrypted flat files; there is no SQLite chat store.
- Rooms are capability-key groups with client-side fan-out and no authoritative
  server roster. Anyone with an invite can read and post.
- Nym hides who-talks-to-whom from network observers, not all metadata from
  gateways.
- The `nym` feature compiles the heavy `nym-sdk` stack; the default build uses
  an explicit unavailable stub.
- `dev-master.key` belongs to NCC development, not this repository.
