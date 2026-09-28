---
cairn: change
id: dav-context-validation
status: landed
created: 2026-09-28
---

# Validate the DAV context root

## Why

`DiscoveryDavResolve` never fails to produce a CalDAV/CardDAV URL. `DiscoveryWellKnown` returns the first `Location` of `.well-known/{caldav,carddav}` unchecked, and when there is none the resolve falls back to the bare origin, also unchecked; a TXT `path` is joined on and returned without a request. Verified on 2026-09-28 with `pim-discovery all`:

- ik.me lists `https://login.infomaniak.com/loginMykSuite` (an HTML login form) as both CalDAV and CardDAV.
- example.com lists `https://example.com/` as both.

The same unvalidated-bootstrap flaw the `jmap-session-validation` change fixed for JMAP, with one more: only the first redirect hop is read, so a chain of two ends on an intermediate URL.

## What

Every candidate context root (the TXT path, the `.well-known` redirect target, the origin fallback) is validated by the same unauthenticated request before it is returned: a `PROPFIND` with `Depth: 0` asking for `current-user-principal`, the first request of the RFC 6764 §6 bootstrap, following redirects. The terminal response qualifies when it is a `207 Multi-Status`, or a `401` carrying a `WWW-Authenticate` challenge. Probed live, unauthenticated:

| Endpoint | PROPFIND |
|---|---|
| carddav.fastmail.com, sync.infomaniak.com, posteo.de:8443, caldav.icloud.com | 401 + challenge |
| login.infomaniak.com (the ik.me target), example.com | 405 |

GET does not separate them: iCloud answers it 400, the login form 200.

- `DiscoveryWellKnown` walks the whole redirect chain with that `PROPFIND` and returns the terminal URL only when the chain redirected at least once and the terminal response qualifies; its return type is unchanged.
- `DiscoveryDavContext` is the same check on a given URL, without the redirect requirement.
- `DiscoveryDavResolve` tries the TXT path, then `.well-known`, then the origin, each validated, and fails with `NotFound` when none qualifies; its return type is unchanged.
- The redirect walk, now needed by four probes, moves into `shared::walk` as `DiscoveryHttpWalk`, a request template replayed on each hop. The JMAP well-known and RFC 9110 auth probes are ported onto it.

## Cost

A domain with no reachable DAV server no longer yields a DAV config, which is the fix. A server answering an unauthenticated `PROPFIND` with something other than 207 or a challenged 401 is no longer discovered; none of the probed providers does.

Out of scope: `from_dav` still assumes password login. The validating `PROPFIND` already sees the server's challenge, so threading its schemes through would fix iCloud's auth, whose GET-based auth probe answers 400; it changes `DiscoveryWellKnown`'s and `DiscoveryDavResolve`'s return types, which pimalaya-linux (path dependency) and pimalaya-android consume, so it wants its own change.
