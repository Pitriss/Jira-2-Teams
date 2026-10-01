#!/usr/bin/env bash
set -u

BIN="${BIN:-./target/x86_64-unknown-linux-musl/release/jira2teams}"
STATE_FILE="${STATE_FILE:-$HOME/.cache/jira2teams/state.json}"
KEY="${KEY:-K2HW-7127}"

if [ ! -x "$BIN" ]; then
  echo "ERROR: binarka neexistuje nebo neni spustitelna: $BIN" >&2
  exit 1
fi
if [ -z "${TEAMS_THREAD_ID:-}" ]; then
  echo "ERROR: nastav TEAMS_THREAD_ID" >&2
  exit 1
fi
if [ ! -f "$STATE_FILE" ]; then
  echo "ERROR: state neexistuje: $STATE_FILE" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: chybi jq" >&2
  exit 1
fi

BACKUP="${STATE_FILE}.before-e2e-test"
cp -p "$STATE_FILE" "$BACKUP"

echo "===== 1. Teams direct test ====="
"$BIN" --test-teams
TEAMS_RC=$?
echo "teams_test_rc=$TEAMS_RC"
echo

echo "===== 2. State before forced change ====="
jq -r --arg k "$KEY" '[$k, (.[$k] // "<missing>")] | @tsv' "$STATE_FILE"

echo "===== 3. Force old timestamp ====="
TMP="${STATE_FILE}.e2e.$$"
jq --arg k "$KEY" '.[$k]="1970-01-01T00:00:00.000+0000"' "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
jq -r --arg k "$KEY" '[$k, (.[$k] // "<missing>")] | @tsv' "$STATE_FILE"
echo

echo "===== 4. Jira -> Teams --once ====="
"$BIN" --once
ONCE_RC=$?
echo "once_rc=$ONCE_RC"
echo

echo "===== 5. State after --once ====="
jq -r --arg k "$KEY" '[$k, (.[$k] // "<missing>")] | @tsv' "$STATE_FILE"
echo
echo "backup=$BACKUP"

if [ "$TEAMS_RC" -eq 0 ] && [ "$ONCE_RC" -eq 0 ]; then
  echo "RESULT: oba kroky vratily rc=0. Pokud zpravy nejsou v Teams, chyba je v Teams doruceni/zobrazeni, ne v Jira detekci."
elif [ "$TEAMS_RC" -ne 0 ]; then
  echo "RESULT: selhal uz primy Teams test; Jira ted neresime."
else
  echo "RESULT: Teams direct test prosel, ale Jira -> Teams selhal; vystup vyse ukazuje konkretni misto."
fi
