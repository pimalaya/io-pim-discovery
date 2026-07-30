---
cairn: spec
capability: packaging
status: current
---

# Packaging

io-pim-discovery is an I/O-free discovery library for PIM (email, calendar, contacts) services, shipped as both a `no_std` library and an optional `std` CLI binary. Given an email address or a domain, it finds where a user's mail, calendar or contacts live and how to authenticate.

### Requirement: Layered io-free design
The crate SHALL be layered like the rest of the io-* family. The coroutine layer is `no_std` state machines that emit read and write requests without performing any I/O, one module per mechanism or RFC. The client layer wraps them into standard, blocking clients over a stream. The CLI layer, behind the `cli` feature, exposes discovery as a command-line binary.

### Requirement: no_std library, std binary
The library SHALL target `no_std` (only `alloc`). `std` is pulled in only behind the `client` or `cli` features. The binary is `std`, as usual.

### Requirement: One feature per mechanism
Every discovery mechanism SHALL sit behind its own cargo feature named after its mechanism or RFC (`autoconfig`, `pacc`, `rfc6186`, `rfc6764`, `rfc8620`, `rfc8414`, `rfc9728`). The `compose` and `coroutine` modules compile only when at least one mechanism feature is on; the `compose::client` std orchestrator is further gated on the `stream` feature. The `cli` feature turns on the mechanisms the `pim-discovery` binary exposes.

### Requirement: Strict Discovery prefix
Every public item SHALL carry the `Discovery` prefix per the Pimalaya naming guidelines (`DiscoveryComposeClientStd`, `DiscoverySrvReport`, `DiscoveryEndpoint`). Only the CLI command types keep their unprefixed names.
