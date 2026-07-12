# paperclips

Two-directional friction log for AI agents: record what breaks, record what works, then review both together.

## Why

AI agents generate a lot of signal during work — dead-end tool calls, broken links, footgun configs on one side; helpers that saved time, docs that answered exactly right on the other. Most friction-logging tools only capture the negative. `paperclips` captures both directions and joins them so you can see where something is simultaneously load-bearing and sharp-edged.

- **papercuts** — log friction (what to fix)
- **paperclip** — log wins (what to keep)

When both logs share a `--where` on the same component, `paperclip review` surfaces it as an **Overlap**: the highest-information record in the system.

## Install

```bash
cargo install --git https://github.com/dlorp/paperclips --bins
```

## Quick start

```bash
# Log friction
papercuts add "auth flow silently swallows expired tokens" \
  --tag auth --severity major --where bridge-trigger

# Log wins
paperclip add "vault search found exactly the right entry on first try" \
  --tag search --impact solid --where vault-query

# Review both
paperclip review --format md
```

## Overlaps

`review` joins cuts × clips on the `--where` field (exact > prefix > tag) and renders:

```
Overlap shapes — read each pair and assign one:
  A. Same component, different aspect — fix the edge, keep the part
  B. Value confirmed, access broken — amplify discoverability
  C. Context-dependence — add a mode or config flag

### bridge-trigger  [exact · 1 cut / 1 clip]
  CUT  [major] pc_a1b2 — auth flow silently swallows expired tokens
  CLIP [solid] cl_f6e5 — trigger fired correctly on first deploy
  Disposition: ____________________
```

## Attribution

Forked from [treygoff24/papercuts](https://github.com/treygoff24/papercuts) (MIT). The `papercuts` binary is behavior-compatible with upstream.

## License

MIT
