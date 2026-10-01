# Troubleshooting

## `Bearer token missing`

Direct Teams Personal username/password authentication is not supported by Jira2Teams.

Use device-code login:

```bash
jira2teams --login
```

## `QuarantineURLsBlocked`

Observed response:

```text
QuarantineURLsBlocked
URLs are not allowed in messages in quarantine mode
```

This means the Microsoft consumer messaging backend has placed the account/session into a restricted anti-abuse state where URLs are rejected.

Jira2Teams strips URLs from Jira notification text as a compatibility measure. This can allow plain notifications to continue, but it does **not** remove the underlying account restriction.

If Microsoft locks or restricts the account:

1. stop automated sending,
2. sign in interactively to the Microsoft account,
3. complete Microsoft's official verification or recovery process,
4. resume only after the account is usable normally.

Do not try to bypass the restriction by adding random delays, simulating typing, changing fingerprints, or otherwise pretending that automated traffic is human activity.

See `docs/TEAMS-CONSUMER-RISK.md`.

## `410 Gone` during endpoint registration

Jira2Teams does not need the legacy endpoint registration flow for its polling-and-send use case. Current builds use regional Teams chat endpoints returned by authentication.

## No Jira notification was sent

Inspect current Jira data and persisted state:

```bash
bash scripts/diagnostics/gather.sh
```

Run the real end-to-end test:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

Since v0.4.0, Jira2Teams also logs a compact per-poll summary with the number of tracked issues, detected changes, and issues that left the JQL result.

## An issue disappeared after resolution or reassignment

v0.4.0 explicitly handles this case.

Jira2Teams:

1. fetches the complete paginated JQL result,
2. compares it with persisted state,
3. fetches issue detail for keys that disappeared,
4. classifies resolution or reassignment when possible,
5. sends a generic "left tracked JQL" notification otherwise.

## systemd

Status:

```bash
systemctl --user status jira2teams.service
```

Recent logs:

```bash
journalctl --user -u jira2teams.service -n 100 --no-pager
```

Follow logs:

```bash
journalctl --user -u jira2teams.service -f
```

## Microsoft refresh token was revoked

Run:

```bash
systemctl --user stop jira2teams.service
jira2teams --login
systemctl --user start jira2teams.service
```

## Jira state migration

The v0.3.x state file is accepted automatically. After the first successful v0.4.x poll, it is rewritten in the richer object format.

Keep a backup before testing a development build if rollback to v0.3.x is required.
