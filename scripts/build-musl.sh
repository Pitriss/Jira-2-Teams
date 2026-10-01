#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

rustup target add x86_64-unknown-linux-musl >/dev/null
cargo build --release

BIN="$ROOT/target/x86_64-unknown-linux-musl/release/jira2teams"
[ -x "$BIN" ] || { echo "Chybi binarka: $BIN" >&2; exit 1; }

file "$BIN"
ldd "$BIN" 2>&1 || true
