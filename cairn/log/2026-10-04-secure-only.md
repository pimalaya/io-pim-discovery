---
cairn: log
change: secure-only
landed: 2026-10-04
---

# Secure-only discovery

`DiscoveryComposeClientStd::with_secure_only(true)`, `--secure-only` on every CLI command (it sits in the shared `ServerArg`), refuses plain transports. Off by default; the wizards see no change.

The guard is the stream pool: a secure-only client builds its pools without the `http` factory (new `DiscoveryStreamPool::without_factory`), so no mechanism can open a plain HTTP connection, whatever URL it was handed or redirected to; the coroutine sees EOF and the mechanism is skipped. On top of it, a mailconf TXT target that is not `https` is skipped before any request, `DiscoveryDavResolve::with_secure_only` ignores the plain `_caldav` / `_carddav` record so the origin is the TLS record's or `https` on the domain, and the PACC, autoconfig, DAV and JMAP runners drop every config failing the new `DiscoveryServiceConfig::is_secure` (`plain` TCP or a non-`https` URL; STARTTLS passes). SRV and the provider rules only ever yield TLS or STARTTLS.

The proposal's premise on autoconfig was partly off: the compose client already fetched only the `https://` ISP URLs (the `http://` flavours are listed by `DiscoveryIsp::all_urls` for other runtimes), and the autoconfig GET refuses redirects. The plain paths that existed were the mailconf target, the `http://` DAV origin, redirects in the DAV, JMAP and auth walks, and `plain` servers in autoconfig documents.

Resolver question settled: secure-only does not refuse a plain resolver (see the proposal).

Spec updated: compose (ADDED: Secure-only discovery), cli (MODIFIED: Shared resolver flag).

Verified: tests (five new: the pool refusing an unregistered scheme, the DAV origin with and without the option, `is_secure` on each endpoint shape), clippy (pre-existing warning only), fmt, feature subsets. Live: `auth http --secure-only http://neverssl.com/` opens no stream; `all` on posteo.de, fastmail.com, gmx.de and laposte.net returns the same configs with and without the flag (none of them advertises a plain endpoint).
