# papercuts — design doc

2026-07-09. Coordinator-authored. Status: draft for adversarial review.

## Thesis and provenance

Agents hit friction constantly — dead-end tool calls, broken links, missing helpers, footgun configs — and silently push through without telling anyone. The signal evaporates. `papercuts` is a tiny agent-first CLI that gives agents a one-line way to file the complaint at the moment they hit it, and gives humans (and other agents) a way to review and burn down the backlog.

Provenance: Steve Ruiz shipped a private version of this inside his repo (X post, 2026-07-09, 39K views / 770 bookmarks in hours) and reported it immediately surfaced real workflow defects his agents had been eating silently: unquoted zsh globs breaking `rg`, wrong test cwd in a yarn workspace, tab-indented YAML breaking deploys, stale Supabase CLI ambiguity. Every one is an actionable fix a human would never have heard about otherwise. This is a validated behavior pattern, not a speculative product.

Why a CLI and not an MCP server or harness feature: every agent harness (Claude Code, Codex, Cursor, Droid, anything) can shell out. A single static binary with a JSON contract is the lowest common denominator and needs zero per-harness integration. One line in an AGENTS.md/CLAUDE.md activates it.

## External contract

Binary and crate: `papercuts` (crates.io name verified free 2026-07-09; bare `papercut` is taken by an image tool). Repo: `treygoff24/papercuts`. License: MIT.

### Commands

```text
papercuts add <TEXT | ->        # file a papercut ('-' reads text from stdin)
papercuts list                  # read papercuts (default: open only, newest first)
papercuts resolve <ID>          # mark a papercut resolved (append-only event)
papercuts schema [all|record|error|exit-codes]   # machine contract, self-orientation
papercuts doctor [--fix]        # validate the log file, quarantine malformed lines
```

