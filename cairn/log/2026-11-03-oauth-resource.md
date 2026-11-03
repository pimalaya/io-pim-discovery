---
cairn: log
change: oauth-resource
date: 2026-11-03
---

# OAuth grants carry their resource and scopes

`DiscoveryAuthMethod::OauthAuthorizationCodeGrant` and `OauthDeviceAuthorizationGrant` gained `resource` and `scopes_supported` (JSON `resource`, `scopesSupported`, both defaulting on input). Issuer resolution fills the scopes from the RFC 8414 metadata. The new `resolve_oauth` pass in src/compose/client.rs reads the RFC 9728 metadata of every distinct HTTP endpoint before the issuer pass: a config without OAuth gains the named authorization servers as issuers (`add_resource_issuers`), and every grant is then bound to the resource (`set_oauth_resource`), its scopes narrowed to the resource's when it lists any. It runs in `compose_raw` and the reduced compose paths alike.

Capabilities moved: `compose` (OAuth resolution added; issuer resolution, which shipped unspecified, is now stated).

Verified: every feature combination checked, clippy clean, `cargo test --all-features` passed (47, the two new tests among them). `pim-discovery --json all test@fastmail.com`: the PACC JMAP, CalDAV, CardDAV and WebDAV grants carry their own URL as `resource` with the resource's scopes, IMAP, POP3 and SMTP the issuer's 8 scopes and no resource, and the RFC 8620 JMAP config, bearer only before, gains the grant through the resource metadata.
