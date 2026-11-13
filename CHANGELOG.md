# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `DiscoveryConfigTrust`, what vouches that a config belongs to the address's domain.

  One of `Rule`, `Domain`, `ThirdParty` or `Unauthenticated`, serialized as `trust` and shown by the CLI table.

- Added `DiscoveryDavResolveOutput`, the context root and whether an SRV record named its origin.
- Added `DiscoveryJmapResolveOutput`, the session resource and whether an SRV record named its origin.
- Added `DiscoveryStreamPool::with_tcp_factory`, the `tcp` factory with a timeout of the caller's choice.

### Changed

- **BREAKING**: Changed `DiscoveryServiceConfig` to carry `trust`.

  A config read without the field counts as `unauthenticated`.

- **BREAKING**: Changed `DiscoveryOauthServerResolve::new` to take the issuer asked.
- **BREAKING**: Changed `DiscoveryOauthResourceResolve::new` to take the resource asked.
- **BREAKING**: Changed `DiscoveryOauthServerResolveError` to carry `Issuer` and `NotHttps`.
- **BREAKING**: Changed `DiscoveryOauthResourceResolveError` to carry `Resource` and `NotHttps`.
- **BREAKING**: Changed `DiscoveryDavResolve` to return `DiscoveryDavResolveOutput`.
- **BREAKING**: Changed `DiscoveryJmapResolve` to return `DiscoveryJmapResolveOutput`.
- **BREAKING**: Changed `DiscoveryServiceConfig::from_autoconfig`, `from_dav` and `from_jmap` to take the trust.
- **BREAKING**: Changed `DiscoveryKnownProvider::configs` to take the trust.
- **BREAKING**: Changed `DiscoveryComposeClientStdError` to carry `InvalidDomain`.

  `check_resolver` and every `compose_*` return it for an input whose domain is not a host name, before any request.

- Changed the single-mechanism entry points of the compose client to yield nothing for a domain that is not a host name.
- Changed `DiscoveryConfigCollector::finish` to list the encrypted endpoints first.
- Changed `DiscoveryServiceConfig::probe_urls` to return nothing for an endpoint that is not `https`.

  The auth of a cleartext endpoint is no longer refined from its own answer.

### Fixed

- Fixed a cleartext endpoint replacing an encrypted one in `compose_all` and `compose_first`.

  A subdomain merges with its parent host between two `https` endpoints only.

- Fixed OAuth metadata being used whatever issuer or resource it named.

  A document naming another one, or a URL that is not `https`, is refused (RFC 8414 §3.3, RFC 9728 §3.3).

- Fixed the redirect walk following any URL scheme.

  A redirect is followed to `https`, or to `http` from an `http` URL.

- Fixed `compose_all_within` running its auth probe and OAuth resolution past the deadline.

  Both passes get what is left of it, and none starts once nothing is left.

- Fixed the default DNS-over-TCP stream waiting without limit on a silent resolver.

  A connect, a read or a write gives up after ten seconds.

- Fixed `DiscoveryIsp::main_url` writing the address unencoded into the query.

  A `+`, `&` or `#` in the local part reaches the server as written.

- Fixed URLs being built from a domain that is not a host name.

  The `DiscoveryIsp` URL helpers, `DiscoveryPacc::url` and the DAV and JMAP resolutions fail on a domain carrying a port, a path or a query.

## [0.11.0] - 2026-11-04

### Added

- Added the RFC 9728 resolution of OAuth grants to every `compose_*`.

  A config whose HTTP endpoint publishes protected resource metadata gets its authorization servers as issuers when it had no OAuth method, and its grants carry the resource.

### Changed

- **BREAKING**: Changed `DiscoveryAuthMethod::OauthAuthorizationCodeGrant` and `OauthDeviceAuthorizationGrant` to carry `resource` and `scopes_supported`.

  The scopes come from the issuer metadata, or from the resource metadata when it lists any.

## [0.10.0] - 2026-11-03

### Added

- Added the resolution of advertised CalDAV and CardDAV roots through their well-known URI (RFC 6764 §5).

  A config with no endpoint path, from PACC, autoconfig or a provider rule, gets its DAV context root, such as `https://caldav.fastmail.com/dav/calendars`.

- Added the MX provider fallback to autoconfig.

  A domain without an autoconfig of its own is tried at the registrable domain of its MX hosts, ISP main URL then ISPDB.

