---
cairn: log
change: mx-autoconfig
landed: 2026-10-16
---

# MX provider autoconfig and alias following

Capabilities moved: mechanisms (MODIFIED Mozilla autoconfig), compose (MODIFIED Ordered mechanism fan-out).

A domain publishing nothing but its MX records now discovers its mail through the provider those records name, as Thunderbird does: `mx_base_domain` reduces each MX host to its registrable domain over the Public Suffix List (`psl`, under `autoconfig`), and `Mechanism::MxAutoconfig`, after the ISPDB, tries its ISP main URL then its ISPDB entry, tagged `DiscoveryConfigSource::Mx`. The ISP main URL, own and provider's, follows a CNAME when it fails (`DiscoveryDnsCname`, reading the chain an A answer carries), because providers alias their customers' autoconfig to a host whose certificate does not cover the alias. `DiscoveryIsp::host_url` builds the URL for any host. `mx_parent_domain` is gone. The Android app's own orchestration takes the same bricks.

Found on `test@migadu.net`: MX at `flatmail.com`, whose `autoconfig.flatmail.com` aliases `autoconfig.migadu.com` and serves a certificate for `admin.migadu.com` only. Verified live: it gives `imap.migadu.com:993` and `smtp.migadu.com:465`; `test@fastmail.com` gains an `mx` row beside its PACC and ISP ones, Gmail is unchanged. `cargo test --all-features`, clippy (the `src/pacc/config.rs` warning predates) and the `autoconfig`-only and `pacc,client` builds pass.
