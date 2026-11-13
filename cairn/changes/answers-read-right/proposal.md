---
cairn: change
id: answers-read-right
status: landed
created: 2026-11-13
---

# Answers are read for what they say, and the core stops linking std

From the family audit of 2026-11-13 (DISC-09, DISC-10, DISC-13, DISC-15, DISC-11).

## Why

- **An internationalized domain failed in DNS.** The DNS name parser refuses non-ASCII labels, and nothing converted a name to its A-label form. The compose client has done the conversion for its own input since the bounds-and-host-names change; every other caller of the DNS coroutines (the light clients, another runtime) still failed, and PACC fetched its document for a name it could not look the digest up under.
- **One unknown value rejected a whole autoconfig document.** The three enums of the document (server type, socket type, authentication method) had no catch-all, so an `<incomingServer type="exchange">` next to an IMAP entry, or a method the library does not know, lost every server of the document. `GSSAPI`, the value the format defines, was spelled `GSAPI`, and the deprecated `plain` and `secure` names were not read.
- **A resolver failure read as "no record".** The record coroutines never looked at the response code: `SERVFAIL` on `_imaps._tcp` was an empty answer, so the STARTTLS record was all a caller saw. In the other direction, a failed TXT lookup ended the RFC 6764 resolution although the TXT `path` is optional and the well-known probe would have answered.
- **The Microsoft rule claimed domains Microsoft does not host.** Any domain whose first label is `outlook`, `hotmail` or `live` matched: `someone@live.example.org` was given the Microsoft configs first, and `compose_first` returned nothing else.
- **The core linked std.** `domain` was built with its `std` feature in every feature set, even the two OAuth mechanisms that ask no DNS question, and serde kept its default `std`. The DNS types come from `domain::new`, behind `unstable-new`, which the crate exempts from its version promises, under a caret requirement.

## What

- **Names go out as A-labels.** `absolute_name`, the one place every DNS coroutine builds its query name through, converts an internationalized name. A name that cannot be converted stays as it is and fails as an invalid name of its lookup.
- **Unknown values parse.** The three enums gain an `Unknown` variant taking any other value. `from_autoconfig` skips a server of unknown type or security and ignores an unknown method. `GSSAPI`, `plain` and `secure` are recognised.
- **A failure code is a failure.** `DiscoveryDnsExchange` completes with the new `Rcode` error when the response code is neither `NOERROR` nor `NXDOMAIN`, so every record lookup fails with it; the resolver probe keeps returning the code. A failed TXT lookup in the RFC 6764 resolution counts as no `path`.
- **The Microsoft labels match under a public suffix only.** `from_domain` asks the Public Suffix List whether what follows the label is a suffix of the registries (`com`, `co.uk`, `com.br`), not a registrable or a private one.
- **The manifest.** `domain` is optional, pinned to `=0.12.3`, without `std`, and pulled by `autoconfig`, `pacc` and `rfc6186` (hence the features built on the last). serde drops its default features. psl, which the provider rule now needs, is pulled by `pacc` and `rfc6186` as well as `autoconfig`.

## Not in this change

- **The duplicate and serialised lookups (DISC-16).** Optional, unmeasured, and not small: asking only the two SRV questions a DAV resolution reads changes its state machine and its error type.
- **`autoconfig` without std.** Its XML parser is a std crate. The docs now say so instead of promising otherwise.
