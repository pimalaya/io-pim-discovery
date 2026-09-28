---
cairn: tasks
change: dav-context-validation
---

# Tasks

- [x] Extract the redirect walk into `shared::walk::DiscoveryHttpWalk`.
- [x] Port `rfc8620::well_known` and `rfc9110` onto it.
- [x] Add `rfc6764::context::DiscoveryDavContext`, the `PROPFIND` validation.
- [x] Walk and validate the whole chain in `DiscoveryWellKnown`.
- [x] Validate the TXT path and the origin fallback in `DiscoveryDavResolve`, failing with `NotFound`.
- [x] Test the 207, challenged 401, 405, unchallenged 401 and multi-hop shapes.
- [x] Check, fmt, and exercise the CLI against ik.me, example.com, posteo.de and fastmail.com.
- [x] Check pimalaya-linux still builds against the path dependency (io-pim-discovery compiles in it; linux itself is blocked on an unrelated io-pimdir error).
- [x] Changelog entry.
