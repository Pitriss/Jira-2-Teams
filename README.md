# Jira2Teams

Jira2Teams is a small statically linked Rust daemon for Linux. It polls Jira Cloud and sends one-to-one notifications to Microsoft Teams when a tracked Jira issue changes.

The project started as a replacement for a Bash watcher built around `curl`, `jq`, and `notify-send`. The production binary is built as a static `musl` executable and does not require those tools at runtime.

## Features

- Jira Cloud polling through the REST API
- configurable JQL
- persistent state across restarts
- detection of new and changed issues
- detection of issues that leave the tracked JQL result
- classification of resolved and reassigned issues when Jira detail remains accessible
- automatic migration of the legacy v0.3.x state format
- Microsoft Teams Personal one-to-one notifications
- device-code login with a persisted refresh-token cache
- `systemd --user` deployment
- static `x86_64-unknown-linux-musl` build
- Debian 13/amd64 package generation
- unit tests, diagnostics, and end-to-end test tooling

## Current Teams transport

The current Teams Personal transport does **not** use Microsoft Graph for message delivery. It uses Microsoft's consumer chat backend.

This backend is not a public or stable API. Microsoft can change or restrict it without notice. Accounts can also be placed into anti-abuse or quarantine states. One observed restriction is:

```text
QuarantineURLsBlocked
URLs are not allowed in messages in quarantine mode
```

Jira2Teams therefore strips URLs from notification text before sending through the consumer transport.

For production environments, read [docs/TEAMS-CONSUMER-RISK.md](docs/TEAMS-CONSUMER-RISK.md). A supported Teams Workflows webhook transport is the preferred future direction.

## Data flow

```text
Jira Cloud
    |
    | REST API + JQL
    v
jira2teams
    |
    | compare current Jira state with persisted state
    v
state.json
    |
    | Teams Personal consumer chat transport
    v
1:1 Teams chat
```

## Jira state model

Since v0.4.0, Jira2Teams stores richer per-issue state:

```json
{
  "K2HW-7127": {
    "updated": "2026-09-30T15:02:27.712+0200",
    "summary": "Example issue",
    "status": "Assigned",
    "assignee": "Example User",
    "resolution": null
  }
}
```

The v0.3.x format:

```json
{
  "K2HW-7127": "2026-09-30T15:02:27.712+0200"
}
```

is accepted and migrated automatically on the next successful poll.

Jira enhanced JQL search is fully paginated with `nextPageToken` before state comparison. This prevents issues on later result pages from being incorrectly classified as missing.

## Build on Debian 13

Install build dependencies:

```bash
sudo apt-get update && sudo apt-get install -y build-essential musl-dev musl-tools pkg-config dpkg-dev ca-certificates
```

Install the Rust target:

```bash
rustup target add x86_64-unknown-linux-musl
```

Build:

```bash
./deploy.sh build
```

The resulting binary is:

```text
target/x86_64-unknown-linux-musl/release/jira2teams
```

Verify static linking:

```bash
file target/x86_64-unknown-linux-musl/release/jira2teams
ldd target/x86_64-unknown-linux-musl/release/jira2teams
```

Expected output includes `static-pie linked` / `statically linked`.

## First Teams login

Run:

```bash
./target/x86_64-unknown-linux-musl/release/jira2teams --login
```

Jira2Teams displays the Microsoft device-code login URL and code. After successful login, the refresh-token cache is stored in:

```text
~/.config/jira2teams/teams-auth.json
```

The file should have mode `0600`.

The cached login normally survives both application and host restarts. If Microsoft revokes the refresh token, run `--login` again.

List available chats:

```bash
./target/x86_64-unknown-linux-musl/release/jira2teams --list-chats
```

## Configuration

The example configuration is `jira2teams.env.example`.

For `systemd --user`, use:

```text
~/.config/jira2teams/jira2teams.env
```

Important variables:

```text
JIRA_URL=https://company.atlassian.net
JIRA_EMAIL=user@example.com
JIRA_API_TOKEN=...
JQL=assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC
POLL_INTERVAL=60
MAX_RESULTS=50
TEAMS_THREAD_ID=19:...@thread.v2
```

Optional runtime paths:

```text
STATE_FILE=~/.cache/jira2teams/state.json
TEAMS_AUTH_FILE=~/.config/jira2teams/teams-auth.json
```

## One-shot tests

Send a direct Teams test message:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' ./target/x86_64-unknown-linux-musl/release/jira2teams --test-teams
```

Run one Jira poll:

```bash
./target/x86_64-unknown-linux-musl/release/jira2teams --once
```

## Background deployment

Install for the current user:

```bash
./deploy.sh install-user
```

Local deployment installs:

```text
~/.local/bin/jira2teams
~/.config/systemd/user/jira2teams.service
~/.config/jira2teams/
~/.cache/jira2teams/
```

Service status:

```bash
systemctl --user status jira2teams.service
```

Follow logs:

```bash
journalctl --user -u jira2teams.service -f
```

Enable the user manager at boot even without an interactive KDE login:

```bash
sudo loginctl enable-linger "$USER"
```

Verify:

```bash
loginctl show-user "$USER" -p Linger
```

## Debian 13 package

Build:

```bash
./deploy.sh package-deb
```

Output:

```text
dist/jira2teams_<version>_amd64.deb
```

Inspect:

```bash
dpkg-deb -c dist/jira2teams_*_amd64.deb
```

Install:

```bash
sudo dpkg -i dist/jira2teams_*_amd64.deb
```

The package installs:

```text
/usr/bin/jira2teams
/usr/lib/systemd/user/jira2teams.service
/usr/share/doc/jira2teams/
```

The Debian package intentionally does not contain Jira credentials, Teams tokens, or runtime state.

## Diagnostics

Gather Jira/state information:

```bash
bash scripts/diagnostics/gather.sh
```

Run the real Jira-to-Teams end-to-end test:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

See [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).

## Validation

Before a commit or release:

```bash
bash scripts/validate.sh
```

The validation checks formatting, unit tests, Clippy with warnings denied, static musl build, Debian package generation, package contents, and Git whitespace.

See [docs/TESTING.md](docs/TESTING.md).

## Security

Read [SECURITY.md](SECURITY.md) before deploying. In particular:

- keep `JIRA_API_TOKEN` in a mode `0600` file,
- protect `teams-auth.json` with mode `0600`,
- never commit tokens or runtime state,
- do not attempt to bypass Microsoft account restrictions by simulating human interaction.

## Development

See:

- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)
- [docs/TESTING.md](docs/TESTING.md)
- [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)
- [docs/TEAMS-CONSUMER-RISK.md](docs/TEAMS-CONSUMER-RISK.md)
