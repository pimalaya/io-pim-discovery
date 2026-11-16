---
cairn: tasks
change: pimalaya-cli-0-6
---

# Tasks

- [x] Cargo.toml: pimalaya-cli 0.6 (dependency and build dependency), `pimalaya-cli/table` dropped from `cli`; `nix develop --command cargo update`.
- [x] src/cli/common.rs: `columns` over `columns::Columns`, network values sanitized; a test pinning the layout and the sanitizing.
- [x] src/cli/misc.rs: `all` on `columns`; the `auth` texts end with a newline.
- [x] src/main.rs: `clap::parse`, `completion::CompletionCommand` and `manual::ManualCommand`, the logger started inside the result.
- [x] CHANGELOG Unreleased; fold the delta into cairn/spec/cli.md.
- [x] `cargo fmt --all --check`; `cargo clippy --all-features --all-targets -- -D warnings`; `cargo test --all-features`; `cargo build --no-default-features`; `cargo check --no-default-features --all-targets` for `client`, `stream`, `cli`, `rustls-aws`, `native-tls` and `native-tls,vendored`.
- [x] Log cairn/log/2026-11-16-pimalaya-cli-0-6.md, status landed.
