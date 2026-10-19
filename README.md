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

The workspace ships two binaries, so each install names its package:

```bash
cargo install --git https://github.com/dlorp/paperclips papercuts
cargo install --git https://github.com/dlorp/paperclips paperclip
```

## Minimum Supported Rust Version

**Rust 1.89**, enforced via `rust-version` in each crate's `Cargo.toml`.

## Quick start

```bash
# Log friction
papercuts add "thumbnail cache returned stale entries after a re-import" \
  --tag media --severity major --where photo-import/cache --agent dlorp

# Log a win on the same component
paperclip add "bulk hash check cut duplicate imports from minutes to seconds" \
  --tag media --impact huge --where photo-import/cache --agent dlorp

# Review both
paperclip review --format md
```

## Overlaps

`review` joins cuts and clips on the `--where` field (exact, then path-prefix, then tag) and renders a disposition worksheet:

```
Overlap shapes: read each pair and assign one:
  A. Same component, different aspect. Clip praises the part, cut names a sharp
     edge on it. Fix the edge, keep the part. The clip is a guardrail on the
     remediation: it tells you what NOT to remove.
  B. Value confirmed, access broken. Clip proves the asset's worth, cut says
     finding/using it is the problem. Amplify: index, rename, document.
     Promotion target is discoverability, not CLAUDE.md prose.
  C. Context-dependence. The same *behavior* is clipped by one agent/workflow
     and cut by another. Neither fix nor remove: add a mode or config flag.
     Rare; highest-information record in the file when it appears.

## Overlaps (1 exact, 1 prefix)

### photo-import/cache  [exact · 1 cut / 1 clip]
  CUT  [major  ] pc_73b361f523c9: thumbnail cache returned stale entries after a re-import
  CLIP [huge   ] cl_8d38d6a54561: bulk hash check cut duplicate imports from minutes to seconds
  Disposition: ____________________

### photo-import  [prefix · 1 cut / 1 clip]
  CUT  [minor  ] pc_efbf60ad7fa4: duplicate uploads when the folder watcher fired twice
  CLIP [solid  ] cl_fccf537c6bb9: resume-after-crash recovered an interrupted import
  Disposition: ____________________
```

## Resolve with verification

A resolution must carry evidence. `resolve` rejects a record that names a fix but never says how effectiveness was checked; a link without verification reproduces the exact failure it is meant to catch.

```bash
papercuts resolve pc_73b3 \
  --agent dlorp \
  --resolution "compare mtime and content hash before serving a cached thumbnail" \
  --resolved-by dlorp \
  --verification-method "re-imported a 400-photo batch twice and diffed the served thumbnails" \
  --verification-result "zero stale thumbnails across 400 files" \
  --linked-task null
```

```json
{"ok":true,"data":{"changed":true,"record":{"kind":"cut","id":"pc_73b361f523c9","ts":"2026-10-18T00:00:00.000Z","agent":"dlorp","text":"thumbnail cache returned stale entries after a re-import","tags":["media"],"severity":"major","cwd":".","repo":".","where":"photo-import/cache","status":"resolved","resolution":{"ts":"2026-10-18T00:00:00.000Z","agent":"dlorp","note":null}},"feedback":{"feedback_id":"pc_73b361f523c9","kind":"cut","disposition":"resolved","resolution":"compare mtime and content hash before serving a cached thumbnail","linked_task":null,"resolved_at":"2026-10-18T00:00:00.000Z","resolved_by":"dlorp","verification_method":"re-imported a 400-photo batch twice and diffed the served thumbnails","verification_result":"zero stale thumbnails across 400 files","verified_at":"2026-10-18T00:00:00.000Z","change_class":null,"changed_artifact":null,"channel":null,"task_id":null,"owner":null,"reason":null,"next_review":null,"recorded_at":"2026-10-18T00:00:00.000Z","updated_at":null}},"meta":{"contract":1,"file":"/private/tmp/paperclips-demo/.papercuts.jsonl","agent_source":"flag"}}
```

`wontfix` is the other terminal state. It requires a reason and leaves the source log untouched:

```bash
papercuts wontfix pc_efbf \
  --reason "the watcher double-fire is an upstream inotify behavior; debounce is documented" \
  --owner dlorp
```

```json
{"ok":true,"data":{"changed":true,"feedback":{"feedback_id":"pc_efbf60ad7fa4","kind":"cut","disposition":"wontfix","resolution":"wontfix: the watcher double-fire is an upstream inotify behavior; debounce is documented","linked_task":null,"resolved_at":"2026-10-18T00:00:00.000Z","resolved_by":"dlorp","verification_method":null,"verification_result":null,"verified_at":"2026-10-18T00:00:00.000Z","change_class":null,"changed_artifact":null,"channel":null,"task_id":null,"owner":"dlorp","reason":"the watcher double-fire is an upstream inotify behavior; debounce is documented","next_review":null,"recorded_at":"2026-10-18T00:00:00.000Z","updated_at":null}},"meta":{"contract":1,"file":"/private/tmp/paperclips-demo/.feedback-resolutions.jsonl"}}
```

