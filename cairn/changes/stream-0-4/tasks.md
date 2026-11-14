---
cairn: tasks
change: stream-0-4
---

# Tasks

- [x] Cargo.toml: pimalaya-stream 0.4, io-http 0.8; `nix develop --command cargo update`; `cargo tree -i pimalaya-stream --all-features` and `cargo tree -i io-http --all-features` each show one version.
- [x] src/main.rs: `Tls { server: flags.tls_cert.map(|cert| TlsServer { cert }), client: None, .. }`.
- [x] Doc comments and examples naming `Tls::cert` follow.
- [x] CHANGELOG Unreleased: the two BREAKING entries.
- [x] `cargo fmt`; `cargo test --all-features`; `cargo clippy --all-features --all-targets -- -D warnings`; `cargo build --no-default-features`; `cargo check --no-default-features --all-targets` for `client`, `stream`, `cli`, `rustls-aws`, `native-tls` and `native-tls,vendored` (as the 2026-11-13 io-http-0-7 log did).
- [x] Log cairn/log/2026-11-14-stream-0-4.md (frontmatter `landed:`), status landed.
- [ ] Commit `build!: bump pimalaya-stream to 0.4 and io-http to 0.8, update deps`, pushed by the owner's session; the apps pin its rev.
