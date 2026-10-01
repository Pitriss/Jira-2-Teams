#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

rustup component add rustfmt clippy >/dev/null
rustup target add x86_64-unknown-linux-musl >/dev/null

echo "== cargo fmt =="
cargo fmt --check

echo "== cargo test =="
cargo test --release

echo "== cargo clippy =="
cargo clippy --release -- -D warnings

echo "== static musl build =="
./deploy.sh build

echo "== Debian package =="
./deploy.sh package-deb

DEB="$(find dist -maxdepth 1 -type f -name 'jira2teams_*_amd64.deb' -print -quit)"
[ -n "$DEB" ] || { echo "Debian package nebyl vytvoren." >&2; exit 1; }

echo "== Debian package contents =="
dpkg-deb -c "$DEB"

echo "== Git whitespace =="
git diff --check

echo "== Git status =="
git status --short

echo "Validation OK"
