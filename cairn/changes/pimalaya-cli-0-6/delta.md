---
cairn: delta
change: pimalaya-cli-0-6
---

# Delta

## ADDED Requirements

### Requirement: Configs print as aligned columns
In text, a config list SHALL print as pimalaya-cli's `columns::Columns`: the header `SERVICE`, `ENDPOINT`, `USERNAME`, `SOURCE`, `TRUST`, `AUTH`, then one line per config, an absent username or auth printed `-`. The endpoint, the username and the auth methods SHALL go through pimalaya-cli's `sanitize::text` first, so a control character read from the network prints as U+FFFD. `all` SHALL print one such list per domain holding configs (`EMAIL`, `CALENDAR`, `CONTACT`, `FILE`), each under its label line, one blank line between them. Every text output SHALL end with a newline.

#### Scenario: A username holding an escape sequence
- GIVEN an IMAP config on `imap.example.org:993` over TLS whose username is `a`, ESC, `[2Jb`
- WHEN the list is printed as text
- THEN its line reads `imap`, `imap.example.org:993 (SSL)`, `a`, U+FFFD, `[2Jb`, then its source, trust and auth, no ESC reaching the terminal

### Requirement: Exit statuses follow the family contract
The CLI SHALL parse its command line through pimalaya-cli's `clap::parse`: a command line the grammar refuses exits 2, printing the failure object coded `usage-invalid` on stdout when a `--json` word comes before any `--`, clap's text on stderr otherwise. Any other failure, a logger that cannot start included, SHALL print the failure object and exit 1.

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
