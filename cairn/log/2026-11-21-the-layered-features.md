---
cairn: log
change: the-layered-features
landed: 2026-11-21
---

# The layered features

The crate moves to the family's layered feature shape (GUIDELINES cargo-006), the one io-imap, io-smtp and io-webdav ship. The old `client` feature, which gated the std-blocking clients and std (anyhow, the io-http std client, resolv-conf, ipconfig), is renamed `std`. `client` now gates the coroutine layer: the `coroutine` module and the io-http and url crates every mechanism needs, so each mechanism feature implies `client` instead of naming those crates itself. `stream` and the TLS features now imply `std`; the default set names `client` beside `rustls-ring`. Every `cfg(feature = "client")` in src/ became `cfg(feature = "std")`, and `extern crate std` sits behind `std` alone (`cli` implies it through `stream`). The pacc-client example requires `pacc` and `std`.

No change folder: the shape is the family rule, applied as io-webdav applied it on 2026-11-18.

Kept outside the model, for the owner to rule on: `stream` stays, since it lets a consumer pick the TLS provider on pimalaya-stream itself and lets the `cli` feature build without one, where io-webdav gates its connect code on the TLS features; and `srv` stays an alias of `rfc6186`. The io-http requirement stays 0.8, whose `client` feature is its std client: the io-http release moving to the same shape renames it `std` and gates its coroutines behind `client`, so the bump to it maps `io-http/client` to `io-http/std` in `std` and adds `io-http/client` to `client`.

Capabilities moved: packaging ("Layered io-free design", "no_std library, std binary" and "One feature per mechanism" modified).

Verified: `cargo build` with no features, `client`, `std`, `stream`, the android set (`autoconfig,pacc,rfc6764,rfc8414,rfc8620`), `rfc9728` alone, a std set and a stream set, `cli` without defaults, default and all features; the pacc-client example under `pacc,std`; `cargo test --all-features` (75 unit); `cargo clippy --all-features --all-targets` and `cargo fmt --check` clean.
