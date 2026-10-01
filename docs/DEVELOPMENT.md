# Development

## Build target

Produkční target je:

```text
x86_64-unknown-linux-musl
```

`.cargo/config.toml` nastavuje tento target jako výchozí, takže:

```bash
cargo build --release
```

má vytvořit statickou musl binárku.

## Kontroly před commitem

```bash
cargo fmt --check
cargo test --release
cargo clippy --release -- -D warnings
./deploy.sh build
git diff --check
```

Pokud `clippy` odhalí pouze již známé `unused_mut`, oprav je před stabilním releasem.

## Verze

Verze je v `Cargo.toml`. `.deb` ji přebírá automaticky.

## Git

Repozitář nesmí obsahovat:

- Jira API token,
- Teams refresh token,
- `state.json`,
- build adresář `target/`,
- lokální `.deb` v `dist/`,
- jednorázové patch soubory z vývoje.
