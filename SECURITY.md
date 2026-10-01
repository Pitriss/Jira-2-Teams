# Security

Jira2Teams handles two credentials that must be treated as secrets:

- the Jira API token,
- the Microsoft/Teams refresh token stored in `teams-auth.json`.

Do not commit either secret.

Recommended local permissions:

```text
~/.config/jira2teams/                 0700
~/.config/jira2teams/jira2teams.env  0600
~/.config/jira2teams/teams-auth.json 0600
~/.cache/jira2teams/                  0700
```

The Debian package intentionally does not contain account configuration,
tokens, or Jira state.

## Private Teams API

Teams Personal message delivery uses Microsoft's consumer chat backend rather
than Microsoft Graph. This is not a public stable API and can change without
notice. Treat API breakage as an integration failure, not as an authentication
reason to log secrets or full token responses.

## Diagnostic output

Before sharing logs, verify that they do not contain:

- Jira API tokens,
- OAuth access or refresh tokens,
- Teams `skypetoken` values,
- credentials copied from environment files.
