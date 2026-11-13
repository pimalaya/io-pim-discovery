---
cairn: tasks
change: bounds-and-host-names
---

# Tasks

- [x] `probe`, `resolve_oauth` and `resolve_issuers` take the time left and collect through `collect_within` under a deadline.
- [x] `collect_within` starts no task once no time is left.
- [x] The `tcp` factory connects with a timeout and arms read and write timeouts; `DiscoveryStreamPool::with_tcp_factory`.
- [x] `DiscoveryIsp::main_url` builds its query with `append_pair`.
- [x] The `DiscoveryIsp` URL helpers, `DiscoveryPacc::url` and the DAV and JMAP origins fail on a host that is not a host name.
- [x] The compose client parses the domain as a host name once; `InvalidDomain`.
- [x] Gates: the tests of `stream,rfc6764`, the unused `keep_secure` of `stream,rfc6186`.
- [x] Tests, each seen failing first.
- [x] CHANGELOG, spec fold, log.
