#!/usr/bin/env bash
set -u

BIN="${JIRA_WATCH_BIN:-./target/x86_64-unknown-linux-musl/release/jira2teams}"
STATE_FILE="${STATE_FILE:-$HOME/.cache/jira2teams/state.json}"
JQL="${JQL:-assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC}"
MAX_RESULTS="${MAX_RESULTS:-50}"
SEND_TEST=0
[ "${1:-}" = "--send-test" ] && SEND_TEST=1

redact() {
  sed -E 's/(JIRA_API_TOKEN|TEAMS_PASSWORD|refresh_token|skypetoken)(["=: ]+)[^ ,}" ]+/\1\2<REDACTED>/g'
}

section() { printf '\n===== %s =====\n' "$1"; }

section "jira2teams gather"
printf 'date: '; date -Is 2>/dev/null || date
printf 'host: %s\n' "$(hostname)"
printf 'user: %s\n' "${USER:-$(id -un)}"
printf 'pwd: %s\n' "$PWD"
printf 'binary: %s\n' "$BIN"
printf 'state: %s\n' "$STATE_FILE"
printf 'JIRA_URL: %s\n' "${JIRA_URL:-<unset>}"
printf 'JIRA_EMAIL: %s\n' "${JIRA_EMAIL:-<unset>}"
printf 'JQL: %s\n' "$JQL"
printf 'MAX_RESULTS: %s\n' "$MAX_RESULTS"
printf 'TEAMS_THREAD_ID: %s\n' "${TEAMS_THREAD_ID:-<unset>}"
printf 'TEAMS_TO: %s\n' "${TEAMS_TO:-<unset>}"

section "binary"
if [ -x "$BIN" ]; then
  "$BIN" --version 2>&1 || true
  file "$BIN" 2>&1 || true
else
  echo "ERROR: binary not found/executable: $BIN"
fi

section "required tools for gather"
for c in curl jq; do
  if command -v "$c" >/dev/null 2>&1; then
    printf '%s: %s\n' "$c" "$(command -v "$c")"
  else
    printf '%s: MISSING\n' "$c"
  fi
done

section "state file"
if [ -f "$STATE_FILE" ]; then
  ls -l "$STATE_FILE" 2>&1 || true
  if command -v jq >/dev/null 2>&1; then
    jq -r 'to_entries[] | [.key,.value] | @tsv' "$STATE_FILE" 2>&1 | sort || true
  else
    cat "$STATE_FILE" 2>&1 || true
  fi
else
  echo "STATE FILE DOES NOT EXIST"
fi

if [ -z "${JIRA_URL:-}" ] || [ -z "${JIRA_EMAIL:-}" ] || [ -z "${JIRA_API_TOKEN:-}" ]; then
  section "jira api"
  echo "ERROR: JIRA_URL/JIRA_EMAIL/JIRA_API_TOKEN must already be exported"
  exit 2
fi
if ! command -v curl >/dev/null 2>&1 || ! command -v jq >/dev/null 2>&1; then
  section "jira api"
  echo "ERROR: gather needs curl and jq"
  exit 2
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
HTTP_CODE="$(curl -sS -o "$TMP" -w '%{http_code}' -G -u "${JIRA_EMAIL}:${JIRA_API_TOKEN}" -H 'Accept: application/json' "${JIRA_URL%/}/rest/api/3/search/jql" --data-urlencode "jql=${JQL}" --data-urlencode "maxResults=${MAX_RESULTS}" --data-urlencode 'fields=summary,status,updated' 2>&1)"

section "jira api response"
echo "HTTP: $HTTP_CODE"
if ! jq -e . "$TMP" >/dev/null 2>&1; then
  echo "Response is not JSON:"
  head -c 2000 "$TMP"; echo
  exit 3
fi
if ! jq -e '.issues | type == "array"' "$TMP" >/dev/null 2>&1; then
  echo "No .issues array. Response:"
  jq . "$TMP" | head -n 120
  exit 3
fi
COUNT="$(jq '.issues | length' "$TMP")"
echo "issues: $COUNT"

section "jira current vs saved state"
printf '%-14s %-22s %-32s %-32s %s\n' 'KEY' 'STATUS' 'CURRENT_UPDATED' 'SAVED_UPDATED' 'RESULT'
printf '%-14s %-22s %-32s %-32s %s\n' '--------------' '----------------------' '--------------------------------' '--------------------------------' '------'
jq -r '.issues[] | [.key,.fields.status.name,.fields.updated,.fields.summary] | @tsv' "$TMP" | while IFS=$'\t' read -r key status updated summary; do
  old=''
  if [ -f "$STATE_FILE" ]; then old="$(jq -r --arg k "$key" '.[$k] // empty' "$STATE_FILE" 2>/dev/null || true)"; fi
  if [ -z "$old" ]; then result='NEW/NOT IN STATE'; elif [ "$old" != "$updated" ]; then result='CHANGED'; else result='same'; fi
  printf '%-14s %-22.22s %-32.32s %-32.32s %s\n' "$key" "$status" "$updated" "${old:--}" "$result"
  printf '  summary: %s\n' "$summary"
done

section "saved tickets missing from current JQL"
if [ -f "$STATE_FILE" ]; then
  jq -r 'keys[]' "$STATE_FILE" 2>/dev/null | while IFS= read -r key; do
    if ! jq -e --arg k "$key" '.issues[]? | select(.key == $k)' "$TMP" >/dev/null; then
      echo "$key"
    fi
  done
else
  echo "(no state file)"
fi

section "what watcher would do now"
CHANGED="$(jq --argfile old "${STATE_FILE:-/dev/null}" -r '[.issues[] | select(($old[.key] // "") != .fields.updated)] | length' "$TMP" 2>/dev/null || true)"
if [ -n "$CHANGED" ]; then
  echo "issues different from saved state: $CHANGED"
else
  echo "could not calculate aggregate; see table above"
fi

echo "NOTE: if state file did not exist when jira2teams started, its first poll intentionally seeds state and sends nothing."
echo "NOTE: tickets that disappear from the JQL (for example after resolution or reassignment) cannot be notified by the current algorithm; they are listed above as missing from current JQL."

section "teams test"
if [ "$SEND_TEST" -eq 1 ]; then
  if [ -x "$BIN" ]; then
    "$BIN" --test-teams 2>&1 | redact || true
  else
    echo "SKIP: binary missing"
  fi
else
  echo "SKIP (run this gather with --send-test to send one test Teams message)"
fi

section "done"
