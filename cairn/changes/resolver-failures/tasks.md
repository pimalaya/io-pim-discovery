---
cairn: tasks
change: resolver-failures
---

# Tasks

- [x] `DiscoveryDnsProbe`: one SOA question, completes with the response code.
- [x] `check_resolver` on the compose client: errors for no answer, a failing response code, and a plain `http://` resolver in secure-only mode.
- [x] Every `compose_*` runs it first; `compose_all_within` bounds it by the deadline.
- [x] CLI: every discovery command checks the resolver.
- [x] Tests: response codes, the probe over a canned exchange, secure-only refusing `http://`, an unreachable resolver failing `compose_raw`.
- [x] Spec folded, log, CHANGELOG; check, clippy, fmt.
