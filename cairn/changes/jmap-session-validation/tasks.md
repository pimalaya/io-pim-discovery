---
cairn: tasks
change: jmap-session-validation
---

# Tasks

- [x] Send `Accept: application/json` on every hop of the `.well-known/jmap` walk.
- [x] Accept a 2xx only when its body parses as a JSON object with `capabilities` (including `urn:ietf:params:jmap:core`) and `apiUrl`.
- [x] Accept a 401 only when it carries a `WWW-Authenticate` challenge.
- [x] Trace `same_origin` on each hop of the `.well-known/jmap` walk.
- [x] Drop the `bearer, password` assumption from `DiscoveryServiceConfig::from_jmap`.
- [x] Add `dep:serde` and `dep:serde_json` to the `rfc8620` feature.
- [x] Cover the rejected shapes (HTML 200, JSON 200 without the core capability, 401 without challenge), a valid session behind a redirect, and the Fastmail shape (text/plain 401 with a Bearer challenge) with tests.
- [x] Check that the himalaya-tui, cardamum and calendula wizards handle an empty auth list.
- [x] Check, fmt, and exercise the CLI against ik.me and fastmail.com.
- [x] Changelog entry.
