---
cairn: spec
capability: compose
status: current
---

# Composition

The `compose` module reduces the individual mechanisms into a single ranked list of service configurations, so a caller (a setup wizard) gets one answer instead of orchestrating each probe.

### Requirement: Provider short-circuit first
Composition SHALL reduce the fixed-provider rules first: when the email domain (or its MX records) matches a known provider (Google, Microsoft), the provider's own configs are produced and tag their source as `Provider`. Provider detection is exposed on its own (`provider`, `is_google`, `is_microsoft`).

### Requirement: Ordered mechanism fan-out
For the remaining mechanisms, composition SHALL run them in a fixed priority order (MX-derived provider, PACC, autoconfig ISP main / fallback / mailconf / ISPDB, RFC 6186 SRV, RFC 6764 CalDAV/CardDAV, RFC 8620 JMAP), scoped to the services requested. `compose_raw`, and the CLI that prints it, SHALL concatenate their configs into one list in that order without deduplicating: each stays the answer of one source, and an endpoint named by several sources appears once per source, merging being the consumer's. `compose_all` and `compose_first` SHALL merge them into one ranked list for callers wanting a single answer (a setup wizard); when they merge a subdomain with its parent host, a resolved endpoint SHALL win over an unresolved one, and between two of the same kind the parent host SHALL win.

### Requirement: Composition entry points
The client SHALL expose `compose_all` (every reachable config), `compose_first` (the first mechanism, in priority order, that yields one), and `compose_raw` (per-mechanism output without merging). Each mechanism is also reachable directly (`autoconfig`, `srv`, `pacc`, `dav`, `jmap`, `auth`, `oauth_server`, `oauth_resource`).

### Requirement: Deadline-bounded composition
`compose_all_within` SHALL run each mechanism on its own detached thread and return only the configs that completed within a caller-supplied timeout. Mechanisms still running at the deadline are abandoned; they finish in the background and their output is dropped. This keeps an interactive caller responsive: a single unreachable endpoint (a firewalled port, a black-hole host) does not stall the whole fan-out until the OS connect timeout expires.

### Requirement: Auth refinement
A composed config's advertised auth MAY be refined against a live `WWW-Authenticate` probe: probed schemes replace account-level claims (a password claim drops when only bearer is challenged), while an OAuth issuer is preserved.

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

### Requirement: Provider API services
The service kinds SHALL include the providers' own APIs: `gmail` (Gmail API), `gcal` (Google Calendar), `gpeople` (Google People), `msgraph` (Microsoft Graph mail), `msgraphCalendar` and `msgraphContacts`. The Google fixed rule SHALL yield `gmail`, `gcal` and `gpeople` and the Microsoft fixed rule `msgraph`, `msgraphCalendar` and `msgraphContacts`, each with its API base URL and its OAuth scope, next to the protocol services the rules already yield, so a consumer can offer the choice.

#### Scenario: A Google address offers the API or the protocol
- GIVEN a `gmail.com` address
- WHEN every config is composed
- THEN both `gmail` and `imap` are offered for mail, `gcal` and `caldav` for calendars, `gpeople` and `carddav` for contacts

#### Scenario: A Microsoft address has calendars and contacts
- GIVEN an `outlook.com` address
- WHEN every config is composed
- THEN `msgraphCalendar` and `msgraphContacts` are offered

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

### Requirement: Secure-only discovery
The client SHALL offer a secure-only mode (`--secure-only` on the CLI), off by default. In it, no request SHALL go over plain HTTP (autoconfig's `http://` URLs, a mailconf target or a redirect to `http://`), RFC 6764 SHALL NOT build an `http://` origin, and no config whose endpoint is unencrypted (`plain` security or an `http://` URL) SHALL be returned. STARTTLS endpoints SHALL be kept. DNS lookups SHALL still go to the configured resolver, plain TCP or RFC 8484 over `https`; a plain `http://` resolver SHALL be refused with an error before any request.

#### Scenario: A plain autoconfig is not fetched
- GIVEN a domain serving its autoconfig over `http://` only
- WHEN discovery runs with `--secure-only`
- THEN no request is made to the `http://` URL and no config comes from it

### Requirement: One JSON shape
Every config SHALL serialize with camelCase keys, its `source` as a string naming the mechanism that produced it, a `resolved` flag (true when its endpoint came out of an RFC 6764 resolution), and, when a fixed provider rule matched, a `provider` field naming the provider.

#### Scenario: A provider config and an autoconfig one read alike
- GIVEN a Google address and a domain with an autoconfig document
- WHEN `pim-discovery --json all` runs on each
- THEN both print `source` as a string, the Google one with `provider: "google"`, and no key holds an underscore
