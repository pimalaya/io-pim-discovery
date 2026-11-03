---
cairn: change
id: oauth-metadata-relocation
status: draft
created: 2026-11-03
---

# Move the OAuth metadata types into io-oauth

## Why
RFC 8414 authorization server metadata and RFC 9728 protected resource metadata are OAuth specifications, and a discovery mechanism belongs to the library owning its RFC. io-pim-discovery holds both today, so io-oauth consumers wanting a server's endpoints depend on a PIM discovery crate. Split out of Ortie's discovery-layering change on 2026-11-03, once the typed resource (io-oauth resource-indicators, this crate's oauth-resource) had removed the urgency.

## What
Move the metadata types and their pure fetch coroutines into io-oauth (`rfc8414`, `rfc9728` modules), leaving the orchestration here: the issuer and resource passes of composition call io-oauth's coroutines. OAuth first, being pure HTTP; the SRV-heavy JMAP and DAV discovery would follow only once a shared DNS/SRV want exists. Autoconfig, ISPDB and the provider rules never move. Breaking for both crates and for every consumer naming `io_pim_discovery::rfc8414` (Ortie's wizard).
