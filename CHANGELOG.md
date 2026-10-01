# Changelog

## 0.4.0 - 2026-10-01

- paginate Jira enhanced JQL search with `nextPageToken` before comparing state,
- persist rich per-ticket state (`updated`, summary, status, assignee, resolution),
- migrate legacy `key -> updated` state automatically,
- notify when a tracked ticket leaves the JQL result,
- classify resolved tickets and assignee changes when Jira detail is still accessible,
- treat Jira detail HTTP 403/404 as unavailable and send a generic missing-ticket notification,
- preserve a generic notification for tickets that are no longer accessible,
- write Jira state with mode `0600`,
- add regression tests for state migration and missing-ticket classification.

## 0.3.0 - 2026-09-30

- renamed application and binary to `jira2teams`,
- established clean Git repository layout,
- static musl build for Debian 13 amd64,
- Jira Cloud polling with persistent state,
- Teams Personal device-code authentication with refresh-token cache,
- Teams Personal 1:1 message delivery without Microsoft Graph,
- regional Teams chat endpoint discovery,
- quarantine-safe Jira notifications without URLs,
- systemd user-service deployment,
- Debian 13 `.deb` package generation,
- diagnostics and deployment documentation.
