---
cairn: log
change: consistent-json
landed: 2026-10-04
---

# Consistent JSON output

A `DiscoveryServiceConfig` now serializes through a private wire struct (`serde(into, try_from)`): `source` is always a string (`provider`, `pacc`, `ispMain`, `ispFallback`, `mailconf`, `ispdb`, `srv`, `dav`, `jmap`) and a fixed rule's provider moves to a `provider` field, left out otherwise. The OAuth methods' fields are camelCase (`authorizationEndpoint`, `tokenEndpoint`, `deviceAuthorizationEndpoint`) through `rename_all_fields`. The Rust types are unchanged: `DiscoveryConfigSource::Provider(DiscoveryKnownProvider)` stays, so the wizards matching on it compile as before. Deserializing reads the new shape only.

Merging: the delta said composition never deduplicates, but `compose_all` and `compose_first` merge through `DiscoveryConfigCollector`, which the wizards rely on for one answer per endpoint. What MOA observed and wants kept is the CLI's output, which comes from `compose_raw` and never merged. The spec now says exactly that: `compose_raw` and the CLI concatenate in priority order without deduplicating, `compose_all` and `compose_first` merge. No code changed for it.

Consumers: the Rust wizards (himalaya, himalaya-tui, neverest, carillon, ortie, cardamum, calendula) never read this JSON. pimalaya-android serializes these configs to its Java layer (`ServiceConfig.java`, `AuthMethod.java` read `source` and the snake_case OAuth fields); it is on 0.7 and must follow when it bumps.

Spec updated: compose (ADDED: One JSON shape; MODIFIED: Ordered mechanism fan-out).

Verified: a test serializes a Google config and an autoconfig one, checks `source`, `provider` and that no key holds an underscore, and round-trips; clippy (pre-existing warning only), fmt, feature subsets. Live: `--json all` on outlook.fr, posteo.de and fastmail.com prints string sources and no underscore key.
