---
cairn: delta
change: dav-root-resolution
---

# Delta

## ADDED Requirements

### Requirement: Advertised DAV roots are resolved
Composition SHALL probe every CalDAV or CardDAV config whose HTTP endpoint carries no path and that no RFC 6764 resolution produced, at `/.well-known/{caldav,carddav}` (RFC 6764 §5), once per origin and service, without credentials. When the probe ends on a DAV server, the config's endpoint SHALL become that context root and the config SHALL be marked resolved; otherwise it SHALL stay as advertised. `compose_raw`, `compose_all`, `compose_first` and `compose_all_within` SHALL run it, the last one within its deadline. In secure-only mode the probe SHALL NOT follow a redirect to `http://`.

#### Scenario: A provider advertises a bare DAV host
- GIVEN a configuration document naming `https://carddav.example.com`, whose `/` is not a DAV collection and whose `/.well-known/carddav` redirects to `/dav/addressbooks`
- WHEN discovery runs
- THEN the config's endpoint is `https://carddav.example.com/dav/addressbooks` and it is marked resolved

#### Scenario: An origin without redirect
- GIVEN an advertised bare DAV host whose well-known URI does not redirect
- WHEN discovery runs
- THEN the config stays as advertised and is not marked resolved

## MODIFIED Requirements

### Requirement: One JSON shape
Every config SHALL serialize with camelCase keys, its `source` as a string naming the mechanism that produced it, a `resolved` flag (true when its endpoint came out of an RFC 6764 resolution), and, when a fixed provider rule matched, a `provider` field naming the provider.

### Requirement: Ordered mechanism fan-out
For the remaining mechanisms, composition SHALL run them in a fixed priority order (MX-derived provider, PACC, autoconfig ISP main / fallback / mailconf / ISPDB, RFC 6186 SRV, RFC 6764 CalDAV/CardDAV, RFC 8620 JMAP), scoped to the services requested. `compose_raw`, and the CLI that prints it, SHALL concatenate their configs into one list in that order without deduplicating: each stays the answer of one source, and an endpoint named by several sources appears once per source, merging being the consumer's. `compose_all` and `compose_first` SHALL merge them into one ranked list for callers wanting a single answer (a setup wizard); when they merge a subdomain with its parent host, a resolved endpoint SHALL win over an unresolved one, and between two of the same kind the parent host SHALL win.

## REMOVED Requirements

None.
