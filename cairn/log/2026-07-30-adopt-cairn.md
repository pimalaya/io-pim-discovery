---
cairn: log
change: adopt-cairn
landed: 2026-07-30
---

# Adopt Cairn

Converted the placeholder `docs/` folder into a Cairn root. `docs/` only ever held an index with no landed sub-documents, so nothing was migrated wholesale; the current, landed truth was seeded once from the `src/lib.rs` header (the crate's architecture document) and the module structure.

The truth became five spec capabilities: packaging (the layered `no_std` io-free library and its optional `std` CLI, one feature per mechanism), coroutine (the I/O-free `DiscoveryCoroutine` contract and the shared DNS/HTTP transport and stream pool), mechanisms (the per-RFC discovery probes: autoconfig, PACC, RFC 6186/8314 SRV, RFC 6764 DAV, RFC 8620 JMAP, RFC 8414/9728 OAuth metadata, and the WWW-Authenticate probe), compose (the reduction into a ranked config list, the provider short-circuit, and the deadline-bounded `compose_all_within`), and cli (the `pim-discovery` binary's domain-organised commands).

The seed reflects current truth including this session's earlier fix, so the `mechanisms` capability already records the four SRV labels (`_imap`, `_imaps`, `_submission`, `_submissions`) rather than the pre-fix three (see the `srv-implicit-tls-submission` log entry).

Defaults apply throughout, so no `cairn.toml` is needed; the `cairn/` directory alone marks the root, `AGENTS.md` carries the activation stanza, and `CLAUDE.md` points Claude Code at it. `CONTRIBUTING.md` now chains to `cairn/` instead of `docs/`.

This is a documentation reorganisation with no behaviour change.
