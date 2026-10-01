# Troubleshooting

## `Bearer token missing`

Starý přímý Teams Personal login přes username/password již není použitelný. Použij:

```bash
jira2teams --login
```

## `QuarantineURLsBlocked`

Teams Personal účet je v quarantine režimu a odmítá zprávy s URL. Jira2Teams posílá Jira notifikace bez URL. Pokud se chyba vrátí, zkontroluj, zda samotný Jira summary neobsahuje URL a zda běží aktuální build.

## `410 Gone` při endpoint registration

Aktuální Jira2Teams nepotřebuje endpoint registration pro push/Trouter. Používá regionální Teams chat endpoints vrácené při autentizaci.

## Nic se neposlalo

Porovnej Jira data se state:

```bash
bash scripts/diagnostics/gather.sh
```

End-to-end test:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

## Systemd

```bash
systemctl --user status jira2teams.service
journalctl --user -u jira2teams.service -n 100 --no-pager
```

## Token byl zneplatněn

Zopakuj:

```bash
systemctl --user stop jira2teams.service
jira2teams --login
systemctl --user start jira2teams.service
```
