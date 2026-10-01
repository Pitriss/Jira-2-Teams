# Testing and validation

Before a commit intended for deployment or release, run:

```bash
bash scripts/validate.sh
```

The validation performs:

1. `cargo fmt --check`
2. release unit tests
3. `cargo clippy --release -- -D warnings`
4. static `x86_64-unknown-linux-musl` release build
5. Debian `amd64` package build
6. inspection of generated `.deb` contents
7. `git diff --check`

## Unit tests

Regression tests cover behavior that has caused real integration failures or carries compatibility risk:

- Microsoft device-code timing fields as JSON numbers or numeric strings,
- the `verification_uri` compatibility alias,
- URL stripping for Teams quarantine mode,
- HTML escaping,
- nested Teams/Skype token extraction,
- character-safe diagnostic truncation,
- migration of the v0.3.x Jira state format,
- rich-state round trips,
- resolved-ticket classification,
- reassignment classification,
- Teams transport default and inference behavior,
- explicit consumer selection when a webhook URL is also present,
- rejection of unsupported transport names.

## Integration diagnostics

These scripts use real configured services and are therefore not part of the automatic unit test suite.

Gather:

```bash
bash scripts/diagnostics/gather.sh
```

End-to-end Jira-to-Teams test:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

Never put production credentials into committed fixtures or test data.

## Structured webhook payload

For v0.5.1, verify that webhook messages keep the compatibility `text` field and also expose structured fields such as `key`, `summary`, `status`, `old_status`, `assignee`, `old_assignee`, and `url`.

When a newly tracked issue was assigned to the current Jira user, test that Jira changelog enrichment reports the previous assignee. Changelog lookup is best-effort: a history API failure must not suppress the notification itself.

The webhook-mode `--test-teams` command sends a synthetic structured event with `old_assignee`, `assignee`, `old_status`, `status`, and `url`. Use it to validate the Power Automate field mapping without modifying a real Jira issue.
