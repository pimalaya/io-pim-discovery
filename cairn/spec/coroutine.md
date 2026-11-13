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
DNS-backed mechanisms SHALL share one transport that speaks DNS-over-TCP (length-prefixed framing) or RFC 8484 DNS-over-HTTPS, picked from the resolver URL scheme. Query names are made absolute (trailing dot) and an internationalized name is turned into its A-label form before parsing, since the `domain` parser rejects relative names and non-ASCII labels. A response whose code is neither `NOERROR` nor `NXDOMAIN` SHALL fail the lookup with that code instead of reading as an empty answer; the resolver probe alone returns it as its result.

### Requirement: Std clients over a stream pool
The client layer SHALL drive each coroutine end-to-end through a `DiscoveryStreamPool` that opens and reuses one stream per endpoint URL, with pluggable per-scheme factories (`tcp` for DNS, `https` for HTTP mechanisms). SRV discovery never opens HTTPS, so its client needs only the `tcp` factory. The `tcp` factory SHALL bound each connect, read and write by a timeout, ten seconds unless the caller registers another.

### Requirement: Redirects stay on HTTP and never leave TLS
The shared redirect walk SHALL follow a redirect to an `https` URL, and to an `http` URL only from an `http` one. A redirect to any other scheme, or from `https` to `http`, SHALL end the walk without a result, as a looping chain does.

#### Scenario: A redirect to a raw TCP URL
- GIVEN a probed server answering `Location: tcp://10.0.0.5:6379/`
- WHEN the walk reads it
- THEN no request is written to that URL and the walk ends without a result
