---
cairn: change
id: resolver-failures
status: landed
created: 2026-10-04
---

# Resolver failures are errors

Asked by MOA (`docs/plan/findings.md`, "`pim-discovery` and DNS over HTTPS"), after [secure-only](../secure-only/proposal.md).

## Why

Every mechanism skips its own errors, which is right for one mechanism that finds nothing, but wrong when the resolver itself does not answer: then every DNS-backed mechanism comes back empty at once, and the result looks like a domain with no services.

- An unreachable resolver, a DNS-over-HTTPS resolver refusing HTTP/1.1 (Quad9 answers `505`), or one answering `SERVFAIL` or `REFUSED`, all give `[]` with exit 0. A Google Workspace domain, found by its MX records only, disappears.
- `gmail.com` still comes back from the fixed provider rules, so the answer is partial with nothing to tell.
- In secure-only mode, a plain `http://` resolver is accepted: the lookups travel in clear over HTTP, the one transport that mode refuses everywhere else. Today the pool refuses the stream silently, and the result is the same empty list.

A caller cannot tell "nothing found" from "could not look". A setup wizard then tells a user their provider offers nothing.

## What

- **A resolver probe before discovery.** The compose client asks the resolver one question about the address's domain (its SOA record) before running the mechanisms:
  - an answer whose response code is `NOERROR` or `NXDOMAIN` means the resolver works, whatever the domain has;
  - no answer (connection refused or closed, HTTP error, unparseable message) or any other response code (`SERVFAIL`, `REFUSED`…) fails discovery with an error naming the resolver.
- **Where it runs.**
  - Library: every `compose_*` entry point, before the fan-out; `compose_all_within` bounds it by the same deadline, and a probe still waiting at the deadline is a failure. The probe is public (`check_resolver`) for callers of the single mechanisms.
  - CLI: every discovery command, so `--json` prints the error envelope and exits 1.
- **Secure-only refuses a plain `http://` resolver** before any request, with an error. A `tcp://` resolver (or a bare `host:port`) stays allowed: [secure-only](../secure-only/proposal.md) settled that DNS stays on the configured resolver, and DNS over HTTPS is the caller's choice.
- **Unchanged.** A mechanism that fails on its own (an autoconfig URL that 404s, a JMAP origin that refuses) is still skipped: only the resolver's failure is fatal.

## Not here

HTTP/2 for DNS over HTTPS, which Quad9, Mullvad and DNS4EU require (io-http speaks HTTP/1.1). The probe makes their refusal visible instead of silent.