Resolutions and wontfix decisions append to `.feedback-resolutions.jsonl` (override with `FEEDBACK_RESOLUTIONS_FILE`). The latest record for an item wins, so history is preserved without editing either log.

## Open work

`--unresolved` drops anything with a terminal disposition. `--digest` summarizes what is left:

```bash
papercuts list --digest --format md
```

```
open cuts by severity: minor=0 major=2 blocker=0 unset=0 (total open=2)
```

```bash
papercuts list --unresolved --format md
```

```
## Major
- [pc_00ae84a69635] launchd job hung: zsh has no timeout wrapper, so the retry ran unbounded: dlorp, 2026-10-18T00:00:00.000Z (shell)
- [pc_0e38be77d1c4] zsh has no timeout, so the retry ran unbounded: dlorp, 2026-10-18T00:00:00.000Z (shell)
```

## Promote

```bash
paperclip promote cl_8d38 --agent dlorp
```

```json
{"ok":true,"data":{"changed":true,"id":"cl_8d38d6a54561","status":"promoted"},"meta":{"contract":1}}
```

```bash
paperclip note cl_8d38 "adopted in the import guide" --agent dlorp
```

```json
{"ok":true,"data":{"changed":true,"id":"cl_8d38d6a54561","notes":1},"meta":{"contract":1}}
```

## Recurrence

When a new cut is the same failure as an existing one (identical normalized text, or the same known failure class), it carries `recurrence_of` pointing at the earliest matching cut. The first cut here establishes the failure class:

```bash
papercuts add "launchd job hung: zsh has no timeout wrapper, so the retry ran unbounded" \
  --agent dlorp --tag shell --severity major
```

```json
{"ok":true,"data":{"changed":true,"record":{"kind":"cut","id":"pc_00ae84a69635","ts":"2026-10-18T00:00:00.000Z","agent":"dlorp","text":"launchd job hung: zsh has no timeout wrapper, so the retry ran unbounded","tags":["shell"],"severity":"major","cwd":".","repo":"."}},"meta":{"contract":1,"file":"/private/tmp/paperclips-demo/.papercuts.jsonl","agent_source":"flag"}}
```

The next cut says the same thing in different words, so the identical-text check misses it and the failure-class signature catches it:

```bash
papercuts add "zsh has no timeout, so the retry ran unbounded" \
  --agent dlorp --tag shell --severity major
```

```json
{"ok":true,"data":{"changed":true,"record":{"kind":"cut","id":"pc_0e38be77d1c4","ts":"2026-10-18T00:00:00.000Z","agent":"dlorp","text":"zsh has no timeout, so the retry ran unbounded","tags":["shell"],"severity":"major","cwd":".","repo":".","recurrence_of":"pc_00ae84a69635"}},"meta":{"contract":1,"file":"/private/tmp/paperclips-demo/.papercuts.jsonl","agent_source":"flag"}}
```

Detection happens at write time. Historic records are never rewritten.

## Review output format

`--format json` (default): machine-readable join records for downstream processing.

`--format md`: Markdown with overlap shapes ready for human disposition, as shown under [Overlaps](#overlaps).

## Schema

```bash
papercuts schema          # every command, flag, record shape, exit code
papercuts schema record   # JSONL record shapes only
paperclip schema          # clips contract
```

## Feature parity

See [docs/parity.md](docs/parity.md) for the verb matrix against the reference Python logger, including record compatibility, CAPA fields, recurrence detection, and the feedback-resolutions join.

## Reproducing the examples

Every command whose output is shown above comes from one run against a scratch git repository at `/tmp/paperclips-demo`, with `PAPERCUTS_NOW=2026-10-18T00:00:00Z`. Record IDs are the first six bytes of a SHA-256 over the record fields, so pinning the clock is what makes them reproducible.

The blocks are pasted verbatim. Each one is the complete envelope the binary printed, `{"ok":true,"data":{...},"meta":{...}}`, with no fields removed. The `meta.file` paths are the scratch repository's own log files.

The JSONL logs and the dispositions ledger are ignored by default; they are working data, not part of the repository.

## Attribution

Forked from [treygoff24/papercuts](https://github.com/treygoff24/papercuts) (MIT). The `papercuts` binary remains behavior-compatible with upstream.

## License

MIT
