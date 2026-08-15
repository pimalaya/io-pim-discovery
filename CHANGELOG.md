# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-08-15

### Changed

- Bumped io-http to 0.5. The coroutines take and yield its types, so a consumer bumps in step for a single version to resolve.

- Bumped pimalaya-stream to 0.3, whose `Read` and `Write` retry a stream reporting it is not ready. **Behaviour change.**

  A blocking socket is not supposed to report `EAGAIN`, yet callers saw one surface mid-exchange and end the exchange with a bare `Resource temporarily unavailable (os error 35)`, macOS especially and the more readily the longer the exchange ran. The transport now retries such a failure for a minute before giving up with a `TimedOut` naming the budget, and arms a socket read deadline at connect time so a server going silent on a healthy connection stops blocking the caller forever. Its `StreamStd` is renamed `stream::Stream` and its connects take a per-transport options struct, which is what this crate now calls.

## [0.6.0] - 2026-08-15

### Changed

- Bumped io-http to 0.4 and pimalaya-stream to 0.2. **Breaking.**

  The `Tls` type taken by every client's `with_tls` comes from pimalaya-stream 0.2, so a consumer bumps in step for a single version to resolve.

- Raised the minimum supported Rust version from 1.87 to 1.88.

## [0.5.0] - 2026-08-07

### Changed

- Bumped pimalaya-cli to 0.2, which re-exports comfy-table 8 instead of 7. **Breaking.**

  `cli::common::table` returns a comfy-table 8 `Table`, a distinct type from the 7 one. Callers styling it replace `load_preset(&str)` with `load_style(TableStyle)`, and read the `presets::*` constants as `TableStyle` values rather than strings.

## [0.4.0] - 2026-08-07

### Added

- Added `compose_all_within`, a deadline-bounded variant of `compose_all`.

  Each mechanism runs on its own detached thread and only the configs completing within the timeout are returned, so one unreachable endpoint no longer stalls an interactive wizard until the operating system connect timeout expires.

### Fixed

- Added the missing `_submissions._tcp` SRV lookup (RFC 8314) to mail service discovery.

  Only `_submission._tcp` (STARTTLS, port 587) was queried on the send side, so a domain publishing just the implicit-TLS variant got its IMAP endpoint discovered and no SMTP one. `DiscoverySrv` runs the fourth lookup, `DiscoverySrvReport` carries a `submissions` slot, and `from_srv` maps it to an implicit-TLS SMTP config.

## [0.3.3] - 2026-07-17

### Fixed

- Corrected the Microsoft IMAP/POP/SMTP OAuth scopes to use the `https://outlook.office.com/` resource.

  `https://outlook.office365.com/` is the server host, not a valid scope resource, so Microsoft's authorize endpoint rejected it with `invalid_scope`. The server hosts themselves are unchanged.

## [0.3.2] - 2026-07-16

### Fixed

- Restored all DNS-based discovery (SRV lookups, MX provider detection, PACC DNS-TXT digest verification), broken since 0.3.0.

  The `domain` 0.12.2 `unstable-new` parser rejects relative names, and every query name was built without a trailing dot, so each lookup failed and its mechanism was silently skipped. Query names are made absolute before parsing. This notably restores OAuth issuer discovery for providers advertising it only through PACC, such as Fastmail.

## [0.3.1] - 2026-07-16

### Fixed

- Release builds in CI.

## [0.3.0] - 2026-07-16

### Changed

- Renamed every public coroutine, client, error and data type to carry the strict `Discovery` prefix, aligning with the Pimalaya naming guidelines.

  For example `ComposeClientStd` became `DiscoveryComposeClientStd`; `ResolveDav`, `ResolveJmap`, `ResolveOauthServer` and `ResolveOauthResource` became `DiscoveryDavResolve`, `DiscoveryJmapResolve`, `DiscoveryOauthServerResolve` and `DiscoveryOauthResourceResolve`; `WellKnown` became `DiscoveryWellKnown`; `ProbeAuth` became `DiscoveryProbeAuth`; `ConfigCollector` became `DiscoveryConfigCollector`; and the shared data types `Service`, `ServiceConfig`, `AuthMethod`, `DavService`, `OauthServerMetadata` and `OauthResourceMetadata` gained the same prefix. The wire-format schema types were prefixed too: the autoconfig XML structs (`DiscoveryAutoconfig`, `DiscoveryEmailProvider`, `DiscoveryServer`, `DiscoveryServerType`, `DiscoverySecurityType`, `DiscoveryAuthenticationType`, …), the PACC JSON structs (`DiscoveryPaccConfig`, `DiscoveryProtocols`, `DiscoveryAuthentication`, `DiscoveryProvider`, …), and `DiscoverySrvReport`, `DiscoverySrvService`, `DiscoveryWebdavSrvReport` and `DiscoveryJmapSessionResource`. The compose config model was prefixed as well (`Endpoint`, `Security`, `ConfigSource` became `DiscoveryEndpoint`, `DiscoverySecurity`, `DiscoveryConfigSource`), the stream-pool trait `Stream` became `DiscoveryStream`, and the known-provider enum `Provider` became `DiscoveryKnownProvider` (kept distinct from the PACC `DiscoveryProvider`). Only the CLI command types keep their unprefixed names.

- Moved each mechanism's data types out of a `types` catch-all into a named public module, path-visible with no re-export.

  The autoconfig, PACC and composed-config schemas live in `autoconfig::config`, `pacc::config` and `compose::config`, and the SRV and DAV service types in `rfc6186::service` and `rfc6764::service`. So `autoconfig::types::EmailProvider` is now `autoconfig::config::EmailProvider`.

