#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
TARGET="x86_64-unknown-linux-musl"
BIN="$ROOT/target/$TARGET/release/jira2teams"
USER_BIN="$HOME/.local/bin/jira2teams"
CONFIG_DIR="$HOME/.config/jira2teams"
CACHE_DIR="$HOME/.cache/jira2teams"
ENV_FILE="$CONFIG_DIR/jira2teams.env"
AUTH_FILE="$CONFIG_DIR/teams-auth.json"
STATE_FILE="$CACHE_DIR/state.json"
USER_UNIT_DIR="$HOME/.config/systemd/user"
USER_UNIT="$USER_UNIT_DIR/jira2teams.service"

version() {
  sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1
}

env_file_value() {
  local key="$1" file="$2" value
  [ -f "$file" ] || return 0
  value="$(sed -n "s/^[[:space:]]*${key}[[:space:]]*=[[:space:]]*//p" "$file" | tail -n 1)"
  value="${value%$'\r'}"
  case "$value" in
    \"*\")
      value="${value#\"}"
      value="${value%\"}"
      ;;
    \'*\')
      value="${value#\'}"
      value="${value%\'}"
      ;;
  esac
  printf '%s' "$value"
}

build() {
  rustup target add "$TARGET" >/dev/null
  cargo build --manifest-path "$ROOT/Cargo.toml" --release
  file "$BIN"
  ldd "$BIN" 2>&1 || true
}

migrate_runtime_state() {
  install -d -m 700 "$CONFIG_DIR" "$CACHE_DIR"
  if [ ! -f "$AUTH_FILE" ] && [ -f "$HOME/.config/jira-watch/teams-auth.json" ]; then
    install -m 600 "$HOME/.config/jira-watch/teams-auth.json" "$AUTH_FILE"
    echo "Migrovan Teams auth: $AUTH_FILE"
  fi
  if [ ! -f "$STATE_FILE" ] && [ -f "$HOME/.cache/jira-watch/state.json" ]; then
    install -m 600 "$HOME/.cache/jira-watch/state.json" "$STATE_FILE"
    echo "Migrovan Jira state: $STATE_FILE"
  fi
  if [ ! -f "$ENV_FILE" ] && [ -f "$HOME/.config/jira-watch/jira-watch.env" ]; then
    install -m 600 "$HOME/.config/jira-watch/jira-watch.env" "$ENV_FILE"
    echo "Migrovan config: $ENV_FILE"
  fi
}

install_user() {
  build
  migrate_runtime_state
  install -d -m 755 "$HOME/.local/bin" "$USER_UNIT_DIR"
  install -m 755 "$BIN" "$USER_BIN"
  install -m 644 "$ROOT/deploy/systemd/jira2teams.service" "$USER_UNIT"

  if [ ! -f "$ENV_FILE" ]; then
    install -m 600 "$ROOT/jira2teams.env.example" "$ENV_FILE"
    echo
    echo "Vytvoren config template: $ENV_FILE"
    echo "Dopln Jira konfiguraci a vyber Teams transport."
    echo "Webhook: nastav TEAMS_TRANSPORT=webhook a TEAMS_WEBHOOK_URL."
    echo "Consumer: nastav TEAMS_TRANSPORT=consumer a TEAMS_THREAD_ID, potom spust $USER_BIN --login."
    echo "Nakonec: systemctl --user enable --now jira2teams.service"
    systemctl --user daemon-reload
    exit 2
  fi

  local teams_transport webhook_url
  teams_transport="$(env_file_value TEAMS_TRANSPORT "$ENV_FILE")"
  webhook_url="$(env_file_value TEAMS_WEBHOOK_URL "$ENV_FILE")"

  if [ "$teams_transport" = "webhook" ] || { [ -z "$teams_transport" ] && [ -n "$webhook_url" ]; }; then
    echo "Teams transport: webhook"
    if [ -z "$webhook_url" ]; then
      echo "Chybi TEAMS_WEBHOOK_URL v $ENV_FILE" >&2
      exit 2
    fi
  elif [ -z "$teams_transport" ] || [ "$teams_transport" = "consumer" ]; then
    echo "Teams transport: consumer"
    if [ ! -f "$AUTH_FILE" ]; then
      echo
      echo "Chybi Teams auth cache: $AUTH_FILE"
      echo "Spust jednou: $USER_BIN --login"
      echo "Pak: systemctl --user enable --now jira2teams.service"
      systemctl --user daemon-reload
      exit 2
    fi
  else
    echo "Neplatny TEAMS_TRANSPORT=$teams_transport v $ENV_FILE" >&2
    exit 2
  fi

  systemctl --user daemon-reload
  systemctl --user enable --now jira2teams.service
  systemctl --user --no-pager --full status jira2teams.service || true
}

