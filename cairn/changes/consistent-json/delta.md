---
cairn: delta
change: consistent-json
---

# Delta

## ADDED Requirements

### Requirement: One JSON shape
Every config SHALL serialize with camelCase keys, its `source` as a string naming the mechanism that produced it, and, when a fixed provider rule matched, a `provider` field naming the provider.

#### Scenario: A provider config and an autoconfig one read alike
- GIVEN a Google address and a domain with an autoconfig document
- WHEN `pim-discovery --json all` runs on each
- THEN both print `source` as a string, the Google one with `provider: "google"`, and no key holds an underscore

## MODIFIED Requirements

### Requirement: Ordered mechanism fan-out
For the remaining mechanisms, composition SHALL run them in a fixed priority order (MX-derived provider, PACC, autoconfig ISP main / fallback / mailconf / ISPDB, RFC 6186 SRV, RFC 6764 CalDAV/CardDAV, RFC 8620 JMAP), scoped to the services requested, and concatenate their configs into one list in that order. Configs SHALL NOT be deduplicated: each stays the answer of one source, and an endpoint named by several sources appears once per source. Merging is the consumer's, whose needs differ.

## REMOVED Requirements

None.
