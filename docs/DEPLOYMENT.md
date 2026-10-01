# Deployment

## Runtime model

Jira2Teams is designed to run in the foreground while `systemd --user` manages lifecycle, restart policy, and logging.

```bash
./deploy.sh install-user
systemctl --user status jira2teams.service
journalctl --user -u jira2teams.service -f
```

To keep the user service available after reboot without an interactive desktop login:

```bash
sudo loginctl enable-linger "$USER"
```

Verify:

```bash
loginctl show-user "$USER" -p Linger
```

Expected result:

```text
Linger=yes
```

## Runtime files

Local user deployment uses:

```text
~/.local/bin/jira2teams
~/.config/jira2teams/jira2teams.env
~/.config/jira2teams/teams-auth.json
~/.cache/jira2teams/state.json
```

Recommended permissions:

```text
~/.config/jira2teams/                 0700
~/.config/jira2teams/jira2teams.env  0600
~/.config/jira2teams/teams-auth.json 0600
~/.cache/jira2teams/                  0700
~/.cache/jira2teams/state.json        0600
```

## Recommended Teams Workflow configuration

For production use, prefer:

```text
TEAMS_TRANSPORT=webhook
TEAMS_WEBHOOK_URL=<Teams Workflow callback URL>
```

The v0.5.0 client expects a **When a Teams webhook request is received** trigger configured with authentication **Anyone**. The workflow should map the incoming `text` property to a Teams post action.

Treat the callback URL as a secret and keep the environment file at mode `0600`.

The Workflow is owned by user accounts rather than by a team/channel object. Assign an appropriate co-owner for operational continuity.

## Debian package

Build:

```bash
./deploy.sh package-deb
```

Install:

```bash
sudo dpkg -i dist/jira2teams_*_amd64.deb
```

The package installs `/usr/bin/jira2teams` and a systemd user unit in `/usr/lib/systemd/user/jira2teams.service`.

It deliberately does not enable the service automatically because credentials and the target Teams account belong to a specific unprivileged user.

After package installation:

```bash
mkdir -p ~/.config/jira2teams
cp /usr/share/doc/jira2teams/examples/jira2teams.env.example ~/.config/jira2teams/jira2teams.env
chmod 600 ~/.config/jira2teams/jira2teams.env
jira2teams --login
systemctl --user enable --now jira2teams.service
```

## Upgrade

For a source checkout:

```bash
git pull --ff-only
./deploy.sh install-user
```

For a Debian package:

```bash
sudo dpkg -i jira2teams_<version>_amd64.deb
systemctl --user restart jira2teams.service
```

The state format from v0.3.x is migrated automatically by v0.4.0.

## GitHub release pipeline

Official release packages are built by GitHub Actions from annotated version tags. Local `.deb` builds are development artifacts only.

Release flow:

```text
validated main
    |
    | annotated tag vX.Y.Z
    v
GitHub Actions release workflow
    |
    +-- verify tag matches Cargo.toml version
    +-- run the full validation suite
    +-- build the static musl binary
    +-- build the Debian amd64 package
    +-- generate SHA-256
    +-- publish GitHub Release assets
```

Create and push a release tag only after the corresponding commit is on `main`:

```bash
git switch main
git pull --ff-only
git tag -a vX.Y.Z -m "Jira2Teams vX.Y.Z"
git push origin vX.Y.Z
```

The release workflow uses the repository-scoped GitHub Actions token with only `contents: write` permission. No personal access token is required for release publishing.
