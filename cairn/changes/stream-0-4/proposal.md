---
cairn: change
id: stream-0-4
status: active
created: 2026-11-14
---

# pimalaya-stream 0.4

## Why

pimalaya-stream 0.4 (2026-11-13) moved the trusted certificate from `Tls::cert` to `Tls::server`, a `TlsServer { cert }`, and added `Tls::client` for mutual TLS. io-http 0.8 changed nothing but that dependency. The family moved on the same day: io-oauth 0.7, io-imap 0.10, io-smtp 0.9, io-msgraph 0.9, io-gmail 0.8, io-jmap 0.9, and on master io-webdav 185ab63, io-gcal dbb4481 and io-gpeople e4a9cac.

This crate is the last one on the 0.3 line that an app links. cardamum and calendula enable its `stream` feature for their CardDAV and CalDAV discovery, so neither can take pimalaya-stream 0.4 without two copies of the stream crate and of io-http, the `Tls` they hand its discovery clients (`with_tls`) being 0.3's. Their migrations (each app's cairn change a-result-carries-its-errors) start with this bump. Carillon, its third user, drops it with its wizard (carillon's the-family-line).

## What

- Cargo.toml: `pimalaya-stream` 0.3.1 to 0.4, `io-http` 0.7 to 0.8; `cargo update`. The optional `pimalaya-cli` 0.3 of the `cli` feature stays: moving the `pim-discovery` binary to pimalaya-cli 0.4 (its `table` is gone) is a change of its own, and no app enables `cli`.
- src/main.rs: `From<TlsFlags> for Tls` fills `server: flags.tls_cert.map(|cert| TlsServer { cert })` and `client: None`. `--tls-cert` keeps its name and meaning.
- Doc comments naming `Tls::cert`, if any, follow.
- CHANGELOG Unreleased: two **BREAKING** entries, as io-http 0.8 and io-oauth 0.7 worded theirs ("Changed the pimalaya-stream dependency to 0.4. Its `tls::Tls` moved the trusted certificate from `cert` to `server`, and gained `client`, the certificate presented when the server asks for one (mutual TLS).", "Changed the io-http dependency to 0.8.").

No version bump: the release (0.13.0) is the owner's. Until then the apps pin the commit by git rev in `[patch.crates-io]`.

## Cost

Callers on pimalaya-stream 0.3 must move to 0.4 with it: the `Tls` they pass changes type. No behaviour changes.
