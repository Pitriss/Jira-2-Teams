# Microsoft Teams Personal consumer transport: risk and account restrictions

## Status

The current Jira2Teams Teams Personal transport uses Microsoft's consumer chat backend. It is reverse engineered and is **not a documented public messaging API**.

That has two consequences:

1. Microsoft can change endpoints, token exchange behavior, payload requirements, or access controls without notice.
2. Automated use can interact with Microsoft anti-abuse controls intended for human consumer accounts.

## Observed restriction

During Jira2Teams development, the consumer endpoint returned:

```text
HTTP 403 Forbidden
QuarantineURLsBlocked
URLs are not allowed in messages in quarantine mode
```

This is direct evidence that the account/session was operating under a Microsoft quarantine restriction.

Microsoft does not publicly document the exact scoring rules that placed this particular account into quarantine, so Jira2Teams cannot truthfully identify one exact trigger.

Reasonable technical risk factors include:

- a consumer account being used primarily as an unattended sender,
- repeated machine-generated messages,
- use of an undocumented consumer backend,
- messages containing external URLs,
- traffic patterns that differ from normal interactive Teams use.

These are risk factors and engineering inferences, not claims about Microsoft's private detection algorithm.

## What Jira2Teams does

The consumer transport removes URLs from notification text before sending. This specifically avoids the observed URL restriction and keeps notifications useful where plain text is still permitted.

This workaround does not guarantee that the account will remain unrestricted.

## What Jira2Teams should not do

Jira2Teams should not attempt to defeat anti-abuse controls by:

- randomizing delays to imitate a person,
- simulating typing or presence,
- spoofing client fingerprints,
- cycling accounts or identities,
- retrying aggressively after account restrictions,
- hiding automation from Microsoft.

Those approaches are brittle and turn an integration problem into an anti-abuse evasion problem.

## Account recovery

If Microsoft restricts or locks the account:

1. stop Jira2Teams sending,
2. sign in interactively,
3. use Microsoft's official account verification/recovery process,
4. confirm that normal Teams/Microsoft account use works,
5. only then resume automation if you accept the consumer API risk.

Microsoft account lock guidance:

https://support.microsoft.com/account-billing/account-has-been-locked-805e8b0d-4141-29b2-7b65-df6ff6c9ce27

## Recommended supported direction

Microsoft currently documents Teams Workflows that can receive an HTTP webhook and post a message or Adaptive Card to a Teams channel or chat.

Documentation:

https://learn.microsoft.com/microsoftteams/platform/webhooks-and-connectors/how-to/add-incoming-webhook

This transport is implemented in Jira2Teams v0.5.0 and is the preferred production direction:

```text
Jira Cloud
    |
    v
jira2teams
    |
    | HTTPS POST
    v
Teams Workflow webhook
    |
    v
Teams chat or channel
```

When `TEAMS_TRANSPORT=webhook` is selected, Jira2Teams does not initialize the consumer Teams OAuth/Skype-token flow. This removes the need for Jira2Teams to hold a Teams Personal refresh token or call the private consumer chat backend.

The v0.5.0 webhook implementation targets a Teams Workflow trigger configured with authentication **Anyone**. The callback URL must be treated as a secret. Tenant-authenticated Workflow triggers require an OAuth token and are outside the v0.5.0 scope.

Another supported architecture is a Teams bot/agent with proactive messaging. Microsoft documents proactive one-to-one messages when the app is installed in personal scope:

https://learn.microsoft.com/microsoftteams/platform/bots/how-to/conversations/send-proactive-messages

The current consumer transport can remain as an experimental compatibility backend, but it should not be presented as a stable supported Microsoft integration.
