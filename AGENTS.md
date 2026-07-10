# AGENTS.md — paperclips

Machine-facing contract for agents working in this repo.

## What this is

`paperclips` is a Rust workspace giving AI agents a two-directional feedback system:
- **papercuts** — log friction (what to fix)
- **paperclip** — log wins (what to keep)

Forked from [treygoff24/papercuts](https://github.com/treygoff24/papercuts) (MIT).

## Build and gate

```bash
cargo build --workspace
cargo test -p papercuts --test cli
```

## Key concepts

- **Cuts** (papercuts): friction. `.papercuts.jsonl`
- **Clips** (paperclip): wins. `.paperclips.jsonl`
- **Overlaps**: same `--where` in both logs = load-bearing AND sharp-edged
- **`--where`**: component or path the record is about. The join key.

## Invariants

- Append-only. stdout = data only. stderr = errors only.
- Old logs without `where` still work.
