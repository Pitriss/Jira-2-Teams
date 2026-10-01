# Security

Jira2Teams handles credentials and persistent account tokens.

## Secrets

Treat these as secrets:

- `JIRA_API_TOKEN`
- the Microsoft/Teams refresh token in `teams-auth.json`
- Teams/Skype access tokens that may appear in debug responses
- future webhook URLs, because possession of a webhook URL can grant message-posting capability

Do not commit any of them.

Recommended local permissions:

```text
~/.config/jira2teams/                 0700
~/.config/jira2teams/jira2teams.env  0600
~/.config/jira2teams/teams-auth.json 0600
~/.cache/jira2teams/                  0700
~/.cache/jira2teams/state.json        0600
```

The Debian package intentionally contains no account configuration, tokens, or Jira state.

## Unsupported Teams consumer API

The Teams Personal consumer transport is not a public stable API. Treat endpoint failures and account restrictions as integration failures.

Do not respond to Microsoft anti-abuse restrictions by trying to imitate human behavior or bypass access controls.

See `docs/TEAMS-CONSUMER-RISK.md`.

## Diagnostic output

Before sharing logs, verify that they do not contain:

- Jira API tokens,
- OAuth access or refresh tokens,
- Teams `skypetoken` values,
- webhook URLs,
- credentials copied from environment files.

## State data

`state.json` can contain Jira issue keys, summaries, status, assignee display names, and resolution names. Protect it accordingly even though it does not contain the Jira API token.
