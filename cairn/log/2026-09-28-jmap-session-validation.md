---
cairn: log
change: jmap-session-validation
landed: 2026-09-28
---

# Validate the JMAP session resource

Fixed [#1](https://github.com/pimalaya/io-pim-discovery/issues/1), reported downstream as [himalaya#745](https://github.com/pimalaya/himalaya/issues/745): ik.me bounces `.well-known/jmap` onto an HTML login form answering 200, and the probe listed it as JMAP.

`DiscoveryJmapWellKnown` now sends `Accept: application/json` on every hop and validates the terminal response. A 2xx counts only when its body is a session object (`capabilities` holding `urn:ietf:params:jmap:core`, and an `apiUrl` string), whatever its content type; a 401 only when it carries a `WWW-Authenticate` challenge. The proposal first checked `Content-Type` on the 2xx and let a non-HTML 401 through without a challenge; both were dropped before implementation, the body shape being the proof and RFC 9110 making the challenge mandatory. `same_origin` is traced on each redirect hop. `DiscoveryServiceConfig::from_jmap` no longer invents `bearer, password` from an empty scheme list; the himalaya, himalaya-tui and cardamum wizards already offer both on an empty list, and calendula has no JMAP path.

Verified live: ik.me no longer resolves JMAP and keeps its SRV IMAP/SMTP configs; fastmail.com still resolves through SRV to `api.fastmail.com/jmap/session` with `bearer`. Six probe tests cover the HTML 200, a JSON 200 without the core capability, a session under a non-JSON type, an unchallenged 401, the Fastmail 401 and a session behind a redirect.

Spec updated: mechanisms (MODIFIED: RFC 8620 JMAP).

Still open: the RFC 6764 CalDAV/CardDAV well-known walk has the same flaw, ik.me listing `login.infomaniak.com/loginMykSuite` as both.
