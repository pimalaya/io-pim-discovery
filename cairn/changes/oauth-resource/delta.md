---
cairn: delta
change: oauth-resource
---

## ADDED Requirements

### Requirement: OAuth resolution
Composition SHALL resolve every OAuth issuer through its RFC 8414 metadata into the grants it advertises (the authorization code grant, and the device grant when a device authorization endpoint is named), each carrying the metadata's `scopes_supported`, leaving an unresolvable issuer as it was. With `rfc9728`, it SHALL first read the protected resource metadata of each distinct HTTP endpoint: a config carrying no OAuth method SHALL gain an issuer per authorization server named, and after issuer resolution every OAuth grant of the config SHALL carry the metadata's `resource`, with its `scopes_supported` when it lists any. An endpoint without metadata SHALL leave its configs as they were.

#### Scenario: A JMAP session publishing its resource metadata
- GIVEN a JMAP config whose session URL serves RFC 9728 metadata naming an authorization server and scopes
- WHEN composition resolves OAuth
- THEN the config's authorization code grant carries the session URL as `resource` and the resource's scopes

## MODIFIED Requirements

## REMOVED Requirements
