---
cairn: change
id: provider-api-services
status: active
created: 2026-10-04
---

# Provider API services

Part of the io-pim-discovery work MOA needs for its account wizard (MOA `docs/plan/discovery.md`, `docs/plan/onboarding.md` step 1b), with [secure-only](../secure-only/proposal.md) and [consistent-json](../consistent-json/proposal.md).

## Why

The fixed provider rules offer Google as IMAP, POP3, SMTP, CalDAV and CardDAV, and Microsoft as IMAP, POP3 and SMTP on `outlook.office365.com`. Neither offers the providers' own APIs, which the Pimalaya sync tools speak and which are often the better choice: neverest syncs Gmail through the Gmail API (`gmail`), Google calendars and contacts through Google Calendar (`gcal`) and Google People (`gpeople`), and Microsoft mail, calendars and contacts through Graph (`msgraph`, `msgraph-calendar`, `msgraph-contacts`). Microsoft exposes calendars and contacts over Graph only, so a Microsoft address discovers no calendar and no contacts today.

A wizard, MOA's or a tool's, lists what discovery found and lets the user choose: a Google address should offer the Gmail API or IMAP and SMTP, Google Calendar or CalDAV, Google People or CardDAV; a Microsoft address Graph or IMAP and SMTP, and Graph for calendars and contacts. Discovery cannot offer what it has no kind for.

## What

- **Six service kinds**, named after neverest's backends (camelCase on the wire): `gmail`, `gcal`, `gpeople`, `msgraph`, `msgraphCalendar`, `msgraphContacts`.
- **Fixed rules yield them**, next to the protocol services already there:
  - Google (domain or MX): `gmail` (`https://gmail.googleapis.com/`, scope `https://mail.google.com/`), `gcal` (`https://www.googleapis.com/calendar/v3/`, scope `https://www.googleapis.com/auth/calendar`), `gpeople` (`https://people.googleapis.com/`, scope `https://www.googleapis.com/auth/contacts`); OAuth authorization code grant only (Google's device flow excludes these scopes).
  - Microsoft (domain or MX): `msgraph` (`https://graph.microsoft.com/v1.0/`, scopes `Mail.ReadWrite Mail.Send offline_access`), `msgraphCalendar` (`Calendars.ReadWrite`), `msgraphContacts` (`Contacts.ReadWrite`), on the same authorization and token endpoints as the IMAP rule, with the authorization code and device grants.
- **CLI:** the `email` domain lists `gmail` and `msgraph`, `calendar` lists `gcal` and `msgraphCalendar`, `contact` lists `gpeople` and `msgraphContacts`; `all` and `is-google` / `is-microsoft` include them.
- The protocol services stay: the user chooses.

## Out of scope

- The Microsoft 365 tenant of a domain (the authorization endpoints stay `common`): its own change.
- Any OAuth client id: a consumer brings its own registered application.
