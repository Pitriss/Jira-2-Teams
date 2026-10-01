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

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
DEB="dist/jira2teams_${VERSION}_amd64.deb"
[ -f "$DEB" ] || { echo "Debian package $DEB nebyl vytvoren." >&2; exit 1; }

PKG="$(dpkg-deb -f "$DEB" Package)"
VER="$(dpkg-deb -f "$DEB" Version)"
ARCH="$(dpkg-deb -f "$DEB" Architecture)"
[ "$PKG" = "jira2teams" ] || { echo "Neocekavany package: $PKG" >&2; exit 1; }
[ "$VER" = "$VERSION" ] || { echo "Neocekavana verze balicku: $VER (ocekavano $VERSION)" >&2; exit 1; }
[ "$ARCH" = "amd64" ] || { echo "Neocekavana architektura: $ARCH" >&2; exit 1; }

echo "== Debian package contents =="
dpkg-deb -c "$DEB"

echo "== Debian package source parity =="
PKG_ROOT="$(mktemp -d)"
trap 'rm -rf "$PKG_ROOT"' EXIT
dpkg-deb -x "$DEB" "$PKG_ROOT"

cmp -s README.md "$PKG_ROOT/usr/share/doc/jira2teams/README.md" || { echo "README.md v balicku neodpovida zdroji." >&2; exit 1; }
cmp -s SECURITY.md "$PKG_ROOT/usr/share/doc/jira2teams/SECURITY.md" || { echo "SECURITY.md v balicku neodpovida zdroji." >&2; exit 1; }
cmp -s CHANGELOG.md "$PKG_ROOT/usr/share/doc/jira2teams/CHANGELOG.md" || { echo "CHANGELOG.md v balicku neodpovida zdroji." >&2; exit 1; }
cmp -s debian/README.Debian "$PKG_ROOT/usr/share/doc/jira2teams/README.Debian" || { echo "README.Debian v balicku neodpovida zdroji." >&2; exit 1; }
cmp -s jira2teams.env.example "$PKG_ROOT/usr/share/doc/jira2teams/examples/jira2teams.env.example" || { echo "jira2teams.env.example v balicku neodpovida zdroji." >&2; exit 1; }

for doc in docs/*.md; do
  name="$(basename "$doc")"
  cmp -s "$doc" "$PKG_ROOT/usr/share/doc/jira2teams/docs/$name" || { echo "$doc v balicku neodpovida zdroji." >&2; exit 1; }
done
echo "Debian package source parity: OK"

echo "== Git whitespace =="
git diff --check

echo "== Git status =="
git status --short

echo "Validation OK"
