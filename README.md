# paperclips

Two-directional friction log for AI agents: record what breaks, record what works, then review both together.

## Why

AI agents generate a lot of signal during work — dead-end tool calls, broken links, footgun configs on one side; helpers that saved time, docs that answered exactly right on the other. Most friction-logging tools only capture the negative. `paperclips` captures both directions and joins them so you can see where something is simultaneously load-bearing and sharp-edged.

- **papercuts** — log friction (what to fix) — `.papercuts.jsonl`
- **paperclip** — log wins (what to keep) — `.paperclips.jsonl`

When both logs share a `--where` on the same component, `paperclip review` surfaces it as an **Overlap**: the highest-information record in the system.

## Install

```bash
cargo install --git https://github.com/dlorp/paperclips --bins
```

## Quick start

```bash
# Log friction
papercuts add "cache invalidation races on concurrent writes" \
  --tag storage --severity major --where kv-store

# Log wins
paperclip add "bulk insert API cut sync time from minutes to seconds" \
  --tag storage --impact huge --where kv-store

# Review both
paperclip review --format md
```

## Overlaps

`review` joins cuts × clips on the `--where` field (exact > prefix > tag) and renders:

```
Overlap shapes — read each pair and assign one:
  A. Same component, different aspect — fix the edge, keep the value
  B. Value confirmed, access broken — amplify discoverability
  C. Context-dependence — add a mode or config flag

### bridge-trigger  [exact · 1 cut / 1 clip]
  CUT  [major] pc_a1b2 — fires without priority check
  CLIP [solid] cl_f6e5 — auto-dispatches useful work, cleared the backlog unattended
  → Shape A: the trigger works (cleared backlog), but the edge case is sharp (no priority check)
  Disposition: add a gate before dispatch ___________
```

## Review output format

`--format md` (default): Markdown with overlap shapes ready for human disposition.

`--format json`: Machine-readable join records for downstream processing.

## Dogfooding

This repo includes `.paperclips.jsonl` with 3 entries (2 clips, 1 cut) demonstrating the format in production — including the canonical `bridge-trigger` Shape A overlap. Read it to see how the `note` field carries context that a single `what` line cannot capture.

## Attribution

Forked from [treygoff24/papercuts](https://github.com/treygoff24/papercuts) (MIT). The `papercuts` binary is behavior-compatible with upstream.

## License

MIT
