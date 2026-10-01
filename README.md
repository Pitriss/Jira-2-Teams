# Jira2Teams

`jira2teams` je malý staticky linkovaný Rust daemon pro Linux, který periodicky sleduje Jira Cloud a při změně tiketu pošle 1:1 zprávu do Microsoft Teams Personal.

Projekt vznikl jako náhrada Bash skriptu používajícího `curl`, `jq` a `notify-send`. Výsledná binárka nepotřebuje tyto runtime nástroje a je sestavená jako statický `musl` executable.

## Co umí

- poll Jira Cloud přes REST API,
- vlastní JQL dotaz,
- detekovat změnu podle Jira `updated`,
- uchovávat stav mezi restarty,
- posílat zprávy do existujícího Teams Personal 1:1 chatu,
- jednorázový device-code login a následné použití uloženého refresh tokenu,
- běžet jako `systemd --user` služba,
- statický `x86_64-unknown-linux-musl` build,
- vytvořit `.deb` balíček pro Debian 13/amd64.

## Důležité omezení Teams

Odesílání zpráv nepoužívá Microsoft Graph. Teams Personal chat používá neveřejný consumer backend Microsoftu. Tento backend není veřejné stabilní API a Microsoft jej může změnit bez upozornění.

Některé nové osobní účty mohou být dočasně v Teams `quarantine mode`. V tomto režimu Microsoft odmítá zprávy obsahující URL (`QuarantineURLsBlocked`). Jira2Teams proto posílá notifikace bez klikatelného Jira URL a URL v textu notifikace filtruje.

## Tok dat

```text
Jira Cloud
    |
    | REST API + JQL
    v
jira2teams
    |
    | porovnání key -> updated
    v
state.json
    |
    | Teams Personal consumer auth/chat
    v
1:1 Teams chat
```

## Build na Debianu 13

Nainstaluj nástroje:

```bash
sudo apt-get update && sudo apt-get install -y build-essential musl-dev musl-tools pkg-config dpkg-dev ca-certificates
```

Rust target:

```bash
rustup target add x86_64-unknown-linux-musl
```

Build:

```bash
./deploy.sh build
```

Výsledná binárka:

```text
target/x86_64-unknown-linux-musl/release/jira2teams
```

Kontrola:

```bash
file target/x86_64-unknown-linux-musl/release/jira2teams
ldd target/x86_64-unknown-linux-musl/release/jira2teams
```

Očekávaný výsledek je `static-pie linked` / `statically linked`.

## První Teams přihlášení

Spusť:

```bash
./target/x86_64-unknown-linux-musl/release/jira2teams --login
```

Aplikace zobrazí Microsoft device-code URL a kód. Po úspěšném přihlášení uloží token cache do:

```text
~/.config/jira2teams/teams-auth.json
```

Soubor má mít práva `0600`.

Login přežívá restart aplikace i počítače. Pokud Microsoft refresh token zneplatní, je potřeba `--login` zopakovat.

Seznam dostupných chatů:

```bash
./target/x86_64-unknown-linux-musl/release/jira2teams --list-chats
```

## Konfigurace Jira a Teams

Vzor je v `jira2teams.env.example`.

Pro `systemd --user` použij:

```text
~/.config/jira2teams/jira2teams.env
```

Nejdůležitější proměnné:

```text
JIRA_URL=https://firma.atlassian.net
JIRA_EMAIL=user@example.com
JIRA_API_TOKEN=...
TEAMS_THREAD_ID=19:...@thread.v2
POLL_INTERVAL=60
MAX_RESULTS=50
```

Výchozí JQL:

```text
assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC
```

Pozor: tiket, který přestane odpovídat JQL, zmizí z aktuální množiny. Tohle je známé omezení současné detekční logiky a je vhodné ho před další stabilní verzí rozšířit o detekci tiketů, které ze sledované množiny zmizely.

## Jednorázový test

```bash
TEAMS_THREAD_ID='19:...@thread.v2' ./target/x86_64-unknown-linux-musl/release/jira2teams --test-teams
```

Jedna kontrola Jira:

```bash
./target/x86_64-unknown-linux-musl/release/jira2teams --once
```

## Deploy jako background služba

Instalace pro aktuálního uživatele:

```bash
./deploy.sh install-user
```

Instaluje:

```text
~/.local/bin/jira2teams
~/.config/systemd/user/jira2teams.service
~/.config/jira2teams/
~/.cache/jira2teams/
```

Při přechodu ze starého `jira-watch` se existující Teams token a Jira state automaticky zkopírují do nového namespace, pokud cílové soubory ještě neexistují.

Stav služby:

```bash
systemctl --user status jira2teams.service
```

Log:

```bash
journalctl --user -u jira2teams.service -f
```

Restart po nasazení nové binárky:

```bash
./deploy.sh restart-user
```

Aby user service běžela i po rebootu bez přihlášení do KDE:

```bash
sudo loginctl enable-linger "$USER"
```

Kontrola:

```bash
loginctl show-user "$USER" -p Linger
```

## Debian 13 balíček

Sestavení:

```bash
./deploy.sh package-deb
```

Výstup:

```text
dist/jira2teams_<verze>_amd64.deb
```

Kontrola obsahu:

```bash
dpkg-deb -c dist/jira2teams_*_amd64.deb
```

Instalace:

```bash
sudo dpkg -i dist/jira2teams_*_amd64.deb
```

Balíček instaluje:

```text
/usr/bin/jira2teams
/usr/lib/systemd/user/jira2teams.service
/usr/share/doc/jira2teams/
```

Debian unit používá `/usr/bin/jira2teams`; lokální `./deploy.sh install-user`
používá `~/.local/bin/jira2teams`.

Konfiguraci a Teams token záměrně nevkládá do `.deb`; jde o uživatelská tajemství.

## Diagnostika

Gather:

```bash
bash scripts/diagnostics/gather.sh
```

End-to-end test:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

Podrobnosti jsou v `docs/TROUBLESHOOTING.md`.

## Bezpečnost

- `JIRA_API_TOKEN` ukládej pouze do souboru s právy `0600`.
- `teams-auth.json` obsahuje dlouhodobý refresh token a musí mít `0600`.
- Tokeny nikdy necommituj.
- `.gitignore` záměrně ignoruje lokální env/token/state soubory.

## Upgrade

Po `git pull`:

```bash
./deploy.sh build
./deploy.sh install-user
```

Nebo jen:

```bash
./deploy.sh install-user
```

`install-user` vždy sestaví aktuální release binárku před instalací.

## Vývoj

Viz `docs/DEVELOPMENT.md`.

## Testy a validace

Před commitem nebo releasem spusť:

```bash
bash scripts/validate.sh
```

Skript kontroluje formátování, unit testy, `clippy` bez warningů, statický musl
build, Debian balíček a Git whitespace. Podrobnosti jsou v
`docs/TESTING.md`.

Bezpečnostní poznámky k Jira a Teams tokenům jsou v `SECURITY.md`.
