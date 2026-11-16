---
cairn: change
id: pimalaya-cli-0-6
status: landed
created: 2026-11-16
---

# pimalaya-cli 0.6

## Why

The `cli` feature still required pimalaya-cli 0.3, the last family dependency of this crate behind its latest release (0.6.0, 2026-11-15). The stream-0-4 change left this move for a change of its own, since pimalaya-cli 0.4 removed the `table` module and its `table` feature, which the `pim-discovery` binary printed its configs with. Releasing the crate on 0.3 would ship a binary off the family's text and exit contract.

## What

- Cargo.toml: both pimalaya-cli requirements (the build helpers and the terminal one) from 0.3 to 0.6, the `pimalaya-cli/table` feature dropped from `cli`; `cargo update`.
- src/cli/common.rs: `table` becomes `columns`, a `columns::Columns` with `AUTH`, the longest value, last; the endpoint, the username and the auth methods go through `sanitize::text`.
- src/cli/misc.rs: `all` prints the columns of each domain; the three "nothing found" texts of `auth` end with a newline.
- src/main.rs: `clap::parse` in place of `Cli::parse`, so a refused command line under `--json` prints the failure object coded `usage-invalid`; the completion and manual commands imported from their now public modules; the logger started inside the result, so a failure to start it is reported and exits 1 rather than panicking.
- CHANGELOG Unreleased and the cli spec follow.

Untouched: the 0.5 partial outcome exit (3), the spinner colour rule and `Listing`'s next page do not reach this binary, which prints no partial result, runs no spinner and pages nothing. Its log lines carry no colour since pimalaya-cli 0.3.2 stopped enabling env_logger's `auto-color`. Help still wraps to the terminal: the crate's own clap requirement enables `wrap_help`.

## Cost

Breaking for a caller of the `cli` module (no app enables `cli`): its commands take pimalaya-cli 0.6's `Printer`, and `cli::common::table` is now `cli::common::columns`. The text of a config list changes layout; its JSON does not.
