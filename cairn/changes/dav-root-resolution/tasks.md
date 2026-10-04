---
cairn: tasks
change: dav-root-resolution
---

# Tasks

- [x] `DiscoveryServiceConfig::resolved`, true for `from_dav`, serialized as `resolved`.
- [x] `resolve_dav_roots` on the compose client: pathless CalDAV and CardDAV endpoints probed through `DiscoveryWellKnown`, in parallel, once per origin and service; the endpoint replaced and `resolved` set when the probe finds a DAV root.
- [x] Run it in `compose_raw` and before the reduction in `compose` (bounded by the deadline in `compose_all_within`).
- [x] The collector keeps a resolved endpoint over an unresolved one when merging a subdomain with its parent.
- [x] Tests: the collector rule, the JSON field, a canned well-known exchange resolving a bare origin, an origin without redirect left as is.
- [x] Live: `pim-discovery --json all --secure-only` on a Fastmail address.
- [x] CHANGELOG, spec fold, log.
