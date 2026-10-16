---
cairn: change
id: mx-autoconfig
status: landed
created: 2026-10-16
---

# MX provider autoconfig and alias following

## Why

`test@migadu.net` discovered nothing, in the CLI and in the Android app. The domain publishes only an MX record (`migadu-48722629.mx.flatmail.com`): no SRV, no TXT, no autoconfig host, and no ISPDB entry. Thunderbird finds it through its MX fallback: `autoconfig.flatmail.com`, an alias of `autoconfig.migadu.com`. The MX mechanism only matched Google and Microsoft, and `mx_parent_domain`, unused, stripped one label (`mx.flatmail.com`) and could reach the parent of a public suffix (`autoconfig.co.uk`), open to registration.

`autoconfig.flatmail.com` then serves a certificate for `admin.migadu.com` only: the alias fails over https, its target does not. The same holds for any provider aliasing its customers' `autoconfig.<domain>` to its own host.

## What

- An MX autoconfig mechanism after the ISPDB: the registrable domain of each MX host (Public Suffix List), other than the address's, tried at its ISP main URL then in the ISPDB, tagged `Mx`.
- CNAME following on the ISP main URL, own and MX provider's: when it fails and its host is an alias, the target of the chain is fetched. The alias is trusted as much as the DNS answer carrying it, the trust the MX record already gets; https only, as before.
- `mx_parent_domain` replaced by `mx_base_domain`.
