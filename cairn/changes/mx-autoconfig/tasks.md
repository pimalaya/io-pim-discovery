---
cairn: tasks
change: mx-autoconfig
---

# Tasks

- [x] `mx_base_domain` over the Public Suffix List (`psl`, under `autoconfig`); `mx_parent_domain` removed.
- [x] `DiscoveryDnsCname`, following the chain an A answer carries.
- [x] `DiscoveryIsp::host_url`, the ISP main URL of any host.
- [x] Compose: alias following on the ISP main URL, `Mechanism::MxAutoconfig` after the ISPDB, `DiscoveryConfigSource::Mx`.
- [x] Live: `test@migadu.net` gives `imap.migadu.com` and `smtp.migadu.com`; Fastmail and Gmail unchanged.
- [x] Spec folded, log, CHANGELOG; clippy, fmt, test.
