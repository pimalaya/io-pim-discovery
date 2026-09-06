---
cairn: change
id: jmap-session-validation
status: active
created: 2026-09-06
issue: https://github.com/pimalaya/io-pim-discovery/issues/1
---

# Validate the JMAP session resource

## Why

`DiscoveryJmapWellKnown` decides on the status code alone. It follows the redirect chain and accepts any 2xx or 401 as a JMAP session resource, without ever looking at what came back. RFC 8620 §2 defines that resource as a JSON object; nothing in the probe asserts it got one.

Apex domains routinely bounce `/.well-known/*` onto a login or marketing page that answers 200 with HTML, and the probe reports it as JMAP. Verified against ik.me on 2026-09-06:

```
GET https://ik.me/.well-known/jmap
  301 -> https://login.infomaniak.com/loginMykSuite
  302 -> https://login.infomaniak.com/en/loginMykSuite
  200  content-type: text/html
```

`email jmap test@ik.me` reports `jmap https://login.infomaniak.com/en/loginMykSuite / bearer, password`. Infomaniak serves no JMAP at all, so a wizard building on this hands the user a session URL that is a login form. The invented auth column compounds it: `DiscoveryServiceConfig::from_jmap` turns an empty scheme list into `bearer, password`, so a page that challenged for nothing looks like it accepts both.

Fastmail is the counter-example the fix must keep. Its session is not on the apex at all (`https://fastmail.com/.well-known/jmap` redirects to `www` and 404s); only the SRV record `_jmap._tcp.fastmail.com -> 0 1 443 api.fastmail.com` points at it, and the probe there ends on a 401 with `content-type: text/plain`, body `No Authorization header`, and `WWW-Authenticate: Bearer resource_metadata=...`. Resolution through SRV works today and must keep working; a validation that demanded JSON on every terminal response would break it.

## What

The terminal response of the redirect walk is validated instead of merely counted:

- The probe sends `Accept: application/json`, so a server is free to content-negotiate rather than hand back its HTML page.
- A 2xx resolves to a session resource only when its `Content-Type` is `application/json` and its body parses as a JSON object carrying `capabilities` (including `urn:ietf:params:jmap:core`) and `apiUrl`. Anything else resolves to `None`.
- A 401 keeps resolving to a session resource, since discovery runs unauthenticated and the body is not the session, but only when it carries a `WWW-Authenticate` challenge, or at least is not `text/html`. A generic portal 401 no longer counts as JMAP.
- `Http11WellKnownOutput::same_origin`, discarded today, is traced on each hop. It is not a validity signal on its own (Fastmail redirects same-origin, ik.me cross-origin) but it explains a rejection when reading logs.
- `DiscoveryServiceConfig::from_jmap` stops inventing `bearer, password` from an empty scheme list. A session resource that advertised nothing reports nothing.

This stays inside this crate. io-jmap owns `JmapSession` and does not depend on discovery, so there is no cycle, but depending on a sibling protocol library for one validity check goes sideways in the layering and drags in its serde, secrecy and schemars surface. The probe parses the two members it needs through `serde_json`, added to the `rfc8620` feature, the way the crate already hand-rolls its own SRV and OAuth-metadata views.

## Cost

A domain whose JMAP server answers 200 with a session document under a non-JSON content type, or a 401 in HTML with no challenge, is no longer discovered. Both are already broken against any RFC 8620 client, and the trace says which check rejected them.

Out of scope, and worth its own change: the redirect walk now exists in three variants in this crate (`rfc8620::well_known` and `rfc9110`, both copies of the same five-hop loop, plus `rfc6764::well_known` following a single hop). Lifting it into io-http needs io-http's yields to carry the current target URL, which is what `DiscoveryStreamPool` routes on.