`log` is an alias of `add` (the verb people will guess from Steve's post); `add` is canonical.

Global flags: `--file <PATH>` (explicit log file, overrides discovery), `--pretty`, `--quiet`. No color anywhere, ever (agent-only tool; there is nothing to colorize — output is JSON).

### `add`

- Positional `TEXT` (or `-` for stdin; stdin also used when text is omitted and stdin is non-TTY).
- `--agent <NAME>`: reporter identity. Resolution order: flag → `PAPERCUTS_AGENT` env → harness detection (`CLAUDECODE`→`claude-code`, `CODEX_*`→`codex`, `CURSOR_*`→`cursor`) → `"unknown"`. The resolved value AND its source (`flag|env|detected|default`) are echoed in output meta — no silent ambient inference.
- `--tag <TAG>` (repeatable), `--severity minor|major|blocker` (default `minor`).
- Captures `cwd` and repo root automatically (filesystem walk for `.git`; no libgit2).
- Output: success envelope containing the full record + `meta.file` (resolved log path) + `meta.agent_source`.

### `list`

- Filters: `--status open|resolved|all` (default `open`), `--agent`, `--tag`, `--severity`, `--since <RFC3339 | Nd | Nh>`.
- `--limit N` (default 50) — bounded output by default; envelope carries `count`, `total`, `truncated`.
- `--format json|jsonl|md` (default `json`). `md` is the one human-facing surface: a compact review digest grouped by severity.
- Empty result is exit 0 with an empty array and a hint in `meta.warnings` — never exit 1.

### `resolve`

- `papercuts resolve <ID> [--note <TEXT>] [--agent <NAME>]`. Appends a `resolve` event; never rewrites history.
- Unknown ID → structured `not_found` error, exit 66, with a hint naming `papercuts list --status all`.
- Already-resolved ID → **idempotent success** with `meta.warnings: ["already resolved"]` (agents retry; retries must be safe).
- ID prefix matching: a unique prefix (≥4 chars) resolves; an ambiguous prefix errors listing the candidates (deterministic forgiveness — never guess between two).

### `schema`

Prints the full machine contract as JSON: contract version, every command/flag, record schemas, error codes, exit-code dictionary. This is the self-orientation surface; an agent that has never seen the tool runs `papercuts schema` and knows everything.

### `doctor`

- Validates the log file: every line parses as a known event, IDs well-formed, no duplicate cut IDs.
- `--fix`: moves malformed lines to `<file>.quarantine` via temp+fsync+rename under the file lock; valid lines untouched.
- Linter-style exit dictionary (0 healthy / 1 findings / 2 partial-fix / 3 fix-failed), published in `schema`.

### Envelope and exit codes

Success: `{"ok":true,"data":{…},"meta":{…}}` on stdout, single line (or pretty with `--pretty`).
Error: `{"ok":false,"error":{"code":"…","message":"…","details":{…},"retryable":bool,"suggested_fix":"paste-ready command"}}` on **stderr**.

Exit codes follow the rust-agent-cli skill dictionary: 0 success/empty, 2 usage, 65 bad input data, 66 missing file / not-found ID, 70 internal, 74 I/O error, 78 config. No network, no auth → 75/77 unused. Doctor uses its own published dictionary.

### Record shapes (contract v1)

Cut event:

```json
{"kind":"cut","id":"pc_a1b2c3d4e5f6","ts":"2026-07-09T18:30:00.123Z","agent":"claude-code","text":"rg failed: unquoted zsh glob expanded before rg ran; quote globs or use --files","tags":["shell","rg"],"severity":"minor","cwd":"/Users/x/proj/apps/web","repo":"/Users/x/proj"}
```

Resolve event:

```json
{"kind":"resolve","id":"pc_a1b2c3d4e5f6","ts":"2026-07-10T09:00:00.000Z","agent":"trey","note":"added rg wrapper to CLAUDE.md"}
```

- `id` = `pc_` + first 12 hex of SHA-256 over `ts|agent|text` — content-addressed, deterministic, collision-negligible at this scale.
- `ts` = UTC RFC3339 milliseconds. `PAPERCUTS_NOW` env (RFC3339) overrides the clock for reproducible tests — documented, not hidden.
- Unknown `kind` values are skipped by `list` with a `meta.warnings` count (forward compatibility) but flagged by `doctor`.

## Storage

**Append-only JSONL, event-sourced.** Per the state-and-persistence reference: append-only + no transactional check-then-act = JSONL, not SQLite. `resolve` is an appended event, not a mutation, so the file is never rewritten in normal operation (only `doctor --fix` rewrites, atomically). `list` folds cut+resolve events into current state at read time — trivial at the scale of a papercuts log (thousands of lines, single-digit ms).

File discovery order:

1. `--file PATH` flag
2. `PAPERCUTS_FILE` env
3. Walk up from cwd to the git repo root; use `<repo-root>/.papercuts.jsonl` (created on first `add`)
4. No repo → `~/.papercuts/log.jsonl`

The per-repo default is the point: the log travels with the repo, and every `add` shows up in `git diff` — exactly how Steve's screenshot surfaced (the green block IS the diff). Teams see papercuts in review for free.

Concurrency: writes open with `O_APPEND`, take an exclusive `std::fs::File::lock` (stabilized std, Rust ≥1.89 — no locking dep), write the single line, flush, unlock. Reads take a shared lock. Multiple concurrent agents on one file are safe.

## Dependencies (each justified)

- `clap` 4 (derive) — parser, per skill.
- `serde` + `serde_json` — every output shape is a struct.
- `thiserror` — typed public error contract.
- `jiff` — RFC3339 UTC timestamps, parsing `--since`. (Modern, well-audited; alternative `time` acceptable — implementer may pick, doc updated to match.)
- `sha2` — content-addressed IDs.
- Dev: `assert_cmd`, `predicates`, `tempfile`.

Nothing else. No tokio, no color crates, no config-file crate, no git library.

## Testing strategy

- Parser unit tests via `Cli::try_parse_from` (conflicts, defaults, bad values).
- Black-box CLI tests via `assert_cmd`: every command's success shape deserialized into its envelope struct; every error path asserts code + exit code + that the `suggested_fix` hint survives (pinned per the error-rewriting craft).
- Concurrency test: N threads `add` simultaneously against one file; assert N valid lines, no interleaving/corruption.
- Determinism test: two identical invocations with `PAPERCUTS_NOW` fixed produce byte-identical output.
- Quality gate: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo build --release`. 5x test sweep before any commit.
- Live acceptance (coordinator-driven): drive the real binary through the full agent lifecycle including empty states, malformed file, ambiguous prefixes, concurrent adds, stdin path.

## Distribution / ship plan

- Public GitHub repo `treygoff24/papercuts`, README written for two audiences: the human installing it, and the agent using it (an AGENTS.md-ready snippet to paste into any repo's agent instructions).
- `cargo install papercuts` as the v0.1.0 install path; `cargo publish` at ship.
- cargo-dist/homebrew/curl-installer deferred to a follow-up release (lens playbook exists; not v0.1 scope).

## Non-goals (v1)

- No server, sync, or telemetry — the file is the product.
- No TUI, no interactive anything.
- No dedup/clustering/AI summarization of cuts (the reviewing agent can do that; this tool is the substrate).
- No Windows CI (nothing platform-specific in the design; just untested).
- No `edit`/`delete` of history — append-only is a feature; `doctor --fix` is the only rewrite path.
- No config file.

## Wave plan

Slimmed foundry (reduced config: Codex authors and fixes, cross-family review via Cursor/Grok, coordinator independently gates and reads riskiest files).

- **Plan review** (this doc): `delegate codex safe --model sol --reasoning-effort xhigh` + `delegate cursor safe` in parallel; coordinator triages all findings in writing; doc amended.
- **Wave 1 — the whole CLI, one lane** (task-clustering: ~1000 LOC sharing one design; splitting would fragment coherence): `delegate codex work --model sol --reasoning-effort high`. Layout per skill: `main.rs`/`cli.rs`/`commands/`/`output.rs`/`error.rs`/`lib.rs`/`tests/`.
- **Review wave**: `delegate cursor safe` adversarial review of the diff + coordinator riskiest-file read (locking/append path, ID fold logic in `list`, doctor `--fix` rewrite). Triage → Codex fix round → coordinator verifies every fix landed → re-review until dry (3-round cap).
- **Acceptance**: coordinator drives the real binary. Zero unexplained failures.
- **Ship**: README/AGENTS.md, GitHub repo + push, tag v0.1.0, `cargo publish`.

Budget: subscription lanes only (Codex + Cursor); zero metered spend expected.
