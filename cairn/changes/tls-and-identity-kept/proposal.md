---
cairn: change
id: tls-and-identity-kept
status: landed
created: 2026-11-13
---

# Discovery does not trade TLS or identity away

From the family audit of 2026-11-13 (DISC-02, DISC-03, DISC-06, DISC-04).

## Why

Discovery decides where a user's credentials go, and four paths let an answer nobody authenticated stand in for one that was.

- **A cleartext endpoint replaced an encrypted one.** The collector merges a subdomain with its parent host by comparing host names only. A plain `_carddav` record naming `d1.carddav.example.com:80` produced a resolved `http://` root that took the place of the `https://carddav.example.com` a document advertised: the caller received one CardDAV config, in clear. The auth probe then asked that cleartext endpoint which schemes it wanted and put its answer first.
- **OAuth metadata was used whoever it named.** Neither the `issuer` of an RFC 8414 document nor the `resource` of an RFC 9728 one was compared with what had been asked, and endpoints of any scheme were copied into the grants. A hostile endpoint could name a provider's real authorization server and real API as its own resource, so a token valid for the real API came back to it.
- **The redirect walk followed any scheme.** A `Location: tcp://host:port/` sent the probe through the pool's DNS factory to any host and port the client reaches, secure-only mode included, since that factory stays registered for DNS. A redirect from `https` to `http` was followed outside secure-only.
- **Nothing told an authenticated answer from a DNS one.** An `ispMain` config came from the domain's own name with its certificate, or from the target of a CNAME that a DNS answer chose. A `dav` or `jmap` config did not say whether its origin was the domain or an SRV target. RFC 6186 section 6 asks a client to check an SRV target against the queried domain or to confirm it with the user: a consumer had nothing to run that check on.

## What

- **The merge keeps to one scheme.** `DiscoveryEndpoint::subdomain_of` holds between two `https` endpoints only, so a cleartext endpoint neither merges with nor replaces an encrypted one: both stay in the list. `DiscoveryConfigCollector::finish` lists the encrypted endpoints first, each group in mechanism order. `probe_urls` is empty for an endpoint that is not `https`, so no auth is refined from a cleartext answer.
- **OAuth metadata is checked where it is fetched.** `DiscoveryOauthServerResolve::new` takes the issuer asked and `DiscoveryOauthResourceResolve::new` the resource asked. A document naming another one is refused (RFC 8414 section 3.3, RFC 9728 section 3.3), as is one naming an endpoint or an authorization server that is not `https`. A metadata URL that is not `https` is refused before any request.
- **The walk stays on HTTP and never leaves TLS.** A redirect is followed to `https`, or to `http` from an `http` URL. Anything else ends the walk without a result, like a looping chain.
- **Every config says what vouches for it.** `DiscoveryServiceConfig` gains `trust`, a `DiscoveryConfigTrust`: `Rule` (a built-in rule matched the domain), `Domain` (the domain answered over TLS under its own name), `ThirdParty` (a fixed third party answered over TLS) or `Unauthenticated` (a DNS answer named the host, or an exchange ran in clear). The alias fallback of the ISP main URL is `Unauthenticated` where the URL itself is `Domain`. `DiscoveryDavResolve` and `DiscoveryJmapResolve` return whether an SRV record named their origin. The RFC 6186 section 6 duty is documented on the field.

## Not in this change

- **The default of secure-only.** The audit also asks to turn it on by default with an explicit opt-out. It is off by a decision recorded on 2026-10-04 (the secure-only change), and reversing it is the maintainer's call. What this change fixes does not depend on it.
- **An RFC 8484 default resolver.** Declined for the CLI on 2026-10-04 for the same reason: it moves the trust to the resolver without authenticating the answer.
