---
cairn: tasks
change: tls-and-identity-kept
---

# Tasks

- [x] `DiscoveryHttpWalk` follows a redirect to `https`, or to `http` from an `http` URL, and ends without a result otherwise.
- [x] `DiscoveryOauthServerResolve` takes the issuer asked and refuses a document naming another issuer or a URL that is not `https`.
- [x] `DiscoveryOauthResourceResolve` takes the resource asked and refuses a document naming another resource or a URL that is not `https`.
- [x] `DiscoveryEndpoint::subdomain_of` holds between `https` endpoints only.
- [x] `DiscoveryServiceConfig::probe_urls` is empty for an endpoint that is not `https`.
- [x] `DiscoveryConfigCollector::finish` lists the encrypted endpoints first.
- [x] `DiscoveryConfigTrust` and `DiscoveryServiceConfig::trust`, serialized as `trust`, set by every constructor and by the compose client, shown by the CLI table.
- [x] `DiscoveryDavResolveOutput` and `DiscoveryJmapResolveOutput` tell whether an SRV record named the origin.
- [x] Tests, each seen failing first: the walk, both metadata coroutines, the collector merge and order, the probe URLs, the JSON field.
- [x] CHANGELOG, spec fold, log.
- [ ] Maintainer: decide whether secure-only becomes the default.
- [ ] Live: `pim-discovery --json all` on the Fastmail test address, to confirm its resource metadata still names the endpoint as fetched.
