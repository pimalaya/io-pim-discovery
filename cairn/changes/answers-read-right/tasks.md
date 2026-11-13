---
cairn: tasks
change: answers-read-right
---

# Tasks

- [x] `absolute_name` turns an internationalized name into its A-label form.
- [x] `Unknown` on `DiscoveryServerType`, `DiscoverySecurityType` and `DiscoveryAuthenticationType`; `GSSAPI`, `plain` and `secure` read; `from_autoconfig` skips what it does not know.
- [x] `DiscoveryDnsExchangeError::Rcode`; the resolver probe still returns the code.
- [x] A failed TXT lookup counts as no `path` in `DiscoveryDavResolve`.
- [x] `DiscoveryKnownProvider::from_domain` matches a Microsoft label right under a public suffix of the registries.
- [x] Manifest: `domain` optional, `=0.12.3`, without `std`; serde without default features; psl for `pacc` and `rfc6186`.
- [x] Docs: the lib header, the README, the contributing guide and the packaging spec say that `autoconfig` needs std.
- [x] Tests, each seen failing first.
- [x] CHANGELOG, spec fold, log.
- [ ] A build for a target without std, in the shared workflow: the toolchain of this repository has none installed.
