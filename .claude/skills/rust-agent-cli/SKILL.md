# rust-agent-cli

Build, test, and lint the paperclips Rust CLI workspace.

## Build

```bash
cargo build --workspace
```

## Test

```bash
# Integration tests (primary gate)
cargo test -p papercuts --test cli

# All workspace tests
cargo test --workspace
```

## Lint

Rustfmt only — no clippy required. Run `cargo fmt` before commits.

## Structure

- `paper-core` — shared library (store, CLI parsing, error types, secrets scanning)
- `papercuts` — binary: log friction (`.papercuts.jsonl`)
- `paperclip` — binary: log wins (`.paperclips.jsonl`)

## MSRV

Rust 1.89 — enforced via `rust-version` in each crate's `Cargo.toml`.

## Pitfalls

- Edition 2024 let-chains are used in `store.rs` and `doctor.rs` — rustfmt on older toolchains may report false errors.
- The `regex` crate is used for secret scanning in `paper-core/src/secrets.rs`.
- `--force` flag on `add` bypasses the secret scan.
- `doctor --scan` retroactively audits existing log files for secrets.
