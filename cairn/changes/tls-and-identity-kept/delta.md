---
cairn: delta
change: tls-and-identity-kept
---

# Delta

## ADDED Requirements

### Requirement: Config trust
(compose) Every config SHALL carry a `trust` telling what vouches that it belongs to the address's domain: `rule` when a fixed provider rule matched the domain itself; `domain` when the domain answered over TLS under its own name (PACC, the autoconfig ISP main and fallback URLs, an RFC 6764 or RFC 8620 resolution whose origin is the domain); `thirdParty` when a fixed third party answered over TLS (the ISPDB entry of the domain); `unauthenticated` when a DNS answer named the host or an exchange ran in clear (a provider matched through MX records, the alias fallback of the ISP main URL, the mailconf target, the MX provider's autoconfig, RFC 6186 SRV records, an RFC 6764 or RFC 8620 resolution whose origin an SRV record named). A config read without the field SHALL count as `unauthenticated`. Discovery SHALL NOT check an `unauthenticated` endpoint against the domain nor ask for a confirmation: that duty (RFC 6186 §6) is the consumer's, before any credential is sent.

#### Scenario: An aliased autoconfig
- GIVEN a domain whose `autoconfig.<domain>` is an alias of a provider's host serving a certificate for its own name
- WHEN email discovery runs
- THEN the configs are tagged `IspMain` with `trust` `unauthenticated`, where a document served by `autoconfig.<domain>` itself is `domain`

### Requirement: Redirects stay on HTTP and never leave TLS
(coroutine) The shared redirect walk SHALL follow a redirect to an `https` URL, and to an `http` URL only from an `http` one. A redirect to any other scheme, or from `https` to `http`, SHALL end the walk without a result, as a looping chain does.

#### Scenario: A redirect to a raw TCP URL
- GIVEN a probed server answering `Location: tcp://10.0.0.5:6379/`
- WHEN the walk reads it
- THEN no request is written to that URL and the walk ends without a result

## MODIFIED Requirements

### Requirement: Ordered mechanism fan-out
(compose) For the remaining mechanisms, composition SHALL run them in a fixed priority order (MX-derived provider, PACC, autoconfig ISP main / fallback / mailconf / ISPDB / MX provider, RFC 6186 SRV, RFC 6764 CalDAV/CardDAV, RFC 8620 JMAP), scoped to the services requested. `compose_raw`, and the CLI that prints it, SHALL concatenate their configs into one list in that order without deduplicating: each stays the answer of one source, and an endpoint named by several sources appears once per source, merging being the consumer's. `compose_all` and `compose_first` SHALL merge them into one ranked list for callers wanting a single answer (a setup wizard), the encrypted endpoints first and each group in mechanism order; when they merge a subdomain with its parent host, a resolved endpoint SHALL win over an unresolved one, and between two of the same kind the parent host SHALL win, the source and the trust following the endpoint kept. A subdomain SHALL merge with its parent host between two `https` endpoints only: an unencrypted endpoint SHALL neither merge with nor replace an encrypted one.

#### Scenario: A plain SRV record next to an advertised TLS host
- GIVEN a document advertising `https://carddav.example.com` and a plain `_carddav` record resolving to `http://d1.carddav.example.com/dav/`
- WHEN every config is composed
- THEN both configs are returned, the `https` one first

### Requirement: Auth refinement
(compose) A composed config's advertised auth MAY be refined against a live `WWW-Authenticate` probe of its `https` endpoint: probed schemes replace account-level claims (a password claim drops when only bearer is challenged), while an OAuth issuer is preserved. An endpoint that is not `https` SHALL NOT be probed, and its config SHALL keep the auth its mechanism reported.

### Requirement: OAuth resolution
(compose) Composition SHALL resolve every OAuth issuer through its RFC 8414 metadata into the grants it advertises (the authorization code grant, and the device grant when a device authorization endpoint is named), each carrying the metadata's `scopes_supported`, leaving an unresolvable issuer as it was. With `rfc9728`, it SHALL first read the protected resource metadata of each distinct HTTP endpoint: a config carrying no OAuth method SHALL gain an issuer per authorization server named, and after issuer resolution every OAuth grant of the config SHALL carry the metadata's `resource`, with its `scopes_supported` when it lists any. An endpoint without metadata SHALL leave its configs as they were. Metadata SHALL be used only when it names what was asked: an issuer's metadata whose `issuer` is that issuer, a resource's metadata whose `resource` is the endpoint URL as fetched. Metadata SHALL be fetched over `https` only, and a document naming an endpoint or an authorization server that is not `https` SHALL be refused. Refused metadata counts as none.

