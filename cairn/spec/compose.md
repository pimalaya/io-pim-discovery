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
For the remaining mechanisms, composition SHALL run them in a fixed priority order (MX-derived provider, PACC, autoconfig ISP main / fallback / mailconf / ISPDB, RFC 6186 SRV, RFC 6764 CalDAV/CardDAV, RFC 8620 JMAP), scoped to the services requested, and merge their configs into one ranked list.

### Requirement: Composition entry points
The client SHALL expose `compose_all` (every reachable config), `compose_first` (the first mechanism, in priority order, that yields one), and `compose_raw` (per-mechanism output without merging). Each mechanism is also reachable directly (`autoconfig`, `srv`, `pacc`, `dav`, `jmap`, `auth`, `oauth_server`, `oauth_resource`).

### Requirement: Deadline-bounded composition
`compose_all_within` SHALL run each mechanism on its own detached thread and return only the configs that completed within a caller-supplied timeout. Mechanisms still running at the deadline are abandoned; they finish in the background and their output is dropped. This keeps an interactive caller responsive: a single unreachable endpoint (a firewalled port, a black-hole host) does not stall the whole fan-out until the OS connect timeout expires.

### Requirement: Auth refinement
A composed config's advertised auth MAY be refined against a live `WWW-Authenticate` probe: probed schemes replace account-level claims (a password claim drops when only bearer is challenged), while an OAuth issuer is preserved.

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
