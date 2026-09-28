---
cairn: delta
change: dav-context-validation
---

# Delta

The RFC 6764 mechanism validates every candidate context root instead of returning the first one it finds.

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: RFC 6764 DAV
The `rfc6764` mechanism SHALL resolve CalDAV and CardDAV context paths for a domain through the RFC 6764 well-known and SRV/TXT discovery, yielding one config per DAV service. Each candidate, in order the TXT `path`, the `.well-known` redirect target and the origin, SHALL be validated before it is returned by an unauthenticated `PROPFIND` (`Depth: 0`, following redirects): it qualifies only when the terminal response is a `207 Multi-Status` or a `401` carrying a `WWW-Authenticate` challenge. The `.well-known` candidate additionally requires the probe to have been redirected. When no candidate qualifies, the mechanism SHALL yield no config.

## REMOVED Requirements

None.
