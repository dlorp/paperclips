# paperclips

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange)](Cargo.toml)
[![Tests](https://img.shields.io/badge/tests-51%20passing-brightgreen)]()

Two-directional friction log for AI agents: record what breaks, record what works, then review both together.

## Why

AI agents generate a lot of signal during work: dead-end tool calls, broken links, and footgun configs on one side; helpers that saved time and docs that answered exactly right on the other. Most friction-logging tools only capture the negative. `paperclips` captures both directions and joins them, so you can see where something is simultaneously load-bearing and sharp-edged.

- **papercuts** logs friction (what to fix) in `.papercuts.jsonl`
- **paperclip** logs wins (what to keep) in `.paperclips.jsonl`

Each record carries a `--where` field naming the component or path it is about. That field is the join key between the two logs.

## Lifecycle

The lifecycle inverts, not just the sentiment. Five behaviours carry the whole model:

| Behaviour | What it does |
|-----------|--------------|
| `resolve` | Closes the cut. Requires evidence: the fix, who made it, how effectiveness was checked, and what the check found. |
| `wontfix` | Records a deliberate no-action decision with a reason. Terminal for counting purposes; the source log is untouched. |
| Recurrence anchoring | A new cut that repeats an earlier failure carries `recurrence_of` pointing at the earliest matching cut. History is never rewritten. |
| Reopen on refailure | Refiling a failure after its resolution. The new record is open, and `recurrence_of` links it to the resolved cut, so a fix that did not hold stays visible. |
| Evidence-recorded resolution | The verification method and result are stored beside the fix. A link without verification reproduces the exact failure it is meant to catch. |

**Cuts** end in `resolve` or `wontfix`. The goal is elimination.

**Clips** end in `promote`: the win is written into project docs or a skill so future sessions inherit it. The goal is compounding. A clip that stays in the log is a single observation; a promoted clip is a permanent capability upgrade.

```
Cuts:   add -> [someone fixes it] -> resolve, or -> wontfix
Clips:  add -> [someone codifies it] -> promote
```

When both logs share a `--where` on the same component, `paperclip review` surfaces it as an **Overlap**: the highest-information record in the system.

## Give your agents the pen

A friction log only works if it gets written to while the context is still hot. Paste this into your `CLAUDE.md`, `AGENTS.md`, or system prompt:

```markdown
## Paperclips

Log both directions during work, not after it.

Friction, meaning a dead-end tool call, a misleading doc, a footgun config, a
missing helper:

    papercuts add "<what you hit and what would have prevented it>" \
      --tag <area> --severity <minor|major|blocker> --where <component>

Wins, meaning something that exceeded expectations, saved real time, or turned
out to be more capable than its docs claimed:

    paperclip add "<what it did and the concrete value it delivered>" \
      --tag <area> --impact <trivial|solid|huge> --where <component>

`--where` names the component the record is about, in the same shape as the
path you would point a reviewer at: `parser/env`, `writer/atomic`,
`cli/parse`. It is the join key. When a cut and a clip carry the same
`--where`, review pairs them as an Overlap and asks whether that surface is
load-bearing and sharp-edged at the same time. Without `--where`, a record can
only join at tag level, which is the weakest signal in the system.

File it and keep working. Do not stop the task to write a record. "Worked
correctly" is not a win; name what it did for you. Severity: minor for an
annoyance, major for a time sink, blocker for a hard wall. Impact: trivial for
a nice surprise, solid for real time saved, huge for a change of approach.

End of session: if you changed code or hit friction, leave at least one cut or
clip in the log. If nothing is worth recording, say "nothing to reflect" out
loud instead of passing over it in silence.

The full contract is one command away: `papercuts schema` and
`paperclip schema` list every flag, record field, and exit code.
```

Then read it back on a schedule that fits the project: `paperclip review --format md` for a disposition worksheet over every overlap, or `papercuts list --format md` for the open cuts alone.

## Install

The workspace ships two binaries, so each install names its package:

```bash
cargo install --git https://github.com/dlorp/paperclips papercuts
cargo install --git https://github.com/dlorp/paperclips paperclip
```

## Minimum Supported Rust Version

**Rust 1.89**, enforced via `rust-version` in each crate's `Cargo.toml`.

## Quick start

The entries below are real records from production use, sanitized: names, paths, and internal tooling removed, technical detail kept. Log friction, log wins, read them back.

```bash
# Log friction
papercuts add "macOS ships no timeout(1): the batch fetch wrapper dies exit 127 with no work done" \
  --tag shell --severity major --where shell/batch --agent dlorp

# A credential footgun rather than a code bug
papercuts add "stale GITHUB_TOKEN classic PAT 401s on every call while the gh CLI token is the live credential" \
  --tag github --severity minor --where github/auth --agent dlorp

# Log wins
paperclip add "package-lock version check filtered 25 stale Dependabot alerts in one pass" \
  --tag deps --impact solid --where deps/audit --agent dlorp

paperclip add "similar-endpoint citation dive surfaced a paper that an arXiv search rate-limit had hidden" \
  --tag research --impact solid --where research/citations --agent dlorp
```

```bash
papercuts list --format md
```

```
## Major
- [pc_74e49c1fcb4e] macOS ships no timeout(1): the batch fetch wrapper dies exit 127 with no work done: dlorp, 2026-10-18T08:00:00.000Z (shell)
## Minor
- [pc_1d0489e2e71c] stale GITHUB_TOKEN classic PAT 401s on every call while the gh CLI token is the live credential: dlorp, 2026-10-18T08:10:00.000Z (github)
```

```bash
paperclip list --format md
```

```
- cl_2cf3b8d7fdab  solid  deps/audit  package-lock version check filtered 25 stale Dependabot alerts in one pass
- cl_443f219be45c  solid  research/citations  similar-endpoint citation dive surfaced a paper that an arXiv search rate-limit had hidden
```

## Overlaps

`review` joins cuts and clips on the `--where` field (exact, then path-prefix, then tag) and renders a disposition worksheet.

Here is a real pair from production use, sanitized, where both records name the same component:

```
CUT:  vision check times out past 300s on exemplar PNG/JPG files, twice in one audit;
      fell back to prose calibration
CLIP: right-edge bright-pixel gap scan caught a mid-token JSON clip that the vision
      check had passed twice; cheap PIL check beats vision for margin overflow
```

```bash
paperclip review --format md
```

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

## Overlaps (1 exact)

### audit/image-margin-check  [exact · 1 cut / 1 clip]
  CUT  [major  ] pc_4b96660989ec: vision check times out past 300s on exemplar PNG/JPG files, twice in one audit; fell back to prose calibration
  CLIP [huge   ] cl_d96ff9f0ae4a: right-edge bright-pixel gap scan caught a mid-token JSON clip that the vision check had passed twice; cheap PIL check beats vision for margin overflow
  Disposition: ____________________
```

That is Shape A. The pixel scan is load-bearing, and the vision check is the sharp edge. Bound the timeout and keep the pixel check as the default for margin work.

## Resolve with verification

A resolution must carry evidence. `resolve` rejects a record that names a fix but never says how effectiveness was checked; a link without verification reproduces the exact failure it is meant to catch.

```bash
papercuts resolve pc_ef087953afa5 \
  --agent dlorp \
  --resolution "resolve the timeout runner via command -v timeout -> gtimeout -> python3 subprocess fallback" \
  --resolved-by dlorp \
  --verification-method "ran the batch wrapper on the macOS host and checked the exit code" \
  --verification-result "wrapper completes; no exit 127" \
  --linked-task null
```

```json
{"disposition":"resolved","resolution":"resolve the timeout runner via command -v timeout -> gtimeout -> python3 subprocess fallback","resolved_by":"dlorp","verification_method":"ran the batch wrapper on the macOS host and checked the exit code","verification_result":"wrapper completes; no exit 127"}
```

`wontfix` is the other terminal state. It requires a reason and leaves the source log untouched:

```bash
papercuts wontfix pc_97c6b47409a5 \
  --reason "upstream watcher behavior; debounce is documented" \
  --owner dlorp
```

```json
{"feedback_id":"pc_97c6b47409a5","disposition":"wontfix","reason":"upstream watcher behavior; debounce is documented","resolved_by":"dlorp"}
```

Resolutions and wontfix decisions append to `.feedback-resolutions.jsonl` (override with `FEEDBACK_RESOLUTIONS_FILE`). The latest record for an item wins, so history is preserved without editing either log.

## Recurrence anchoring

When a new cut is the same failure as an existing one (identical normalized text, or the same known failure class), it carries `recurrence_of` pointing at the earliest matching cut:

```bash
papercuts add "batch wrapper exits 127 because macOS ships no timeout(1)" \
  --agent dlorp --tag shell --severity blocker --where shell/batch
```

```json
{"id":"pc_520bc6d53056","text":"batch wrapper exits 127 because macOS ships no timeout(1)","recurrence_of":"pc_ef087953afa5"}
```

Detection happens at write time. Historic records are never rewritten.

## Reopen on refailure

Reopening needs no special verb. File the failure again. The new record is open, and `recurrence_of` anchors it to the cut that was resolved before, so a fix that did not hold shows up as a chain instead of a fresh mystery:

```bash
papercuts list --unresolved --format md
```

```
## Blocker
- [pc_520bc6d53056] batch wrapper exits 127 because macOS ships no timeout(1): dlorp, 2026-10-18T12:00:00.000Z (shell)
## Minor
- [pc_a8004bc9d7bc] duplicate uploads when the folder watcher fired twice: dlorp, 2026-10-18T12:05:00.000Z (media)
```

The resolved cut (`pc_ef087953afa5`) stays closed and the refailure is open with `recurrence_of` pointing at it. Nothing was edited to make that happen.

`--unresolved` drops anything with a terminal disposition. `--digest` summarizes what is left:

```bash
papercuts list --digest
```

```json
{"ok":true,"data":{"open_by_severity":{"minor":1,"major":0,"blocker":1,"unset":0},"total_open":2}}
```

## Promote

```bash
paperclip promote cl_2cf3b8d7fdab --agent dlorp
```

```json
{"ok":true,"data":{"changed":true,"id":"cl_2cf3b8d7fdab","status":"promoted"}}
```

```bash
paperclip note cl_2cf3b8d7fdab "adopted in the dependency audit guide" --agent dlorp
```

```json
{"ok":true,"data":{"changed":true,"id":"cl_2cf3b8d7fdab","notes":1}}
```

## Batch helpers

`contrib/` holds two helpers for a log that has grown past the point where reading it top to bottom is useful. Both take paths from flags or environment and work in any repository:

- `contrib/papercuts-route.py --digest` clusters unresolved cuts by tag and component, prints recurring clusters first, and writes the full report. `--resolve RULES.json` closes matching cuts by calling the CLI, dry run unless `--apply`.
- `contrib/papercuts-digest.sh` is the one-line digest wrapper.

See [contrib/README.md](contrib/README.md).

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

Every command whose output is shown above was run against a scratch directory with the merged binaries, `PAPERCUTS_NOW` pinned for stable timestamps, and an explicit `--file`, so the code blocks are the output the binaries printed. JSON envelopes are trimmed to the fields under discussion; the full envelope adds `ok`, `meta`, and the echoed source record.

The entry texts are real records from production use, sanitized. Names, paths, and internal tooling were removed; the technical detail was not.

The JSONL logs and the dispositions ledger are ignored by default; they are working data, not part of the repository.

## Attribution

Forked from [treygoff24/papercuts](https://github.com/treygoff24/papercuts) (MIT). The `papercuts` binary remains behavior-compatible with upstream.

## License

MIT
