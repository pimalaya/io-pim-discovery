---
cairn: log
change: resolver-failures
landed: 2026-10-04
---

# Resolver failures are errors

How it works:
- `DiscoveryComposeClientStd::check_resolver(input)` runs the new `DiscoveryDnsProbe`: one SOA question about the domain, reading only the response code (`DiscoveryDnsRcode`, the low nibble of the fourth header byte).
- `NOERROR` and `NXDOMAIN` pass. No answer, a too-short message, an exchange error (HTTP status, HTTP error) or another code fail with `DiscoveryComposeClientStdError::Resolver`, naming the resolver and the reason.
- A plain `http://` resolver fails first with `PlainResolver` in secure-only mode. The pool would have refused the stream anyway, silently.

Where it runs:
- The fan-outs (`parallel_outputs`, `parallel_outputs_within`) run it after `plan`, so every `compose_*` does.
- The bounded one runs the probe through `collect_within` with the same deadline, then hands the mechanisms what is left.
- In the CLI, `ServerArg::checked_client` gives every `email`, `calendar`, `contact` and `file` subcommand the same check. `all` goes through `compose_raw`, and the `auth` probes make no DNS lookup.

Why a probe rather than errors carried out of each mechanism:
- the mechanisms rightly skip their own failures;
- the reader's EOF on a failed write loses the cause anyway;
- a resolver failing mid-run after a good probe stays possible but rare, against one extra query.

A resolver that drops packets fails only at the system's TCP connect timeout (135 s here), as each mechanism already did. Callers wanting less use `compose_all_within` or their own timeout. MOA wraps the binary.

Spec updated:
- compose: ADDED "Resolver failures are errors", MODIFIED "Secure-only discovery";
- cli: MODIFIED "Shared resolver flag".

Verified:
- tests (40, three new: response codes and EOF on the probe, an unreachable resolver failing `compose_raw` for `gmail.com` and a bare domain and `compose_all_within`, secure-only refusing `http://`);
- clippy (pre-existing warning only), fmt, feature subsets.

Live, `all --secure-only`:

| Resolver | Result |
|---|---|
| default (Cloudflare) | unchanged |
| Quad9 over DoH | error "answered HTTP 505" (it wants HTTP/2), exit 1 |
| `http://1.1.1.1/dns-query` | refused |
| `127.0.0.1:9` | "did not answer", exit 1; `email srv` too |

A domain that does not exist still gives `[]`, exit 0.
