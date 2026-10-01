# Deployment

## User service

Jira2Teams používá `systemd --user`. Proces zůstává ve foregroundu a lifecycle, restart a logování řeší systemd.

```bash
./deploy.sh install-user
systemctl --user status jira2teams.service
journalctl --user -u jira2teams.service -f
```

Pro běh po rebootu bez interaktivního loginu:

```bash
sudo loginctl enable-linger "$USER"
```

## Runtime soubory

```text
~/.local/bin/jira2teams
~/.config/jira2teams/jira2teams.env
~/.config/jira2teams/teams-auth.json
~/.cache/jira2teams/state.json
```

`jira2teams.env` a `teams-auth.json` mají být `0600`.

## Debian package

```bash
./deploy.sh package-deb
sudo dpkg -i dist/jira2teams_*_amd64.deb
```

Balíček neaktivuje user service automaticky, protože nemá rozhodovat, pod kterým uživatelem má Teams/Jira účet běžet.

Po instalaci `.deb`:

```bash
mkdir -p ~/.config/jira2teams
cp /usr/share/doc/jira2teams/examples/jira2teams.env.example ~/.config/jira2teams/jira2teams.env
chmod 600 ~/.config/jira2teams/jira2teams.env
jira2teams --login
systemctl --user enable --now jira2teams.service
```