- Added CNAME following to the autoconfig ISP main URL.

  A failing `autoconfig.<domain>` alias is retried on its target, so a domain hosted at Migadu discovers `imap.migadu.com`.

- Added `DiscoveryDnsCname`, the CNAME chain lookup.
- Added `DiscoveryIsp::host_url`.
- Added `mx_base_domain`, the registrable domain of an MX host.

### Changed

- **BREAKING**: Changed `DiscoveryServiceConfig` to carry `resolved`, true when an RFC 6764 resolution produced the endpoint.
- **BREAKING**: Changed `DiscoveryConfigSource` to carry `Mx`, serialized as `mx`.
- **BREAKING**: Changed a DNS resolver that does not answer into an error.

  Every `compose_*` and CLI command first runs `check_resolver`: no answer, `SERVFAIL`, `REFUSED` or a DNS-over-HTTPS HTTP error fails, naming the resolver. `NXDOMAIN` is still an answer.

- Changed `compose_all` and `compose_first` to keep a resolved endpoint over an unresolved one when merging a subdomain with its parent host.
- Changed secure-only mode to refuse a plain `http://` DNS resolver.
- Changed the minimum Rust version to 1.89.
- Replaced the base64 crate with [base64-codec](https://crates.io/crates/base64-codec) for the PACC digest.

  The CLI installs the detected CPU token first, so its base64 runs on SIMD.

- Changed the io-http requirement to 0.6.
- Changed the pimalaya-cli requirement to 0.3.

### Removed

- **BREAKING**: Removed `mx_parent_domain`, replaced by `mx_base_domain`.

  It could reach the parent of a public suffix, such as `autoconfig.co.uk`.

### Fixed

- Fixed discovery probes reading unbounded responses and spinning on a connection closed mid-body.

  io-http 0.6 caps response heads at 64 KiB and bodies at 256 MiB.

### Security

- Raised the pimalaya-stream requirement to 0.3.1, fixing RUSTSEC-2026-0346 to RUSTSEC-2026-0349.

  It requires rustls 0.23.46.

## [0.9.0] - 2026-10-04

### Added

- Added the Gmail API, Google Calendar, Google People and Microsoft Graph service kinds.

  The Google and Microsoft provider rules offer them next to the protocols, so a Microsoft address now has calendars and contacts.

- Added a secure-only mode, `with_secure_only` on the compose client and `--secure-only` on the CLI.

  It makes no plain HTTP request, builds no `http://` CalDAV/CardDAV origin and drops unencrypted endpoints, keeping STARTTLS.

### Changed

- Changed `email is-google` and `email is-microsoft` to list the provider configs of every domain.

- Changed the JSON of a service config. **Breaking.**

  `source` is always a string, a fixed rule's provider moves to a `provider` field, and the OAuth method fields are camelCase.

## [0.8.0] - 2026-09-28

### Added

- Added the `DiscoveryDavContext` coroutine.

  Checks that a URL leads to a CalDAV or CardDAV server with an unauthenticated `PROPFIND`.

- Added the `DiscoveryHttpWalk` coroutine.

  Follows a redirect chain for the `.well-known` and auth probes.

### Changed

- Stopped assuming bearer and password in `DiscoveryServiceConfig::from_jmap` when the session advertised no scheme. **Behaviour change.**

  The auth list stays empty and the consumer decides what to offer.

### Fixed

- Stopped reporting a web page as a JMAP session resource.

  A 2xx now needs a session object with the core capability and an `apiUrl`, and a 401 a `WWW-Authenticate` challenge ([himalaya#745](https://github.com/pimalaya/himalaya/issues/745)).

- Stopped reporting a URL without a DAV server as a CalDAV or CardDAV context root. **Behaviour change.**

  Each candidate must answer a `PROPFIND` with a 207 or a challenged 401, otherwise `DiscoveryDavResolve` fails with the new `NotFound` error.

## [0.7.0] - 2026-08-15

### Changed

- Bumped io-http to 0.5.

  The coroutines take and yield its types, so consumers bump in step.

- Bumped pimalaya-stream to 0.3. **Behaviour change.**

  A stream reporting it is not ready is retried for a minute instead of failing with `os error 35`, and reads time out on a silent server.

## [0.6.0] - 2026-08-15

### Changed

- Bumped io-http to 0.4 and pimalaya-stream to 0.2. **Breaking.**

  The `Tls` type taken by `with_tls` comes from pimalaya-stream 0.2, so consumers bump in step.

- Raised the minimum supported Rust version from 1.87 to 1.88.

## [0.5.0] - 2026-08-07

### Changed

- Bumped pimalaya-cli to 0.2. **Breaking.**

  `cli::common::table` returns a comfy-table 8 `Table`, styled with `load_style(TableStyle)` instead of `load_preset(&str)`.

## [0.4.0] - 2026-08-07

### Added

- Added `compose_all_within`, a deadline-bounded `compose_all`.

  Mechanisms still running at the deadline are abandoned, so one unreachable endpoint no longer stalls an interactive wizard.

### Fixed

- Added the missing `_submissions._tcp` SRV lookup (RFC 8314).

  A domain publishing only implicit-TLS submission now gets its SMTP config.

## [0.3.3] - 2026-07-17

### Fixed

- Corrected the Microsoft IMAP, POP and SMTP OAuth scopes to the `https://outlook.office.com/` resource.

## [0.3.2] - 2026-07-16

### Fixed

- Restored DNS-based discovery (SRV, MX provider detection, PACC digest), broken since 0.3.0.

  Query names are now absolute, as the `domain` 0.12.2 parser requires.

## [0.3.1] - 2026-07-16

### Fixed

- Fixed release builds in CI.

## [0.3.0] - 2026-07-16

### Changed

- Renamed every public type with the `Discovery` prefix. **Breaking.**

  For example `ComposeClientStd` became `DiscoveryComposeClientStd` and `WellKnown` became `DiscoveryWellKnown`; only the CLI command types keep their names.

- Moved the data types out of the `types` modules into named public modules.

  For example `autoconfig::types::EmailProvider` is now `autoconfig::config::EmailProvider`.

- Switched the DNS coroutines to the `domain` 0.12.2 SRV API.

  The owned answer aliases are `TxtRecord`, `SrvRecord` and `MxRecord`, with public fields.

- Bumped io-http to 0.3, pimalaya-stream to 0.1 and pimalaya-cli to 0.1.

- Documented every public item.

### Fixed

- Boxed the oversized HTTP coroutines of the DNS-over-HTTPS and JMAP well-known state machines.

## [0.2.0] - 2026-07-13

### Added

- Added the `compose` orchestrator (`ComposeClientStd`).

  Turns an email or domain into a `ServiceConfig` list by chaining provider rules, PACC, autoconfig, SRV, DAV and JMAP.

- Added RFC 8620 JMAP discovery (requires `rfc8620` feature).

- Added RFC 8484 DNS-over-HTTPS resolvers.

- Added the `ProbeAuth` authentication probe (`rfc9110` module).

  Refines each config's `password` and `bearer` methods from `WWW-Authenticate`.

- Added the `Bearer` authentication method.

- Added the `rfc8414` and `rfc9728` OAuth metadata modules, moved from io-oauth.

- Added OAuth issuer resolution.

  A discovered `OauthIssuer` is upgraded to concrete authorization-code and device grants.

### Changed

- Renamed the crate from `pimconf` to `io-pim-discovery`, and its binary to `pim-discovery`.

- Gated the CLI behind the non-default `cli` feature.

- Made `compose` plain library code instead of a feature.

- Organised the CLI by PIM domain (`all`, `email`, `calendar`, `contact`, `file`, `auth`).

- Replaced the serial `SearchAll` and `SearchFirst` coroutines with `ComposeClientStd`.

- Switched the DNS coroutines to the stable `domain` release.

- Made the DNS coroutines end with an `Eof` error on an empty resume.

- Made the RFC 6764 resolve fall back to the `.well-known` probe when the SRV lookup fails.

### Fixed

- Deduplicated a service reached under two names, such as fastmail's CardDAV shards.

- Fixed the assumed JMAP authentication order when the endpoint advertises no scheme, bearer first.

- Fixed the PACC `oauth-public` and `content-type` keys not deserializing.

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

[unreleased]: https://github.com/pimalaya/io-pim-discovery/compare/v0.11.0..HEAD
[0.11.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.10.0..v0.11.0
[0.10.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.9.0..v0.10.0
[0.9.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.8.0..v0.9.0
[0.8.0]: https://github.com/pimalaya/io-pim-discovery/compare/v0.7.0..v0.8.0
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
