---
cairn: change
id: bounds-and-host-names
status: landed
created: 2026-11-13
---

# Discovery keeps its deadline and takes host names only

From the family audit of 2026-11-13 (DISC-07, DISC-08, DISC-14), with two feature-shape repairs found on the way.

## Why

- **The deadline stopped before the last two passes.** `compose_all_within` bounds the mechanisms and the DAV root resolution, then ran the auth probe and the OAuth resolution on scoped threads, which are joined. A discovered host that drops packets held the call for as long as the operating system waits on a connect, minutes past a deadline of a few seconds.
- **The default DNS stream had no timeout.** The `tcp` factory of the stream pool opened a bare `TcpStream`: a network that accepts connections on port 53 and never answers blocked the resolver check, hence every `compose_*` and every CLI command, for ever. The HTTP factories have carried a read deadline since the stream-0-3 change; the DNS one was left out.
- **The address entered URLs raw.** `DiscoveryIsp::main_url` formatted the address into the query, so `anna+lists@example.org` reached the server as `anna lists@example.org` and a `&` or a `#` in the local part added parameters or cut the query. Every helper also interpolated the domain into a host or a path: `example.org:8443/x?` steered the fetch to another port and path. The `compose_*` entry points were spared by accident, the DNS name parser refusing those characters and blaming the resolver; the single-mechanism entry points and the URL helpers took the domain as it came.
- **Two feature sets did not hold.** The tests did not compile with `stream` and `rfc6764` alone, and `stream` with `rfc6186` alone warned about an unused method.

## What

- **Every pass is bounded.** The auth probe and the OAuth resolution, its resource and issuer steps both, take the time left and run as the DAV root resolution does: one detached thread per task, a task still running at the deadline abandoned and its config left as it was. No pass starts once no time is left.
- **The `tcp` factory times out.** A connect, a read and a write each give up after ten seconds. `DiscoveryStreamPool::with_tcp_factory` registers the factory with another timeout.
- **A domain is a host name or an error.** The compose client parses the domain of its input as a host name once, and uses its lowercased ASCII form from there on. `check_resolver` and every `compose_*` return the new `InvalidDomain` error before any request; the single-mechanism entry points yield nothing. The URL helpers of `DiscoveryIsp`, `DiscoveryPacc::url` and the origins of the DAV and JMAP resolutions fail on a host that is not one, so other runtimes get the same guarantee. `main_url` builds its query with `append_pair`, as `host_url` already did.
- **The gates.** The test module imports what its DAV helper needs, and `keep_secure` is compiled only with the mechanisms that call it.

## Not in this change

- Parsing the domain as a host name converts an internationalized domain to its A-labels on the way, which is most of what the audit asks under DISC-09. That line is not closed here: it has its own checks to write.
