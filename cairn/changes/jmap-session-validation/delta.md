---
cairn: delta
change: jmap-session-validation
---

# Delta

The RFC 8620 mechanism gains a validity test on the terminal response of its redirect walk, and stops claiming auth schemes the server never advertised.

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: RFC 8620 JMAP
The `rfc8620` mechanism SHALL resolve the JMAP session resource (`.well-known/jmap`, requested with `Accept: application/json`, following redirects) and yield a JMAP config carrying the session URL and its advertised auth schemes. The terminal response of the walk SHALL be validated before it counts as a session resource: a 2xx qualifies only when its body parses as a JSON object carrying `capabilities` (including `urn:ietf:params:jmap:core`) and `apiUrl`, whatever its content type; a 401 qualifies only when it carries a `WWW-Authenticate` challenge, since discovery runs unauthenticated and the body is not the session. Any other response means no JMAP behind that origin. A session resource that advertised no scheme SHALL report none rather than assuming password or bearer login.

## REMOVED Requirements

None.
