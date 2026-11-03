---
cairn: change
id: oauth-resource
status: landed
created: 2026-11-03
---

# OAuth grants carry their resource and scopes

## Why

Fastmail's authorization server refuses a request without an RFC 8707 `resource` (`invalid_target`), and a consumer could not learn which resource to send nor which scopes the server takes: Ortie pinned both by host match (its discovery-layering change). Fastmail publishes all of it, the JMAP session and each DAV root answering RFC 9728 protected resource metadata, its issuer RFC 8414 metadata with `scopes_supported`.

## What

`OauthAuthorizationCodeGrant` and `OauthDeviceAuthorizationGrant` gain `resource: Option<String>` and `scopes_supported: Vec<String>`. Issuer resolution fills the scopes from the RFC 8414 metadata. With `rfc9728`, composition then reads the protected resource metadata of every HTTP endpoint, once per URL and in parallel: a config with no OAuth method gains an issuer per authorization server it names, and after issuer resolution every OAuth grant of the config is bound to the resource, its scopes replaced by the resource's own when it lists any.
