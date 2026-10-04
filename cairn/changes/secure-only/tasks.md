---
cairn: tasks
change: secure-only
---

# Tasks

- [x] Client option `secure_only`, CLI flag `--secure-only` on every command.
- [x] Autoconfig: only the `https://` URLs; no `http://` mailconf target or redirect.
- [x] RFC 6764: no `http://` origin.
- [x] Composition: drop `plain` and `http://` endpoints.
- [x] Tests: each plain path skipped with the option, kept without it.
- [x] Settle the resolver question.
- [x] Spec folded, log, CHANGELOG; check, clippy, fmt.
