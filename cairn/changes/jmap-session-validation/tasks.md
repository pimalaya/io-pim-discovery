---
cairn: tasks
change: jmap-session-validation
---

# Tasks

- [ ] Send `Accept: application/json` on every hop of the `.well-known/jmap` walk.
- [ ] Accept a 2xx only when it is `application/json` and its body parses as a JSON object with `capabilities` (including `urn:ietf:params:jmap:core`) and `apiUrl`.
- [ ] Accept a 401 only when it carries a `WWW-Authenticate` challenge or is not `text/html`.
- [ ] Trace `same_origin` on each hop of both redirect walks.
- [ ] Drop the `bearer, password` assumption from `DiscoveryServiceConfig::from_jmap`.
- [ ] Add `dep:serde_json` to the `rfc8620` feature.
- [ ] Cover the rejected shapes (HTML 200, HTML 401 without challenge) and the Fastmail shape (text/plain 401 with a Bearer challenge) with tests.
- [ ] Check, fmt, and exercise the CLI against ik.me and fastmail.com.
- [ ] Changelog entry.
