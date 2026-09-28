---
cairn: log
change: dav-context-validation
landed: 2026-09-28
---

# Validate the DAV context root

RFC 6764 resolution never failed to produce a CalDAV/CardDAV URL: `DiscoveryWellKnown` returned the first `.well-known` redirect target unchecked, the resolve fell back to the bare origin when there was none, and a TXT path was returned without a request. ik.me listed its login form as both services and example.com its home page, the DAV side of the flaw the `jmap-session-validation` change fixed the same day.

Every candidate, in order the TXT path, the `.well-known` target and the origin, is now checked by the new `DiscoveryDavContext`: the `PROPFIND` of the RFC 6764 §6 bootstrap (`Depth: 0`, `current-user-principal`), following the whole redirect chain, qualifying on a `207 Multi-Status` or a challenged `401`. GET could not separate a DAV server from a web page (iCloud answers it 400, the ik.me login form 200); `PROPFIND` gets a challenged 401 from fastmail, sync.infomaniak.com, posteo and iCloud, and a 405 from the login form and example.com. `DiscoveryWellKnown` keeps its redirect requirement and `DiscoveryDavResolve` fails with a new `NotFound` when no candidate qualifies; neither return type changed, since pimalaya-linux (path dependency) and pimalaya-android consume both.

The redirect walk, copied in the JMAP well-known and RFC 9110 auth probes and now needed by DAV, moved into `shared::walk::DiscoveryHttpWalk`, a request template replayed on each hop; both probes were ported onto it, their tests unchanged and green.

Verified live with `pim-discovery all`: ik.me and example.com yield no DAV config; posteo (`:8443`, `:8843`), fastmail (`d277161.*.fastmail.com/dav/*`) and iCloud (`caldav.icloud.com`, `contacts.icloud.com`) keep theirs. Seven tests cover the 207, challenged and unchallenged 401, the 405, a two-hop chain, a login-page redirect and an unredirected origin. pimalaya-linux could not be fully checked: io-pim-discovery compiles in it, but the build stops on an unrelated io-pimdir error.

Spec updated: mechanisms (MODIFIED: RFC 6764 DAV).

Not done: the checking `PROPFIND` sees the server's challenge, which would give DAV configs real auth schemes instead of the assumed password (iCloud's GET-based auth probe gets a 400), but threading them out changes the two return types above.
