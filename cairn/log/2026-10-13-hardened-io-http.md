---
cairn: log
change: hardened-io-http
landed: 2026-10-13
---

# io-http pinned to its hardening fix

`[patch.crates-io]` pins io-http to e181530, unreleased, until a 0.5 release carries it. Every probe goes through it, against hosts discovery has no reason to trust: a response head over 64 KiB or a body over 256 MiB now fails the probe instead of being buffered, a connection closed before the body is complete fails with `Eof` instead of spinning on `WantsRead`, and a chunk size near `usize::MAX` no longer panics. Nothing in this crate changed: the new error variants only travel inside `Http11SendError`.

Capabilities moved: none.

Verified: `cargo build`, `clippy --all-targets` and `test --all-features` with `rustls-ring,cli` and the defaults; the one clippy warning (`src/pacc/config.rs`, collapsible `if`) predates the pin. `pim-discovery --json all --secure-only` answers Fastmail live as before.
