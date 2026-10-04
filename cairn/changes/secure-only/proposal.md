---
cairn: change
id: secure-only
status: landed
created: 2026-10-04
---

# Secure-only discovery

Part of the io-pim-discovery work MOA needs (MOA `docs/plan/discovery.md`), with [provider-api-services](../provider-api-services/proposal.md) and [consistent-json](../consistent-json/proposal.md).

## Why

Discovery decides where a user's credentials go. A production client (MOA) requires TLS end to end: an answer fetched over plain HTTP can be rewritten on the way, and a server reached without TLS reads the password. Today, with nothing in the output to tell them apart:
- autoconfig tries `http://autoconfig.<domain>/…` and `http://<domain>/.well-known/autoconfig/…` after their `https://` flavours (`autoconfig/isp.rs`), and the mailconf TXT target may be `http://`;
- RFC 6764 builds an `http://` origin from a `_caldav._tcp` / `_carddav._tcp` record when no TLS record exists (`rfc6764/resolve.rs`);
- an autoconfig document may list `plain` servers (`DiscoverySecurity::Plain`).

## What

- **An option, off by default:** `secure_only` on the client (library), `--secure-only` on every CLI command. Default behaviour is unchanged for the tools' wizards.
- **With it:**
  - no request over plain HTTP: the `http://` autoconfig URLs are not fetched, a mailconf target or a redirect to `http://` is not followed;
  - RFC 6764 uses the TLS SRV records (`_caldavs._tcp`, `_carddavs._tcp`) or `https` on the domain, never an `http://` origin;
  - a config whose endpoint is `plain` or `http://` is dropped. STARTTLS stays: it is TLS, required by the consumer once offered (no fallback to plain).
- **DNS** is out of this change's reach: the SRV and TXT lookups go to the configured resolver. A caller wanting more than plain UDP passes an RFC 8484 resolver (`--server https://…`), already supported.

## Open question, settled

- Should `--secure-only` also refuse a plain `host:port` resolver, making DNS over HTTPS the only choice in that mode? It protects the lookups' transport, not their authenticity (no DNSSEC).

  Settled 2026-10-04: no. DNS over HTTPS only moves the trust from the network to the resolver, without DNSSEC an answer is no more authentic, and refusing the default resolver would make the flag fail out of the box. A caller wanting it passes `--server https://…`; the flag's help says the lookups stay on the resolver.
