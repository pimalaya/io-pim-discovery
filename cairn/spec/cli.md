---
cairn: spec
capability: cli
status: current
---

# CLI

Behind the `cli` feature, the crate ships a `pim-discovery` `std` binary that exposes discovery from the command line, organised by PIM domain.

### Requirement: Domain-organised commands
The CLI SHALL group commands by PIM domain: `email` (IMAP, POP3, SMTP, JMAP, ManageSieve), `file` (WebDAV), `calendar` (CalDAV, JMAP) and `contact` (CardDAV, JMAP). Each domain groups the mechanisms relevant to it.

### Requirement: Per-mechanism and first subcommands
Each domain SHALL offer a `first` subcommand (the first mechanism, in priority order, that yields a config) plus one subcommand per relevant mechanism presenting its raw, per-mechanism output without merging: `autoconfig`, `srv`, `pacc`, `jmap` for email; `dav`, `pacc`, `jmap` for calendar/contact; `is-google` / `is-microsoft` for the fixed-provider short-circuit.

### Requirement: Shared resolver flag
Every command SHALL accept a DNS resolver argument (a URL or `host:port` pair) defaulting to `1.1.1.1:53`, and render its discovered configs through the shared CLI printer.
