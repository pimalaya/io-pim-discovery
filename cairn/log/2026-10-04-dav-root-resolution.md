---
cairn: log
change: dav-root-resolution
landed: 2026-10-04
---

# Resolve advertised DAV roots

How it works:
- `DiscoveryComposeClientStd::resolve_dav_roots` runs after the mechanisms and before any reduction, in `compose` (so `compose_all`, `compose_first`, and `compose_all_within` with what is left of its deadline) and in `compose_raw`.
- It collects the targets (`dav_root_target`): every CalDAV or CardDAV config, not yet resolved, whose HTTP endpoint has no path. The origin and service pairs are deduplicated.
- It runs one `DiscoveryWellKnown` probe per pair, in parallel: scoped threads, or `collect_within` under a deadline.
- A probe that ends on a DAV server replaces the endpoint of every config of that origin and service, and sets `resolved`. A failed probe, no redirect, or a probe still running at the deadline leaves the config as it was.
- In secure-only mode the pool has no `http` factory, so neither a plain origin nor a redirect to `http://` is reached, and a non-`https` root is never applied.

`DiscoveryServiceConfig::resolved` is new: `from_dav` sets it, every other constructor leaves it false. It serializes as `resolved` (deserialized with a default of false).

The collector now keeps the resolved endpoint when it merges a subdomain with its parent host; between two of the same kind the parent still wins.

Spec updated:
- compose: ADDED "Advertised DAV roots are resolved", MODIFIED "Ordered mechanism fan-out" and "One JSON shape".

Verified:
- tests (43, three new):
  - a local host shaped like Fastmail's (404 at `/`, `/.well-known/carddav` redirecting to `/dav/addressbooks`, 401 there) resolved, a URL with a path untouched;
  - a host without redirect left as advertised, and secure-only reaching nothing over `http`;
  - the collector rule on Fastmail's shapes;
- the JSON field;
- clippy (pre-existing warning only), fmt, feature subsets.

Live, `all --secure-only`:
- `test@fastmail.com`: the PACC configs, advertised as `https://caldav.fastmail.com` and `https://carddav.fastmail.com`, now read `/dav/calendars` and `/dav/addressbooks`, `resolved: true`; the `dav` source's shards unchanged;
- `test@posteo.net` and `vous@gmail.com`: unchanged (their DAV URLs already carry a path, or come from the walk).
