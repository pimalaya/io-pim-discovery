---
cairn: spec
capability: mechanisms
status: current
---

# Discovery mechanisms

One module per mechanism or RFC, each probing a way providers publish their service settings and yielding raw configs. The source tree mirrors this organisation (one module per RFC).

### Requirement: Mozilla autoconfig
The `autoconfig` mechanism SHALL fetch and parse the Mozilla/Thunderbird autoconfig XML from, in order, the ISP main URL, the ISP fallback URL, the mailconf TXT redirect target, and the Thunderbird ISPDB. Each source that yields a document is tagged with its own `DiscoveryConfigSource` (`IspMain`, `IspFallback`, `Mailconf`, `Ispdb`). Placeholders (`%EMAILADDRESS%`, `%EMAILLOCALPART%`, `%EMAILDOMAIN%`) are substituted, and a port omitted by the document is filled from the service/security defaults.

### Requirement: PACC
The `pacc` mechanism SHALL fetch the provider auto-configuration (PACC) JSON document and verify its DNS-TXT digest, yielding one config per advertised protocol with its authentication data (including any OAuth issuer).

### Requirement: RFC 6186 / RFC 8314 SRV
The `rfc6186` mechanism SHALL run four SRV lookups on the domain: `_imap._tcp` (STARTTLS), `_imaps._tcp` (implicit TLS), `_submission._tcp` (STARTTLS, port 587) and `_submissions._tcp` (implicit TLS, port 465, RFC 8314). It picks the best record per service (lowest priority, highest weight on ties), dropping targets of the root name (`.`, "service not available"). SRV records advertise no authentication, so password login is assumed; `_imaps`/`_submissions` map to implicit TLS and `_imap`/`_submission` to STARTTLS.

### Requirement: RFC 6764 DAV
The `rfc6764` mechanism SHALL resolve CalDAV and CardDAV context paths for a domain through the RFC 6764 well-known and SRV/TXT discovery, yielding one config per DAV service.

### Requirement: RFC 8620 JMAP
The `rfc8620` mechanism SHALL resolve the JMAP session resource (`.well-known/jmap`, following redirects) and yield a JMAP config carrying the session URL and its advertised auth schemes.

### Requirement: OAuth metadata
The `rfc8414` and `rfc9728` mechanisms SHALL fetch OAuth 2.0 authorization-server metadata (RFC 8414) for an issuer and protected-resource metadata (RFC 9728) for a resource, exposing the endpoints and supported grants a broker needs.

### Requirement: WWW-Authenticate probe
An HTTP endpoint's advertised schemes MAY be refined by a `DiscoveryProbeAuth` request that reads the `WWW-Authenticate` challenge, so a config's auth reflects what the server actually challenges for rather than only what a document claimed.
