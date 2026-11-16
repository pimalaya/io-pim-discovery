---
cairn: spec
capability: cli
status: current
---

# CLI

Behind the `cli` feature, the crate ships a `pim-discovery` `std` binary that exposes discovery from the command line, organised by PIM domain.

### Requirement: Domain-organised commands
The CLI SHALL group commands by PIM domain: `email` (IMAP, POP3, SMTP, JMAP, ManageSieve, Gmail API, Graph mail), `file` (WebDAV), `calendar` (CalDAV, JMAP, Google Calendar, Graph calendars) and `contact` (CardDAV, JMAP, Google People, Graph contacts). Each domain groups the mechanisms relevant to it; `is-google` and `is-microsoft` list the provider's fixed configs of every domain.

### Requirement: Per-mechanism and first subcommands
Each domain SHALL offer a `first` subcommand (the first mechanism, in priority order, that yields a config) plus one subcommand per relevant mechanism presenting its raw, per-mechanism output without merging: `autoconfig`, `srv`, `pacc`, `jmap` for email; `dav`, `pacc`, `jmap` for calendar/contact; `is-google` / `is-microsoft` for the fixed-provider short-circuit.

### Requirement: Shared resolver flag
Every command SHALL accept a DNS resolver argument (a URL or `host:port` pair) defaulting to `1.1.1.1:53` and a `--secure-only` flag, check the resolver before discovering (an unusable resolver is an error, exit 1), and render its discovered configs through the shared CLI printer.

### Requirement: Configs print as aligned columns
In text, a config list SHALL print as pimalaya-cli's `columns::Columns`: the header `SERVICE`, `ENDPOINT`, `USERNAME`, `SOURCE`, `TRUST`, `AUTH`, then one line per config, an absent username or auth printed `-`. The endpoint, the username and the auth methods SHALL go through pimalaya-cli's `sanitize::text` first, so a control character read from the network prints as U+FFFD. `all` SHALL print one such list per domain holding configs (`EMAIL`, `CALENDAR`, `CONTACT`, `FILE`), each under its label line, one blank line between them. Every text output SHALL end with a newline.

#### Scenario: A username holding an escape sequence
- GIVEN an IMAP config on `imap.example.org:993` over TLS whose username is `a`, ESC, `[2Jb`
- WHEN the list is printed as text
- THEN its line reads `imap`, `imap.example.org:993 (SSL)`, `a`, U+FFFD, `[2Jb`, then its source, trust and auth, no ESC reaching the terminal

### Requirement: Exit statuses follow the family contract
The CLI SHALL parse its command line through pimalaya-cli's `clap::parse`: a command line the grammar refuses exits 2, printing the failure object coded `usage-invalid` on stdout when a `--json` word comes before any `--`, clap's text on stderr otherwise. Any other failure, a logger that cannot start included, SHALL print the failure object and exit 1.
