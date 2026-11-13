---
cairn: log
change: answers-read-right
landed: 2026-11-13
---

# Answers are read for what they say, and the core stops linking std

Five findings of the family audit of 2026-11-13 (DISC-09, DISC-10, DISC-13, DISC-15, DISC-11).

Internationalized names (DISC-09):
- `absolute_name` in src/shared/dns.rs, the function every DNS coroutine builds its query name through (TXT, SRV, MX, CNAME, the resolver probe), turns a non-ASCII name into its A-label form with `url::Host::parse`. An ASCII name does not go through it and is sent as before.
- A name the conversion refuses is left as it is: the name parser then fails the lookup with its own "not a valid name" error.
- The compose client already parsed its input as a host name (bounds-and-host-names), so `compose_*` no longer blamed the resolver. What this adds is the same for every other caller, and PACC asking its digest under the name it fetched its document for.

Autoconfig (DISC-10):
- `DiscoveryServerType`, `DiscoverySecurityType` and `DiscoveryAuthenticationType` gain `Unknown`, marked `#[serde(other)]`, which serde-xml-rs honours for the attribute and for the text elements alike.
- `GSSAPI` is read into the existing `GsApi` variant (the `GSAPI` spelling still is, and the variant keeps its name: renaming it would break more than it fixes). `plain` and `secure`, the deprecated names of the two password methods, are read as them. `Display` prints `GSSAPI`.
- `from_autoconfig` skips a server of unknown type or of unknown security, since nothing says how to reach it, and ignores an unknown method as it already ignored the ones it has no use for.

Response codes and the TXT lookup (DISC-13):
- `DiscoveryDnsExchange` reads the response code of the message it completes with, over TCP and over RFC 8484 alike, and returns the new `DiscoveryDnsExchangeError::Rcode` when it is neither `NOERROR` nor `NXDOMAIN`. The four record coroutines already wrap an exchange error, so none of them changed.
- `DiscoveryDnsProbe` turns that error back into its result: the resolver check still reports "answered SERVFAIL".
- What callers see: the RFC 6186 mechanism fails as a whole when one of its four lookups is refused, instead of returning the records of the others; the RFC 6764 and RFC 8620 resolutions treat it like an unreachable resolver and probe the domain itself over `https`, as they did for any SRV error; PACC, mailconf, the MX and alias steps are skipped.
- In `DiscoveryDavResolve`, a failed TXT lookup moves on to the well-known probe. `DiscoveryDavResolveError::Txt` is therefore no longer returned, like `Srv` before it; both variants stay.

The Microsoft rule (DISC-15):
- `DiscoveryKnownProvider::from_domain` matches `outlook`, `hotmail` or `live` only when the rest of the domain is itself a public suffix of the ICANN section of the Public Suffix List. `outlook.com.br` and `hotmail.co.uk` match; `live.example.org` does not, nor `hotmail.github.io` (a private suffix), nor the bare label.
- The rule lives in the compose module, which every mechanism but the two OAuth ones compiles, so psl is now pulled by `pacc` and `rfc6186` as well as `autoconfig`.

The manifest (DISC-11):
- `domain` is optional, required as `=0.12.3` with `alloc` and `unstable-new`, and pulled by `autoconfig`, `pacc` and `rfc6186`. The code compiles unchanged without its `std` feature. serde is required with `default-features = false` and `alloc`, `derive`.
- `cargo tree --no-default-features -e features,no-dev`, before: `domain`, `jiff`, `jiff-core` and `octseq` carry `std` in every feature set, the empty one included, and `serde`, `serde_core` carry it with any mechanism. After: no dependency carries a `std` feature in the empty set nor with `pacc`, `rfc6186`, `rfc6764`, `rfc8620`, `rfc8414` or `rfc9728`; `domain` is absent from the empty set and from the two OAuth mechanisms. With `autoconfig`, `serde`, `serde_core` and `thiserror` carry `std`, through serde-xml-rs.
- What still needs std: `autoconfig` (its XML parser), `client` (the light clients use `std::io` and `std::net` and read the system resolver), `stream`, the TLS features and `cli`.
- The lockfile loses `bumpalo` and two `bytes` edges, which only `domain/std` and `octseq/std` brought.
- Not proven by a build: the toolchain of this repository has no target without std. The feature graph is the evidence.

Not done: DISC-16, the duplicate and serialised lookups. It was optional, is unmeasured, and its smallest part (two SRV questions per DAV resolution instead of four) rewrites a state machine and an error type.

Capabilities moved: `compose` (MODIFIED "Provider short-circuit first"), `coroutine` (MODIFIED "Shared DNS transport"), `mechanisms` (MODIFIED "Mozilla autoconfig", "RFC 6186 / RFC 8314 SRV", "RFC 6764 DAV"), `packaging` (MODIFIED "no_std library, std binary").

Verified: `cargo test --all-features` (74 passed, 7 new), `cargo clippy --all-features --all-targets -- -D warnings` clean on the full set and on nineteen feature subsets, `cargo build --no-default-features`, `cargo doc` with broken links denied. Each new test was seen failing first; the one that checks the compose client with an internationalized domain passes since the previous change, so it was run against the tree before that change, rebuilt aside, where it fails with "DNS probe domain `bücher.example` is not a valid name". No live run.
