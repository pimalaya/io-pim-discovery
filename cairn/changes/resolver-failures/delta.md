---
cairn: delta
change: resolver-failures
---

# Delta

## ADDED Requirements

### Requirement: Resolver failures are errors
Before running any mechanism, composition SHALL ask the resolver one question about the address's domain (`check_resolver`). An answer with response code `NOERROR` or `NXDOMAIN` SHALL let discovery proceed; no answer, an unparseable answer or any other response code SHALL fail discovery with an error naming the resolver, instead of an empty or partial list. `compose_all_within` SHALL bound the probe by its deadline. Failures of a single mechanism SHALL still be skipped.

#### Scenario: An unreachable resolver
- GIVEN a resolver that refuses connections
- WHEN `compose_raw` runs for any address, `gmail.com` included
- THEN it returns an error naming the resolver, not a list

#### Scenario: A domain that does not exist
- GIVEN a working resolver
- WHEN discovery runs for an address whose domain does not exist
- THEN the probe passes (`NXDOMAIN` is an answer) and the result is the mechanisms' own

## MODIFIED Requirements

### Requirement: Secure-only discovery
The client SHALL offer a secure-only mode (`--secure-only` on the CLI), off by default. In it, no request SHALL go over plain HTTP (autoconfig's `http://` URLs, a mailconf target or a redirect to `http://`), RFC 6764 SHALL NOT build an `http://` origin, and no config whose endpoint is unencrypted (`plain` security or an `http://` URL) SHALL be returned. STARTTLS endpoints SHALL be kept. DNS lookups SHALL still go to the configured resolver, plain TCP or RFC 8484 over `https`; a plain `http://` resolver SHALL be refused with an error before any request.

### Requirement: Shared resolver flag
Every command SHALL accept a DNS resolver argument (a URL or `host:port` pair) defaulting to `1.1.1.1:53` and a `--secure-only` flag, check the resolver before discovering (an unusable resolver is an error, exit 1), and render its discovered configs through the shared CLI printer.

## REMOVED Requirements

None.
