---
cairn: delta
change: provider-api-services
---

# Delta

## ADDED Requirements

### Requirement: Provider API services
The service kinds SHALL include the providers' own APIs: `gmail` (Gmail API), `gcal` (Google Calendar), `gpeople` (Google People), `msgraph` (Microsoft Graph mail), `msgraphCalendar` and `msgraphContacts`. The Google fixed rule SHALL yield `gmail`, `gcal` and `gpeople` and the Microsoft fixed rule `msgraph`, `msgraphCalendar` and `msgraphContacts`, each with its API base URL and its OAuth scope, next to the protocol services the rules already yield, so a consumer can offer the choice.

#### Scenario: A Google address offers the API or the protocol
- GIVEN a `gmail.com` address
- WHEN every config is composed
- THEN both `gmail` and `imap` are offered for mail, `gcal` and `caldav` for calendars, `gpeople` and `carddav` for contacts

#### Scenario: A Microsoft address has calendars and contacts
- GIVEN an `outlook.com` address
- WHEN every config is composed
- THEN `msgraphCalendar` and `msgraphContacts` are offered

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
