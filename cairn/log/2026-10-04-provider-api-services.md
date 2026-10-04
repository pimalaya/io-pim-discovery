---
cairn: log
change: provider-api-services
landed: 2026-10-04
---

# Provider API services

`DiscoveryService` gained six kinds named after neverest's backends: `Gmail`, `Gcal`, `Gpeople`, `Msgraph`, `MsgraphCalendar`, `MsgraphContacts` (camelCase on the wire). The Google rule now yields the Gmail API (`https://gmail.googleapis.com/`), Google Calendar (`https://www.googleapis.com/calendar/v3/`) and Google People (`https://people.googleapis.com/`), authorization code grant only, after its IMAP, POP3, SMTP, CalDAV and CardDAV configs. The Microsoft rule yields Graph (`https://graph.microsoft.com/v1.0/`) for mail (`Mail.ReadWrite Mail.Send`), calendars (`Calendars.ReadWrite`) and contacts (`Contacts.ReadWrite`), with the same `common` endpoints and grants as its IMAP config. A Microsoft address now has calendar and contact services.

The provider APIs are left out of the unauthenticated auth probe (`probe_urls` is empty for them): their grants are fixed, and Graph's challenge would only add a bare `bearer`.

CLI: `email` lists `gmail` and `msgraph`, `calendar` `gcal` and `msgraphCalendar`, `contact` `gpeople` and `msgraphContacts`, and `all` shows them in its sections. `email is-google` and `email is-microsoft` now print the provider's configs of every domain instead of the mail ones only.

Wizards checked (they depend on 0.8 from crates.io, so nothing breaks before they bump): cardamum, himalaya, himalaya-tui, neverest, carillon and calendula filter on the services they speak and ignore the new kinds; ortie's `service_name` matches `DiscoveryService` exhaustively and needs six arms when it bumps. pimalaya-linux (path dependency) has no exhaustive match.

Spec updated: compose (ADDED: Provider API services), cli (MODIFIED: Domain-organised commands).

Verified: tests (three new: each rule's services and scopes, provider APIs unprobed), clippy (one pre-existing warning in `pacc/config.rs`), fmt; `pim-discovery email is-microsoft test@outlook.fr` and `all test@gmail.com` live.
