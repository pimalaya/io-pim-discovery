---
cairn: change
id: dav-root-resolution
status: landed
created: 2026-10-04
---

# Resolve advertised DAV roots

Asked by MOA (`docs/plan/findings.md`, "Fastmail: DAV only through /.well-known"), after the same problem in pimalaya-linux (`cairn/spec/onboarding.md`, "A resolved endpoint outranks an advertised one", "A bare origin is resolved, not chased") and pimalaya-android.

## Why

Several mechanisms answer for one CalDAV or CardDAV service, and they do not agree. For Fastmail:

- the provider document (PACC, the ISPDB) advertises the bare service host `https://carddav.fastmail.com`;
- the RFC 6764 walk (`dav` source) resolves the real context root `https://dNNNNNN.carddav.fastmail.com/dav/addressbooks`.

The bare host's `/` is not a DAV collection: Fastmail answers 404 there, and only `/.well-known/carddav` redirects (301) to `/dav/addressbooks`. A client that takes the advertised URL as its context root fails its principal lookup: neverest reports "Cannot discover the CardDAV home set", and MOA could not add the box with the first CardDAV choice.

Two things go wrong in io-pim-discovery:

- an advertised bare origin is returned as is, although the crate holds the probe that resolves it (`DiscoveryWellKnown`, RFC 6764 §5);
- the reduction (`compose_all`) folds the resolved shard under its parent host and keeps the parent's endpoint, so the one working URL disappears behind the advertised one.

Every consumer then repeats the same fix (pimalaya-linux in its connector, pimalaya-android in its CardDAV and CalDAV clients, MOA in its ranking). It belongs where the URLs come from.

## What

- **Resolve advertised roots.** After the mechanisms run, every CalDAV or CardDAV config whose HTTP endpoint carries no path, and that the RFC 6764 walk did not already produce, is probed at `/.well-known/{caldav,carddav}` (RFC 6764 §5), in parallel, once per origin and service. When the probe ends on a DAV server, the config's endpoint becomes that context root; otherwise the config stays as advertised. No credential is sent; in secure-only mode the probe goes over HTTPS only, and a redirect to `http://` is not followed (the pool has no `http` factory).
- **Say what was resolved.** A config gains `resolved`: true when its endpoint came out of an RFC 6764 resolution (the `dav` mechanism's walk, or the probe above), false when it is as advertised. It serializes as `resolved` in the JSON.
- **The reduction keeps the resolved endpoint.** When `compose_all` or `compose_first` merges a subdomain with its parent host, a resolved endpoint wins over an unresolved one; between two of the same kind, the parent host still wins, as today.
- **Where it runs:** `compose_raw` (so `pim-discovery all` and MOA get it), `compose_all`, `compose_first` and `compose_all_within` (the probe bounded by what is left of the deadline). The single `dav` entry point is unchanged: its configs are resolved by construction.

## Not here

- A probe of URLs that already carry a path: they are taken as the provider's own context root.
- Credentials, the principal and home-set walk: those stay in the WebDAV client (neverest).
