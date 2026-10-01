# Development

## Production target

The production target is:

```text
x86_64-unknown-linux-musl
```

`.cargo/config.toml` configures this target as the default, so:

```bash
cargo build --release
```

should produce a statically linked musl binary.

## Pre-commit validation

Run:

```bash
bash scripts/validate.sh
```

The underlying checks include:

```bash
cargo fmt --check
cargo test --release
cargo clippy --release -- -D warnings
./deploy.sh build
./deploy.sh package-deb
git diff --check
```

Warnings are treated as validation failures.

## Versioning

The application version is stored in `Cargo.toml`. The Debian package version is derived from it automatically.

Use semantic versioning for public releases.

## State compatibility

v0.4.0 accepts both:

- legacy v0.3.x `key -> updated` string values,
- rich v0.4.x state objects.

State migration happens in memory while loading and is persisted in the new format after a successful poll.

When changing the state schema:

1. preserve backward compatibility whenever practical,
2. add regression tests for migration,
3. never overwrite state after a failed notification send,
4. keep state writes atomic and mode `0600`.

## Jira pagination

The enhanced JQL endpoint uses `nextPageToken`. Jira2Teams must fetch all result pages before comparing current issues against persisted state.

Never treat a partially fetched result set as authoritative for missing-ticket detection.

## Git hygiene

The repository must not contain:

- Jira API tokens,
- Teams refresh or Skype tokens,
- runtime `state.json`,
- `target/`,
- locally generated `.deb` files,
- one-off development patches,
- temporary migration scripts.

## Teams transport

The current Teams Personal consumer transport is reverse engineered and unsupported. Keep it isolated from Jira/state logic so a supported transport can be added without rewriting the watcher.

See `docs/TEAMS-CONSUMER-RISK.md`.