#### Scenario: A JMAP session publishing its resource metadata
- GIVEN a JMAP config whose session URL serves RFC 9728 metadata naming an authorization server and scopes
- WHEN composition resolves OAuth
- THEN the config's authorization code grant carries the session URL as `resource` and the resource's scopes

#### Scenario: An endpoint naming another resource
- GIVEN a config whose endpoint serves RFC 9728 metadata naming another URL as `resource`
- WHEN composition resolves OAuth
- THEN the config is left as it was: no issuer is added and no grant is bound

### Requirement: Advertised DAV roots are resolved
(compose) Composition SHALL probe every CalDAV or CardDAV config whose HTTP endpoint carries no path and that no RFC 6764 resolution produced, at `/.well-known/{caldav,carddav}` (RFC 6764 §5), once per origin and service, without credentials. When the probe ends on a DAV server, the config's endpoint SHALL become that context root and the config SHALL be marked resolved; otherwise it SHALL stay as advertised. `compose_raw`, `compose_all`, `compose_first` and `compose_all_within` SHALL run it, the last one within its deadline. The probe SHALL NOT follow a redirect from `https` to `http://`, nor, in secure-only mode, any redirect to `http://`. A root found through an origin that is not `https` SHALL mark its config `unauthenticated`.

#### Scenario: A provider advertises a bare DAV host
- GIVEN a configuration document naming `https://carddav.example.com`, whose `/` is not a DAV collection and whose `/.well-known/carddav` redirects to `/dav/addressbooks`
- WHEN discovery runs
- THEN the config's endpoint is `https://carddav.example.com/dav/addressbooks` and it is marked resolved

#### Scenario: An origin without redirect
- GIVEN an advertised bare DAV host whose well-known URI does not redirect
- WHEN discovery runs
- THEN the config stays as advertised and is not marked resolved

### Requirement: One JSON shape
(compose) Every config SHALL serialize with camelCase keys, its `source` as a string naming the mechanism that produced it, a `resolved` flag (true when its endpoint came out of an RFC 6764 resolution), its `trust` as a string (`rule`, `domain`, `thirdParty` or `unauthenticated`), and, when a fixed provider rule matched, a `provider` field naming the provider.

### Requirement: RFC 6764 DAV
(mechanisms) The `rfc6764` mechanism SHALL resolve CalDAV and CardDAV context paths for a domain through the RFC 6764 well-known and SRV/TXT discovery, yielding one config per DAV service. Each candidate, in order the TXT `path`, the `.well-known` redirect target and the origin, SHALL be validated before it is returned by an unauthenticated `PROPFIND` (`Depth: 0`, following redirects): it qualifies only when the terminal response is a `207 Multi-Status` or a `401` carrying a `WWW-Authenticate` challenge. The `.well-known` candidate additionally requires the probe to have been redirected. When no candidate qualifies, the mechanism SHALL yield no config. The resolution SHALL tell whether an SRV record named the origin or the domain itself was probed.

### Requirement: RFC 8620 JMAP
(mechanisms) The `rfc8620` mechanism SHALL resolve the JMAP session resource (`.well-known/jmap`, requested with `Accept: application/json`, following redirects) and yield a JMAP config carrying the session URL and its advertised auth schemes. The terminal response of the walk SHALL be validated before it counts as a session resource: a 2xx qualifies only when its body parses as a JSON object carrying `capabilities` (including `urn:ietf:params:jmap:core`) and `apiUrl`, whatever its content type; a 401 qualifies only when it carries a `WWW-Authenticate` challenge, since discovery runs unauthenticated and the body is not the session. Any other response means no JMAP behind that origin. A session resource that advertised no scheme SHALL report none rather than assuming password or bearer login. The resolution SHALL tell whether an SRV record named the origin or the domain itself was probed.

### Requirement: OAuth metadata
(mechanisms) The `rfc8414` and `rfc9728` mechanisms SHALL fetch OAuth 2.0 authorization-server metadata (RFC 8414) for an issuer and protected-resource metadata (RFC 9728) for a resource, exposing the endpoints and supported grants a broker needs. Each SHALL take the identifier asked, the issuer or the resource, next to the URL to fetch, and SHALL refuse a document naming another one (RFC 8414 §3.3, RFC 9728 §3.3), a metadata URL that is not `https` before any request, and a document naming an endpoint, an authorization server or a key set that is not `https`.

## REMOVED Requirements
