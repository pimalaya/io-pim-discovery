---
cairn: delta
change: mx-autoconfig
---

# Delta

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: Mozilla autoconfig
The `autoconfig` mechanism SHALL fetch and parse the Mozilla/Thunderbird autoconfig XML from, in order, the ISP main URL, the ISP fallback URL, the mailconf TXT redirect target, the Thunderbird ISPDB, then the MX provider's. When the ISP main URL yields nothing and its host is a CNAME, the last target of the chain SHALL be fetched in its place. The MX provider is the registrable domain, per the Public Suffix List, of each MX host, best record first, other than the address's domain: its ISP main URL (the alias followed likewise), then its ISPDB entry; the first document wins, and a public suffix is never a provider. Each source that yields a document is tagged with its own `DiscoveryConfigSource` (`IspMain`, `IspFallback`, `Mailconf`, `Ispdb`, `Mx`). Placeholders (`%EMAILADDRESS%`, `%EMAILLOCALPART%`, `%EMAILDOMAIN%`) are substituted, and a port omitted by the document is filled from the service/security defaults.

#### Scenario: A domain publishing only its MX records
- GIVEN `test@migadu.net`, whose only record is an MX at `migadu-48722629.mx.flatmail.com`, and `autoconfig.flatmail.com` an alias of `autoconfig.migadu.com` serving a certificate for another name
- WHEN email discovery runs
- THEN it yields `imap.migadu.com:993` and `smtp.migadu.com:465` over TLS, tagged `Mx`

### Requirement: Ordered mechanism fan-out
For the remaining mechanisms, composition SHALL run them in a fixed priority order (MX-derived provider, PACC, autoconfig ISP main / fallback / mailconf / ISPDB / MX provider, RFC 6186 SRV, RFC 6764 CalDAV/CardDAV, RFC 8620 JMAP), scoped to the services requested. `compose_raw`, and the CLI that prints it, SHALL concatenate their configs into one list in that order without deduplicating: each stays the answer of one source, and an endpoint named by several sources appears once per source, merging being the consumer's. `compose_all` and `compose_first` SHALL merge them into one ranked list for callers wanting a single answer (a setup wizard); when they merge a subdomain with its parent host, a resolved endpoint SHALL win over an unresolved one, and between two of the same kind the parent host SHALL win.

## REMOVED Requirements

None.
