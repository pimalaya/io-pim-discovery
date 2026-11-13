---
cairn: log
change: bounds-and-host-names
landed: 2026-11-13
---

# Discovery keeps its deadline and takes host names only

Three findings of the family audit of 2026-11-13 (DISC-07, DISC-08, DISC-14) and two feature-shape repairs.

The deadline (DISC-07):
- `probe`, `resolve_oauth` and `resolve_issuers` in src/compose/client.rs take an `Option<Duration>`. `compose` hands each pass what is left of the deadline at the moment it starts; `resolve_oauth` does the same between its resource step and its issuer step. `compose_raw`, `compose_all` and `compose_first` pass `None`.
- A new private `collect(tasks, deadline)` holds the choice the DAV root resolution already made by hand: `collect_within` under a deadline, scoped threads joined without one. The three passes and `resolve_dav_roots` go through it, their tasks owning a shallow clone of the client as the mechanisms' do.
- A task that misses the deadline leaves its slot empty: the config keeps its auth, the endpoint gains no issuer, the issuer stays bare. Its thread ends in the background.
- `collect_within` returns at once, starting no thread, when the deadline is zero. Before, a pass reached with no time left still opened its connections and dropped their answers.
- The doc comment of `collect_within` had drifted onto `dav_root_target`: it is back on its function.

The DNS stream (DISC-08):
- The `tcp` factory resolves the host, connects to each address with `TcpStream::connect_timeout` and arms read and write timeouts on the stream it returns. The timeout is ten seconds by default, in `DiscoveryStreamPool::new`.
- New `DiscoveryStreamPool::with_tcp_factory(timeout)` registers the factory with another value. It is what the test uses, and what a caller with a tighter budget than ten seconds has been missing.
- A read that times out reaches the coroutine as the end of the stream, like any other read failure in the compose client: the resolver check reports the resolver as unusable. The connect half was recorded on 2026-10-04 as failing at the system timeout (135 s): it is ten seconds too now.
- Ten seconds is a choice, not a measurement: one DNS exchange is a single round trip, and the HTTP streams keep their sixty.

The address and the domain (DISC-14):
- `DiscoveryIsp::main_url` builds its host and hands over to `host_url`, whose query goes through `append_pair`. `anna+lists&x=1#y@example.org` now arrives as that one value; before, the query decoded to `emailaddress=anna lists` and `x=1`.
- `main_url`, `host_url`, `fallback_url` and `db_url` parse their domain or host with `url::Host::parse` and format the parsed value, so a port, a path, a query or a user part is an error instead of a part of the URL. `DiscoveryPacc::url` does the same, and `DiscoveryDavResolve` and `DiscoveryJmapResolve` check the host of their origin, the domain or an SRV target, returning `InvalidOrigin`.
- In the compose client `domain_part` returns the domain as a host name or the new `DiscoveryComposeClientStdError::InvalidDomain`. `plan` and `check_resolver` propagate it, so `compose_*` no longer blames the resolver for `example.org:8443/x?`. `autoconfig`, `srv`, `pacc`, `dav`, `jmap`, `provider`, `is_google` and `is_microsoft` return nothing for such an input. An IP address is refused as a mail domain: nothing here can be looked up under it.
- The value kept is the one `Host::parse` returns: lowercased, and in A-labels for an internationalized domain. That is the conversion DISC-09 asks for, arriving as a side effect; one assertion pins it, the rest of that finding is not claimed here.

Feature shapes:
- With `stream` and `rfc6764` alone the tests did not compile: their DAV helper names `DiscoveryConfigSource`, imported at the top of the file under `autoconfig` only. The test module imports it itself.
- With `stream` and `rfc6186` alone `keep_secure` was never used: it is compiled with the four mechanisms that call it.

Capabilities moved: `compose` (ADDED "The domain is a host name"; MODIFIED "Deadline-bounded composition"), `mechanisms` (ADDED "URLs take host names and encoded values"), `coroutine` (MODIFIED "Std clients over a stream pool").

Verified: `cargo test --all-features` (67 passed, 8 new, each seen failing on the code before its fix), `cargo clippy --all-features --all-targets -- -D warnings` clean on the full set and on nineteen feature subsets, `cargo build --no-default-features`, `cargo doc` with broken links denied. The deadline test holds two passes against a local host that accepts and stays silent for three seconds: six seconds before the fix, under half a second after. No live run.
