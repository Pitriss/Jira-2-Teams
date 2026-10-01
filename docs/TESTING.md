# Testing and validation

Before a commit intended for deployment or release, run:

```bash
bash scripts/validate.sh
```

The validation performs:

1. `cargo fmt --check`
2. release unit tests
3. `cargo clippy --release -- -D warnings`
4. static `x86_64-unknown-linux-musl` release build
5. Debian `amd64` package build
6. inspection of the generated `.deb`
7. `git diff --check`

## Unit tests

The initial regression tests cover behavior that already caused real integration
problems during development:

- Microsoft device-code responses where `expires_in` and `interval` arrive as
  either JSON numbers or numeric strings,
- the `verification_uri` compatibility alias,
- stripping URLs from Teams notifications for accounts in quarantine mode,
- HTML escaping,
- extraction of nested Teams/Skype tokens,
- character-safe truncation of diagnostics.

## Integration diagnostics

The scripts below use real configured services and therefore are not part of
the automatic unit test suite:

```bash
bash scripts/diagnostics/gather.sh
```

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

Never use production credentials in committed fixtures or test data.
