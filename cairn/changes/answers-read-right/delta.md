---
cairn: delta
change: answers-read-right
---

# Delta

## ADDED Requirements

## MODIFIED Requirements

### Requirement: Provider short-circuit first
(compose) Composition SHALL reduce the fixed-provider rules first: when the email domain (or its MX records) matches a known provider (Google, Microsoft), the provider's own configs are produced and tag their source as `Provider`. Provider detection is exposed on its own (`provider`, `is_google`, `is_microsoft`). A provider's label (`outlook`, `hotmail`, `live`) SHALL match only right under a public suffix of the registries, per the Public Suffix List (`outlook.com`, `hotmail.co.uk`), never under a registrable or a private domain.

#### Scenario: A provider's label under someone's domain
- GIVEN the address `someone@live.example.org`
- WHEN composition runs
- THEN no fixed-provider config is produced and the MX records are asked

### Requirement: Shared DNS transport
(coroutine) DNS-backed mechanisms SHALL share one transport that speaks DNS-over-TCP (length-prefixed framing) or RFC 8484 DNS-over-HTTPS, picked from the resolver URL scheme. Query names are made absolute (trailing dot) and an internationalized name is turned into its A-label form before parsing, since the `domain` parser rejects relative names and non-ASCII labels. A response whose code is neither `NOERROR` nor `NXDOMAIN` SHALL fail the lookup with that code instead of reading as an empty answer; the resolver probe alone returns it as its result.

### Requirement: Mozilla autoconfig
(mechanisms) The `autoconfig` mechanism SHALL fetch and parse the Mozilla/Thunderbird autoconfig XML from, in order, the ISP main URL, the ISP fallback URL, the mailconf TXT redirect target, the Thunderbird ISPDB, then the MX provider's. When the ISP main URL yields nothing and its host is a CNAME, the last target of the chain SHALL be fetched in its place. The MX provider is the registrable domain, per the Public Suffix List, of each MX host, best record first, other than the address's domain: its ISP main URL (the alias followed likewise), then its ISPDB entry; the first document wins, and a public suffix is never a provider. Each source that yields a document is tagged with its own `DiscoveryConfigSource` (`IspMain`, `IspFallback`, `Mailconf`, `Ispdb`, `Mx`). Placeholders (`%EMAILADDRESS%`, `%EMAILLOCALPART%`, `%EMAILDOMAIN%`) are substituted, and a port omitted by the document is filled from the service/security defaults. A server type, a socket type or an authentication method the library does not know SHALL NOT reject the document: it parses as unknown, a server of unknown type or security is skipped and an unknown method is ignored. The `GSSAPI` method and the deprecated `plain` and `secure` method names SHALL be recognised.

#### Scenario: A domain publishing only its MX records
- GIVEN `test@migadu.net`, whose only record is an MX at `migadu-48722629.mx.flatmail.com`, and `autoconfig.flatmail.com` an alias of `autoconfig.migadu.com` serving a certificate for another name
- WHEN email discovery runs
- THEN it yields `imap.migadu.com:993` and `smtp.migadu.com:465` over TLS, tagged `Mx`

#### Scenario: A document listing a server of another kind
- GIVEN a document with an `<incomingServer type="exchange">` next to its IMAP server
- WHEN it is parsed and flattened
- THEN the IMAP config is returned and the other server is left out

### Requirement: RFC 6186 / RFC 8314 SRV
(mechanisms) The `rfc6186` mechanism SHALL run four SRV lookups on the domain: `_imap._tcp` (STARTTLS), `_imaps._tcp` (implicit TLS), `_submission._tcp` (STARTTLS, port 587) and `_submissions._tcp` (implicit TLS, port 465, RFC 8314). It picks the best record per service (lowest priority, highest weight on ties), dropping targets of the root name (`.`, "service not available"). SRV records advertise no authentication, so password login is assumed; `_imaps`/`_submissions` map to implicit TLS and `_imap`/`_submission` to STARTTLS. A lookup answered with a failure response code SHALL fail the mechanism rather than read as a missing record.

### Requirement: RFC 6764 DAV
(mechanisms) The `rfc6764` mechanism SHALL resolve CalDAV and CardDAV context paths for a domain through the RFC 6764 well-known and SRV/TXT discovery, yielding one config per DAV service. Each candidate, in order the TXT `path`, the `.well-known` redirect target and the origin, SHALL be validated before it is returned by an unauthenticated `PROPFIND` (`Depth: 0`, following redirects): it qualifies only when the terminal response is a `207 Multi-Status` or a `401` carrying a `WWW-Authenticate` challenge. The `.well-known` candidate additionally requires the probe to have been redirected. When no candidate qualifies, the mechanism SHALL yield no config. A failed TXT lookup SHALL count as no `path`. The resolution SHALL tell whether an SRV record named the origin or the domain itself was probed.

### Requirement: no_std library, std binary
(packaging) The library SHALL target `no_std` (only `alloc`). `std` is pulled in only behind the `client`, `stream` or `cli` features, and by the `autoconfig` mechanism, whose XML parser is a std crate: no other mechanism SHALL build a dependency with its `std` feature. The DNS library SHALL be pulled only by the mechanisms that look records up (`autoconfig`, `pacc`, `rfc6186` and the features built on it), without its `std` feature, and its version SHALL be pinned exactly for as long as the crate uses its unstable `new` API. The binary is `std`, as usual.

## REMOVED Requirements
