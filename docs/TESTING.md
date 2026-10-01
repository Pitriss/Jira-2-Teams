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
6. inspection of generated `.deb` contents
7. `git diff --check`

## Unit tests

Regression tests cover behavior that has caused real integration failures or carries compatibility risk:

- Microsoft device-code timing fields as JSON numbers or numeric strings,
- the `verification_uri` compatibility alias,
- URL stripping for Teams quarantine mode,
- HTML escaping,
- nested Teams/Skype token extraction,
- character-safe diagnostic truncation,
- migration of the v0.3.x Jira state format,
- rich-state round trips,
- resolved-ticket classification,
- reassignment classification.

## Integration diagnostics

These scripts use real configured services and are therefore not part of the automatic unit test suite.

Gather:

```bash
bash scripts/diagnostics/gather.sh
```

End-to-end Jira-to-Teams test:

```bash
TEAMS_THREAD_ID='19:...@thread.v2' bash scripts/diagnostics/e2e-test.sh
```

Never put production credentials into committed fixtures or test data.
