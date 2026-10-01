# Changelog

## 0.5.1 - 2026-10-01

- send a structured versioned JSON payload to the Teams Workflow webhook while retaining the legacy `text` field,
- expose `event`, `key`, `summary`, `status`, `old_status`, `assignee`, `old_assignee`, `resolution`, and `url`,
- enrich newly tracked issues with Jira changelog data so assignment notifications can show the previous assignee,
- include a simultaneous status transition when it is part of the same Jira changelog entry as the assignment,
- keep changelog enrichment best-effort so Jira notifications still send when history retrieval fails,
- add regression tests for structured payloads and Jira changelog assignment extraction.
- make webhook-mode `--test-teams` send a synthetic structured event so Workflow mappings can be validated without modifying Jira.

## 0.5.0 - 2026-10-01

- add a transport abstraction for Teams message delivery,
- add Microsoft Teams Workflows webhook transport,
- keep the existing Teams Personal consumer backend as an explicit legacy/experimental fallback,
- infer webhook mode when `TEAMS_WEBHOOK_URL` is configured and no transport is explicitly selected,
- keep `consumer` as the compatibility default when no webhook is configured,
- send simple JSON `{ "text": ... }` payloads to Teams Workflows,
- add retry/backoff handling for webhook HTTP 429 and 5xx responses,
- enforce the documented 28 KB webhook payload limit,
- keep webhook URLs out of error messages and document them as secrets,
- include Jira URLs in webhook notifications while continuing to strip URLs for the consumer transport,
- add transport-selection regression tests,
- document Workflows setup and the v0.5 unauthenticated trigger scope.
- add GitHub Actions CI for pushes and automatic Debian release publishing for version tags.
- refuse tagged releases whose commit is not already contained in `main`.

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