restart_user() {
  systemctl --user daemon-reload
  systemctl --user restart jira2teams.service
  systemctl --user --no-pager --full status jira2teams.service
}

uninstall_user() {
  systemctl --user disable --now jira2teams.service 2>/dev/null || true
  rm -f "$USER_UNIT" "$USER_BIN"
  systemctl --user daemon-reload
  echo "Konfigurace a tokeny v $CONFIG_DIR byly ponechany."
  echo "State v $CACHE_DIR byl ponechan."
}

package_deb() {
  build
  command -v dpkg-deb >/dev/null 2>&1 || { echo "Chybi dpkg-deb (balik dpkg-dev)." >&2; exit 1; }

  local ver arch stage out
  ver="$(version)"
  arch="amd64"
  stage="$ROOT/dist/deb-root"
  out="$ROOT/dist/jira2teams_${ver}_${arch}.deb"

  rm -rf "$stage"
  install -d "$stage/DEBIAN" "$stage/usr/bin" "$stage/usr/lib/systemd/user" "$stage/usr/share/doc/jira2teams/examples" "$stage/usr/share/doc/jira2teams/docs" "$stage/usr/share/doc/jira2teams"
  install -m 755 "$BIN" "$stage/usr/bin/jira2teams"
  install -m 644 "$ROOT/debian/jira2teams.service" "$stage/usr/lib/systemd/user/jira2teams.service"
  install -m 644 "$ROOT/jira2teams.env.example" "$stage/usr/share/doc/jira2teams/examples/jira2teams.env.example"
  install -m 644 "$ROOT/README.md" "$stage/usr/share/doc/jira2teams/README.md"
  install -m 644 "$ROOT/SECURITY.md" "$stage/usr/share/doc/jira2teams/SECURITY.md"
  install -m 644 "$ROOT/CHANGELOG.md" "$stage/usr/share/doc/jira2teams/CHANGELOG.md"
  install -m 644 "$ROOT/debian/README.Debian" "$stage/usr/share/doc/jira2teams/README.Debian"
  for doc in "$ROOT"/docs/*.md; do
    [ -f "$doc" ] || continue
    install -m 644 "$doc" "$stage/usr/share/doc/jira2teams/docs/$(basename "$doc")"
  done

  cat > "$stage/DEBIAN/control" <<CONTROL
Package: jira2teams
Version: $ver
Section: net
Priority: optional
Architecture: $arch
Maintainer: ${DEB_MAINTAINER:-Jira2Teams Project <jira2teams@localhost>}
Description: Jira Cloud watcher sending changes to Microsoft Teams
 Static musl Rust binary. Uses Jira REST API and supports Microsoft Teams
 Workflows webhooks plus a legacy Teams Personal consumer transport.
CONTROL

  dpkg-deb --root-owner-group --build "$stage" "$out"
  dpkg-deb -I "$out"
  echo
  echo "Debian 13 package: $out"
}

case "${1:-help}" in
  build)
    build
    ;;
  install-user)
    install_user
    ;;
  restart-user)
    restart_user
    ;;
  uninstall-user)
    uninstall_user
    ;;
  package-deb)
    package_deb
    ;;
  all)
    build
    package_deb
    ;;
  *)
    cat <<USAGE
Pouziti:
  ./deploy.sh build
  ./deploy.sh install-user
  ./deploy.sh restart-user
  ./deploy.sh uninstall-user
  ./deploy.sh package-deb
  ./deploy.sh all
USAGE
    ;;
esac
