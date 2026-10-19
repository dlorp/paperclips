# paperclips

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange)](Cargo.toml)
[![Tests](https://img.shields.io/badge/tests-51%20passing-brightgreen)]()

Two-directional friction log for AI agents: record what breaks, record what works, then review both together.

## Why

AI agents generate a lot of signal during work: dead-end tool calls, broken links, and footgun configs on one side; helpers that saved time and docs that answered exactly right on the other. Most friction-logging tools only capture the negative. `paperclips` captures both directions and joins them, so you can see where something is simultaneously load-bearing and sharp-edged.

- **papercuts** logs friction (what to fix) in `.papercuts.jsonl`
- **paperclip** logs wins (what to keep) in `.paperclips.jsonl`

## Lifecycle

The lifecycle inverts, not just the sentiment.

**Cuts** end in `resolve` (the problem is gone) or `wontfix` (a deliberate no-action decision). The goal is elimination.

**Clips** end in `promote`: the win is written into project docs or a skill so future sessions inherit it. The goal is compounding. A clip that stays in the log is a single observation; a promoted clip is a permanent capability upgrade.

```
Cuts:   add -> [someone fixes it] -> resolve, or -> wontfix
Clips:  add -> [someone codifies it] -> promote
```

When both logs share a `--where` on the same component, `paperclip review` surfaces it as an **Overlap**: the highest-information record in the system.

## Install

```bash
cargo install --git https://github.com/dlorp/paperclips --bins
```

## Minimum Supported Rust Version

**Rust 1.89**, enforced via `rust-version` in each crate's `Cargo.toml`.

## Quick start

```bash
# Log friction
papercuts add "thumbnail cache returned stale entries after a re-import" \
  --tag media --severity major --where photo-import/cache

# Log a win on the same component
paperclip add "bulk hash check cut duplicate imports from minutes to seconds" \
  --tag media --impact huge --where photo-import/cache

# Review both
paperclip review --format md
```

## Overlaps

`review` joins cuts and clips on the `--where` field (exact, then path-prefix, then tag) and renders a disposition worksheet:

```
Overlap shapes — read each pair and assign one:
  A. Same component, different aspect — clip praises the part, cut names a sharp
     edge on it. → Fix the edge, keep the part. The clip is a guardrail on the
     remediation: it tells you what NOT to remove.
  B. Value confirmed, access broken — clip proves the asset's worth, cut says
     finding/using it is the problem. → Amplify: index, rename, document.
     Promotion target is discoverability, not CLAUDE.md prose.
  C. Context-dependence — the same *behavior* is clipped by one agent/workflow
     and cut by another. → Neither fix nor remove: add a mode or config flag.
     Rare; highest-information record in the file when it appears.

## Overlaps (1 exact, 1 prefix)

### photo-import/cache  [exact · 1 cut / 1 clip]
  CUT  [major  ] pc_3cb053bf67bb — thumbnail cache returned stale entries after a re-import
  CLIP [huge   ] cl_d35cf5a35935 — bulk hash check cut duplicate imports from minutes to seconds
  Disposition: ____________________

### photo-import  [prefix · 1 cut / 1 clip]
  CUT  [minor  ] pc_2153242e6e17 — duplicate uploads when the folder watcher fired twice
  CLIP [solid  ] cl_e70fa4d2b96c — resume-after-crash recovered an interrupted import
  Disposition: ____________________
```

## Resolve with verification

A resolution must carry evidence. `resolve` rejects a record that names a fix but never says how effectiveness was checked; a link without verification reproduces the exact failure it is meant to catch.

```bash
papercuts resolve pc_3cb0 \
  --agent dlorp \
  --resolution "compare mtime and content hash before serving a cached thumbnail" \
  --resolved-by dlorp \
  --verification-method "re-imported a 400-photo batch twice and diffed the served thumbnails" \
  --verification-result "zero stale thumbnails across 400 files" \
  --linked-task null
```

```json
{"disposition":"resolved","resolution":"compare mtime and content hash before serving a cached thumbnail","verification_method":"re-imported a 400-photo batch twice and diffed the served thumbnails","verification_result":"zero stale thumbnails across 400 files"}
```

`wontfix` is the other terminal state. It requires a reason and leaves the source log untouched:

```bash
papercuts wontfix pc_2153 \
  --reason "the watcher double-fire is an upstream inotify behavior; debounce is documented" \
  --owner dlorp
```

```json
{"disposition":"wontfix","reason":"the watcher double-fire is an upstream inotify behavior; debounce is documented"}
```

Resolutions and wontfix decisions append to `.feedback-resolutions.jsonl` (override with `FEEDBACK_RESOLUTIONS_FILE`). The latest record for an item wins, so history is preserved without editing either log. `--unresolved` filters out anything with a terminal disposition, and `--digest` summarizes what is left:

```
open cuts by severity: minor=2 major=2 blocker=0 unset=0 (total open=4)
```

## Promote

```bash
paperclip promote cl_d35c --agent dlorp
```

```json
{"changed":true,"id":"cl_d35cf5a35935","status":"promoted"}
```

```bash
paperclip note cl_d35c "adopted in the import guide" --agent dlorp
```

```json
{"changed":true,"id":"cl_d35cf5a35935","notes":1}
```

## Recurrence

When a new cut is the same failure as an existing one (identical normalized text, or the same known failure class), it carries `recurrence_of` pointing at the earliest matching cut:

```bash
papercuts add "zsh has no timeout, so the retry ran unbounded" --agent dlorp --tag shell --severity major
```

```json
{"id":"pc_1e4a56ada4ed","recurrence_of":"pc_a3921653a77b"}
```

Detection happens at write time. Historic records are never rewritten.

## Review output format

`--format md` (default): Markdown with overlap shapes ready for human disposition.

`--format json`: Machine-readable join records for downstream processing.

## Schema

```bash
papercuts schema          # every command, flag, record shape, exit code
papercuts schema record   # JSONL record shapes only
paperclip schema          # clips contract
```

## Feature parity

See [docs/parity.md](docs/parity.md) for the full feature matrix against the reference Python logger, including record compatibility, CAPA fields, recurrence detection, and the feedback-resolutions join.

## Reproducing the examples

Every command and output block above was run against a scratch directory with `PAPERCUTS_NOW` pinned for stable timestamps. The JSONL logs are ignored by default; they are working data, not part of the repository.

## Attribution

Forked from [treygoff24/papercuts](https://github.com/treygoff24/papercuts) (MIT). The `papercuts` binary remains behavior-compatible with upstream.

## License

MIT
