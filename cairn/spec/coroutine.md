---
cairn: spec
capability: coroutine
status: current
---

# Coroutine contract

Every mechanism is an I/O-free coroutine that drives its protocol as a state machine, emitting read and write requests the caller services against a real stream. The shared module holds the DNS and HTTP plumbing the mechanisms have in common.

### Requirement: I/O-free state machines
Each mechanism SHALL implement `DiscoveryCoroutine`, resumed with the bytes read from a stream and yielding `DiscoveryYield::WantsRead` / `DiscoveryYield::WantsWrite` until it returns its result. The coroutine SHALL perform no I/O itself: it only parses input and produces the next request.

### Requirement: Yields carry their endpoint
Every `WantsRead` / `WantsWrite` SHALL carry the URL of the endpoint it targets (the DNS resolver, or an HTTPS host), so the runtime routes the bytes to the correct stream when several are multiplexed.

### Requirement: Shared DNS transport
DNS-backed mechanisms SHALL share one transport that speaks DNS-over-TCP (length-prefixed framing) or RFC 8484 DNS-over-HTTPS, picked from the resolver URL scheme. Query names are made absolute (trailing dot) before parsing, since the `domain` parser rejects relative names.

### Requirement: Std clients over a stream pool
The client layer SHALL drive each coroutine end-to-end through a `DiscoveryStreamPool` that opens and reuses one stream per endpoint URL, with pluggable per-scheme factories (`tcp` for DNS, `https` for HTTP mechanisms). SRV discovery never opens HTTPS, so its client needs only the `tcp` factory.
