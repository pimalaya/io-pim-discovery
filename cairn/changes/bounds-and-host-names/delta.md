---
cairn: delta
change: bounds-and-host-names
---

# Delta

## ADDED Requirements

### Requirement: The domain is a host name
(compose) Composition SHALL parse the domain of its input, an address or a bare domain, as a host name once, and use its lowercased ASCII form for every lookup and URL. A domain that is not a host name (one carrying a port, a path, a query or a user part, or an IP address) SHALL fail `check_resolver` and every `compose_*` with an error naming it, before any request, and SHALL make a single-mechanism entry point yield nothing.

#### Scenario: A domain carrying a port and a path
- GIVEN the input `anna@example.org:8443/x?`
- WHEN `compose_raw` runs
- THEN it returns an error naming the domain, and no request is made

### Requirement: URLs take host names and encoded values
(mechanisms) Every URL a mechanism builds from a domain, an SRV target or an alias target SHALL take it as a host name and fail otherwise, so that no port, path, query or user part enters through it: the autoconfig URL helpers, the PACC well-known URL, and the origins of the RFC 6764 and RFC 8620 resolutions. The autoconfig `emailaddress` parameter SHALL carry the address form-encoded, as one value.

#### Scenario: A plus sign in the local part
- GIVEN the address `anna+lists@example.org`
- WHEN the ISP main URL is built
- THEN its `emailaddress` parameter decodes to `anna+lists@example.org`

## MODIFIED Requirements

### Requirement: Deadline-bounded composition
(compose) `compose_all_within` SHALL run each mechanism on its own detached thread and return only the configs that completed within a caller-supplied timeout. Mechanisms still running at the deadline are abandoned; they finish in the background and their output is dropped. This keeps an interactive caller responsive: a single unreachable endpoint (a firewalled port, a black-hole host) does not stall the whole fan-out until the OS connect timeout expires. The passes that follow (the DAV root resolution, the auth refinement and the OAuth resolution) SHALL run within what is left of the timeout: a task of theirs still running at the deadline is abandoned and its config stays as it was, and no pass SHALL start once no time is left.

### Requirement: Std clients over a stream pool
(coroutine) The client layer SHALL drive each coroutine end-to-end through a `DiscoveryStreamPool` that opens and reuses one stream per endpoint URL, with pluggable per-scheme factories (`tcp` for DNS, `https` for HTTP mechanisms). SRV discovery never opens HTTPS, so its client needs only the `tcp` factory. The `tcp` factory SHALL bound each connect, read and write by a timeout, ten seconds unless the caller registers another.

## REMOVED Requirements
