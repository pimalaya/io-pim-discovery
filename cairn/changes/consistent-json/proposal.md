---
cairn: change
id: consistent-json
status: active
created: 2026-10-04
---

# Consistent JSON output

Part of the io-pim-discovery work MOA needs (MOA `docs/plan/discovery.md`), with [provider-api-services](../provider-api-services/proposal.md) and [secure-only](../secure-only/proposal.md).

## Why

`pim-discovery --json` is how a program other than a Rust wizard consumes discovery: MOA runs the binary and reads its output. Checked on 2026-10-04 (0.8.0, `all` on Gmail, Posteo, Outlook.fr and a Google Workspace domain): one document on stdout, nothing else, and `{"error": …}` with exit 1 on a bad address. Two things make it awkward to read:
- `source` is a string for a mechanism (`"ispdb"`) and an object for a provider rule (`{"provider": "google"}`);
- the OAuth methods' fields are snake_case (`authorization_endpoint`, `token_endpoint`, `device_authorization_endpoint`) inside an otherwise camelCase document.

## What

- **One shape for `source`:** always a string naming the mechanism (`provider`, `pacc`, `ispMain`, `ispFallback`, `mailconf`, `ispdb`, `srv`, `dav`, `jmap`), and the provider, when a fixed rule matched, in its own field of the config (`provider: "google" | "microsoft"`).
- **camelCase keys throughout**, the OAuth methods' fields included (`authorizationEndpoint`, `tokenEndpoint`, `deviceAuthorizationEndpoint`).
- **No merging in the output.** Each config stays the answer of one source; an endpoint named by several sources appears once per source. Deduplicating is the consumer's choice, as needs differ (MOA merges by endpoint and keeps every source, to apply its trust rules).

Breaking for a reader of the current JSON; the library types change their serde attributes only.
