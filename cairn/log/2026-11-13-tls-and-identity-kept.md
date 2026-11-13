---
cairn: log
change: tls-and-identity-kept
landed: 2026-11-13
---

# Discovery does not trade TLS or identity away

Four findings of the family audit of 2026-11-13 (DISC-02, DISC-03, DISC-06, DISC-04), one change.

The merge (DISC-02):
- `DiscoveryEndpoint::subdomain_of` now requires both endpoints to be `https`. The takeover in `DiscoveryConfigCollector::collect` is reached through `equivalent` (same URL, scheme included) or `subdomain_of`, so an `http://` shard can no longer replace, nor merge its auth into, the `https://` host a document advertised: both stay.
- `DiscoveryConfigCollector::finish` sorts the encrypted endpoints first (a stable sort on `is_secure`, so mechanism order holds inside each group). `compose_raw` does not go through the collector and keeps its order.
- `DiscoveryServiceConfig::probe_urls` returns nothing for an endpoint that is not `https`, so the auth probe of the compose client never refines a config from a cleartext answer. With the redirect rule below, a probe that starts on `https` stays on it.
- Not done: secure-only stays off by default. That default is the decision of 2026-10-04 and reversing it is the maintainer's.

OAuth metadata (DISC-03):
- `DiscoveryOauthServerResolve::new(url, issuer)` and `DiscoveryOauthResourceResolve::new(url, resource)` take the identifier asked next to the URL to fetch. The check sits in the coroutines so that it follows them wherever they move.
- A metadata URL that is not `https` completes with `NotHttps` on the first resume, before any request. A document whose `issuer` or `resource` differs completes with `Issuer` or `Resource`, carrying the identifier found. A document naming a URL that is not `https` completes with `NotHttps`: for the server, the issuer, the authorization, token, registration, revocation, introspection and device authorization endpoints and the key set; for the resource, the resource, its authorization servers and its key set.
- The comparison is `Url` equality, which is what the Android bridge already ran on top of the library: host case, a default port and an empty path against `/` do not count as a difference, a trailing slash on a longer path does.
- `oauth_server` and `oauth_resource` on the compose client keep their signatures and answer `None` for refused metadata, with the reason at trace level. `resolve_oauth` therefore reads nothing from an `http://` endpoint.

The redirect walk (DISC-06):
- `DiscoveryHttpWalk` follows a redirect to `https`, or to `http` when the current URL is `http`. Any other target completes the walk with `None`, the value a looping chain already gave, so the DAV resolution moves on to its next candidate and the JMAP and auth probes report nothing.
- This also closes the secure-only bypass through the `tcp` factory, which that mode keeps for DNS.

Provenance (DISC-04):
- New `DiscoveryConfigTrust` (`Rule`, `Domain`, `ThirdParty`, `Unauthenticated`, the default for a config read without the field) and `DiscoveryServiceConfig::trust`, serialized as `trust`. The RFC 6186 section 6 duty is documented on the field.
- Who sets what: the provider rule on the domain is `Rule`, the same rule matched through MX records `Unauthenticated`; PACC, the ISP main URL and the ISP fallback URL are `Domain`; the ISPDB entry of the domain is `ThirdParty`; the alias fallback of the ISP main URL, the mailconf target, the three steps of the MX provider's autoconfig and RFC 6186 SRV records are `Unauthenticated`; an RFC 6764 or RFC 8620 resolution is `Domain` when the domain itself was probed and `Unauthenticated` when an SRV record named the origin.
- For that last line `DiscoveryDavResolve` returns a `DiscoveryDavResolveOutput { root, srv }` and `DiscoveryJmapResolve` a `DiscoveryJmapResolveOutput { session, srv }`. `DiscoveryWebdavClientStd::resolve` still returns the root alone.
- `from_autoconfig`, `from_dav`, `from_jmap` and `DiscoveryKnownProvider::configs` take the trust as an argument rather than guessing it from the source: a runtime that fetches the alias target or matches a provider through MX records has to say so.
- When the collector lets an endpoint take over, the trust follows it with the source. Two configs naming the same endpoint keep the first one's source and trust.
- `resolve_dav_roots` marks a config `Unauthenticated` when its root was found through an origin that is not `https`.
- The CLI table gains a TRUST column.

Capabilities moved: `compose` (ADDED "Config trust"; MODIFIED "Ordered mechanism fan-out", "Auth refinement", "OAuth resolution", "Advertised DAV roots are resolved", "One JSON shape"), `mechanisms` (MODIFIED "RFC 6764 DAV", "RFC 8620 JMAP", "OAuth metadata"), `coroutine` (ADDED "Redirects stay on HTTP and never leave TLS").

Verified: `cargo test --all-features` (59 passed, 12 new), `cargo clippy --all-features --all-targets -- -D warnings` clean, `cargo build --no-default-features`, `cargo doc` with broken links denied, every mechanism feature alone. The tests of the walk, of both metadata coroutines, of the merge, of the order, of the probe URLs and of the JSON field were seen failing on the code before the fix; those of the `srv` flag and of the trust on takeover exercise API that did not exist before.

Not verified: no live run. The resource check is strict on the URL as fetched, which matches what the 2026-11-03 run observed on Fastmail (grants carrying their own URL as `resource`), but that run predates the check: `pim-discovery --json all` on the Fastmail test address should confirm the OAuth grants are still bound.
