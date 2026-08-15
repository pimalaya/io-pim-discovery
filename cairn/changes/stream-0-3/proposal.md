---
cairn: change
id: stream-0-3
status: landed
created: 2026-08-15
---

# pimalaya-stream 0.3

## Why

pimalaya-stream 0.3 renames `StreamStd` to `stream::Stream`, flattens the `std` module away and moves each connect onto a per-transport options struct. This crate's `with_http_factories` calls two of those constructors, so it does not compile against the new transport until it follows.

The bump is worth taking on its own merits: the new `Read` and `Write` retry a stream reporting it is not ready, which is the failure behind himalaya#731 and himalaya#732, and every HTTPS probe this crate runs inherits it.

## What

`DiscoveryStreamPool::with_http_factories` builds its two factories through `Stream::connect_tcp` and `Stream::connect_tls`, each taking its options struct. The doc links that pointed at `pimalaya_stream::std::stream::StreamStd` follow the type to its new path.

## Cost

None to callers: the factories keep their signatures, and a caller registering its own already passes whatever stream it likes.
