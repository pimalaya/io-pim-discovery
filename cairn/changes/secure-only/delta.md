---
cairn: delta
change: secure-only
---

# Delta

## ADDED Requirements

### Requirement: Secure-only discovery
The client SHALL offer a secure-only mode (`--secure-only` on the CLI), off by default. In it, no request SHALL go over plain HTTP (autoconfig's `http://` URLs, a mailconf target or a redirect to `http://`), RFC 6764 SHALL NOT build an `http://` origin, and no config whose endpoint is unencrypted (`plain` security or an `http://` URL) SHALL be returned. STARTTLS endpoints SHALL be kept. DNS lookups SHALL still go to the configured resolver, plain or RFC 8484.

#### Scenario: A plain autoconfig is not fetched
- GIVEN a domain serving its autoconfig over `http://` only
- WHEN discovery runs with `--secure-only`
- THEN no request is made to the `http://` URL and no config comes from it

## MODIFIED Requirements

### Requirement: Shared resolver flag
Every command SHALL accept a DNS resolver argument (a URL or `host:port` pair) defaulting to `1.1.1.1:53` and a `--secure-only` flag, and render its discovered configs through the shared CLI printer.

## REMOVED Requirements

None.
