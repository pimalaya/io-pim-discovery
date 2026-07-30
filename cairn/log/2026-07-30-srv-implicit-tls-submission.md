---
cairn: log
change: srv-implicit-tls-submission
landed: 2026-07-30
---

# RFC 8314 implicit-TLS submission SRV lookup

Taught RFC 6186 SRV discovery to query `_submissions._tcp` (RFC 8314 implicit TLS, port 465), the send-side counterpart of the `_imaps._tcp` receive-side lookup it already ran.

The SRV flow was asymmetric: the receive side queried both `_imap._tcp` (STARTTLS) and `_imaps._tcp` (implicit TLS), but the send side queried only `_submission._tcp` (STARTTLS, port 587). A domain that publishes only `_submissions` (mirroring its `_imaps`, as Migadu-hosted domains do) therefore had its IMAP endpoint discovered but no SMTP one. The composed report exposed a `smtp: None`, so a downstream wizard (Himalaya) fell back to a guessed `smtp.<domain>` and failed to connect — reported as [himalaya#722](https://github.com/pimalaya/himalaya/issues/722).

`DiscoverySrv` now runs a fourth lookup after `_submission`, `DiscoverySrvReport` carries a `submissions` slot alongside `submission`, and `DiscoveryServiceConfig::from_srv` maps it to an SMTP config over implicit TLS. A consumer that prefers the most secure endpoint (TLS over STARTTLS) then picks the implicit-TLS submission when both are advertised. A regression test in `compose/config.rs` reproduces the reporter's DNS shape (`_imaps` + `_submissions` → Migadu) and asserts the TLS SMTP config is produced.

This landed just before the repository adopted Cairn (see the `adopt-cairn` log entry the same day); the four SRV labels are folded into the `mechanisms` spec capability that the adoption seeded.
