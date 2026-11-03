---
cairn: tasks
change: oauth-resource
---

- [x] Add `resource` and `scopes_supported` to both OAuth grant variants, defaulting on deserialization
- [x] Fill `scopes_supported` from the RFC 8414 metadata on issuer resolution
- [x] Add the RFC 9728 pass to `compose_raw`, `compose_all`, `compose_first` and `compose_all_within`
- [x] Tests for the issuer addition and the resource binding; live check on test@fastmail.com