- Switched the DNS coroutines to the `domain` 0.12.2 `unstable-new` SRV API, dropping the git patch that pinned the unreleased revision.

  The owned answer aliases are now `TxtRecord`, `SrvRecord` and `MxRecord`, all with public fields instead of accessor methods.

- Bumped io-http to 0.3, pimalaya-stream to 0.1 and pimalaya-cli to 0.1.

- Documented every public item and aligned the crate with the Pimalaya documentation guidelines.

  The src/lib.rs architecture header replaced the README include, the README dropped its inline code, and docs.rs builds with all features.

### Fixed

- Boxed the oversized HTTP coroutines held by the DNS-over-HTTPS and JMAP well-known state machines, clearing the `clippy::large_enum_variant` warnings.

## [0.2.0] - 2026-07-13

### Added

- Added the unified `compose` orchestrator (`ComposeClientStd`), turning one email or domain into a `ServiceConfig` list.

  It chains provider rules, PACC, autoconfig, RFC 6186 SRV, RFC 6764 DAV and RFC 8620 JMAP. `compose_all` merges across mechanisms, `compose_first` keeps the highest-priority hit, `compose_raw` returns them unmerged.

- Added RFC 8620 JMAP autodiscovery behind the `rfc8620` feature.

  `ResolveJmap` chains a `_jmap._tcp` SRV lookup and a `/.well-known/jmap` probe, following redirects and judging the terminal 2xx or 401.

- Added RFC 8484 DNS-over-HTTPS, so every DNS mechanism accepts a DoH resolver.

  `DnsExchange` picks `tcp://` length-framing or an `https://…/dns-query` POST from the resolver URL, and the CLI `--server` flags take a URL too.

- Added a per-endpoint authentication probe (`rfc9110` module, `ProbeAuth`).

  It reads `WWW-Authenticate` on an unauthenticated 401 to refine each config's `password` and `bearer` methods, leaving OAuth methods untouched.

- Added the `Bearer` authentication method, detected from the JMAP session probe.
- Added the OAuth 2.0 metadata modules `rfc8414` (authorization server) and `rfc9728` (protected resource), moved from io-oauth.

  They bring the `ResolveOauthServer` and `ResolveOauthResource` coroutines, `ComposeClientStd::oauth_server` and `oauth_resource`, and the CLI `auth server` and `auth resource` commands.

- Added automatic OAuth issuer resolution.

  `compose` fetches a discovered `OauthIssuer`'s RFC 8414 metadata and upgrades it to a concrete `OauthAuthorizationCodeGrant`, plus a device grant when advertised.

### Changed

- Renamed the crate from `pimconf` to `io-pim-discovery`, library path `io_pim_discovery` and CLI binary `pim-discovery`.
- Gated the CLI behind a non-default `cli` feature.
- Made `compose` plain library code instead of a feature.

  It lives behind `stream` plus at least one discovery mechanism, and composes whichever mechanisms are enabled, skipping the rest.

- Organised the CLI by PIM domain (`all`, `email`, `calendar`, `contact`, `file`, `auth`) instead of by mechanism.

  The old flat `autoconfig`, `pacc`, `srv`, `webdav` and `search` commands are gone, provider detection is `email is-google` and `email is-microsoft`, and mechanisms are shown independently since the CLI never merges.

- Replaced the serial `SearchAll` and `SearchFirst` coroutines with bricks orchestrated by `ComposeClientStd`.

  The bricks are the pure `ConfigCollector` plus the per-mechanism coroutines, run one thread per mechanism and one probe per config.

- Switched the DNS coroutines from the unreleased `domain` new API to the stable release, dropping the git patch and unblocking releases.
- Made the DNS coroutines honor the EOF convention, an empty resume slice ending them with an `Eof` error instead of yielding reads forever on a dead stream.
- Made the RFC 6764 resolve fall back to the `.well-known` probe when the SRV lookup fails.

### Fixed

- Deduplicated a service reached under two names.

  HTTP endpoints compare as normalized URLs, and a subdomain host merges into its parent, as fastmail's rotated CardDAV shards need.

- Fixed the assumed JMAP authentication order when the endpoint advertises no scheme, bearer first and password second.
- Fixed the PACC `oauth-public` / `content-type` keys not deserializing from their wire names, which silently dropped a provider's OAuth issuer.

## [0.1.0] - 2026-06-06

### Added

- Added Thunderbird Autoconfig support (requires `autoconfig` feature).

- Added [PACC] support (requires `pacc` feature).

  [PACC]: https://www.ietf.org/archive/id/draft-ietf-mailmaint-pacc-02.html

- Added [RFC 6186] SRV-based mail service discovery (requires `rfc6186` feature).

  [RFC 6186]: https://datatracker.ietf.org/doc/html/rfc6186

- Added [RFC 6764] SRV-based CalDAV/CardDAV discovery (requires `rfc6764` feature).

  [RFC 6764]: https://datatracker.ietf.org/doc/html/rfc6764

- Added CLI (requires `cli` feature).

[unreleased]: https://github.com/pimalaya/io-pim-discovery/compare/v0.7.0..HEAD
[0.7.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.6.0..v0.7.0
[0.6.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.5.0..v0.6.0
[0.5.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.4.0..v0.5.0
[0.4.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.3.3..v0.4.0
[0.3.3]: https://github.com/pimalaya/io-pim-discovery/compare/v0.3.2..v0.3.3
[0.3.2]: https://github.com/pimalaya/io-pim-discovery/compare/v0.3.1..v0.3.2
[0.3.1]: https://github.com/pimalaya/io-pim-discovery/compare/v0.3.0..v0.3.1
[0.3.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.2.0..v0.3.0
[0.2.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.1.0..v0.2.0
[0.1.0]: https://github.com/pimalaya/io-pim-discovery/compare/root..v0.1.0
