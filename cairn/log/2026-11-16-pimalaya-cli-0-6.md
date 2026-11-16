---
cairn: log
change: pimalaya-cli-0-6
landed: 2026-11-16
---

# pimalaya-cli 0.6

The `cli` feature moves from pimalaya-cli 0.3 to 0.6.0 (2026-11-15), both its dependency and its build dependency, and drops `pimalaya-cli/table`, which 0.4 removed. `cargo update` took pimalaya-cli 0.6.0 and removed comfy-table and the crossterm, parking_lot and winapi crates it pulled. Every other family dependency was already at its latest release: io-http 0.8, pimalaya-stream 0.4, base64-codec 0.1, simd-cpu 0.1.

**Configs print as aligned columns.** The bordered comfy-table, sized to the terminal, gives way to `columns::Columns`: `SERVICE`, `ENDPOINT`, `USERNAME`, `SOURCE`, `TRUST`, then `AUTH` last, the value of unbounded length the family's list rule puts at the end. The endpoint, the username and the auth methods, read from autoconfig, PACC, SRV or JMAP answers, go through `sanitize::text`, which nothing did before: a username holding ESC `[2J` prints U+FFFD in place of the ESC (test `prints_configs_as_columns_with_network_values_sanitized`). `cli::common::table` became `cli::common::columns`. JSON is unchanged.

**The exit contract.** The binary parses through `pimalaya_cli::clap::parse`: `pim-discovery --json email bogus-cmd` printed clap's text on stderr, it prints `{"code":"usage-invalid",...}` on stdout, both exiting 2. The logger starts inside the result, so a failure to start it prints the failure object and exits 1 where it panicked. The 0.5 partial outcome status (3), the 0.5.2 spinner colour rule and 0.6's `Listing` next page reach nothing here: the binary prints no partial result, runs no spinner and pages nothing. Its log lines carry no colour since 0.3.2.

**The `auth` texts end with a newline.** `No authentication scheme advertised by ...`, `No authorization server metadata found for ...` and `No protected resource metadata found for ...` were written without one.

Capabilities moved: cli (ADDED "Configs print as aligned columns" and "Exit statuses follow the family contract").

Verified: `cargo fmt --all --check` clean, `cargo clippy --all-features --all-targets -- -D warnings` clean, `cargo test --all-features` passed (75), `cargo build --no-default-features` clean, `cargo check --no-default-features --all-targets` clean for `client`, `stream`, `cli`, `rustls-aws`, `native-tls` and `native-tls,vendored`; `pim-discovery --version`, a refused subcommand with and without `--json` (exit 2) checked by hand, no discovery run.
