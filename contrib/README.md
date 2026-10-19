# contrib

Helpers for working a papercuts log that is too long to read top to bottom. Both
are shipped as run against real log data, not sketches.

| File | What it is |
|------|------------|
| `papercuts-route.py` | Digest of unresolved cuts, and rule-driven resolution that calls the `papercuts` CLI. |
| `papercuts-digest.sh` | One-line wrapper for `papercuts-route.py --digest`. |

Requirements: Python 3.9+ and the `papercuts` binary on `PATH` (or pointed at
with `--papercuts-bin`). No third-party dependencies.

## Digest

```bash
./papercuts-digest.sh --days 7
./papercuts-digest.sh --days 30 --store ../my-project/.papercuts.jsonl
./papercuts-digest.sh --highlight timeout --report-dir ./reports
```

Unresolved cuts are grouped by tag and component. Recurring clusters (three or
more cuts) print first, because a cluster that keeps returning usually wants a
guard or a lint rule, not another one-off repair. `--highlight` takes a term and
moves matching cuts to the top; repeat it for more terms. The full report lands
in `--report-dir` as `digest-YYYYMMDD.md`.

A real run against a production log, identifiers redacted for publication:

```
$ ./papercuts-digest.sh --days 7 --highlight timeout
Papercut routing digest: last 7d: 420 unresolved cut(s), 148 cluster(s), 20 highlighted.
Highlighted findings:
  pc_c4a53209471c a full-index rebuild is one 420s run with no batch or incremental mode
  pc_33b84c6d0603 GNU timeout not installed on the macOS host; scripts using `timeout N cmd` fail with command not found
  pc_abb99fc4b861 GNU timeout is not on this Mac by default (macOS ships no coreutils); a 'timeout ... python3 ...' wrapper exits 127
Recurring clusters (>=3, candidates for a script/lint/guard fix):
   35x [tag redacted]: e.g. pc_55d024be7c5c
   30x [tag redacted]: e.g. pc_66d065201f36
   27x [tag redacted]: e.g. pc_20260726133448
   21x [tag redacted]: e.g. pc_3e66d1cbd7d1
   15x [tag redacted]: e.g. pc_d1a60a41ed43
Resolve with: papercuts resolve <id> --resolution ... --resolved-by ... --verification-method ... --verification-result ...
report: .papercuts-digests/digest-20261018.md
```

The counts, ids, and cut texts are unchanged from the run. Tag labels are
redacted because they name the author's own internal surfaces, not paperclips.

## Rule-driven resolution

`--resolve RULES.json` closes open cuts that match a rule. It shells out to the
`papercuts` binary so the disposition ledger keeps the CLI's own schema. The
default is a dry run.

```bash
./papercuts-route.py --resolve rules.json                 # show what would close
./papercuts-route.py --resolve rules.json --apply         # write the resolutions
```

A rules file is a JSON array. Each rule uses either `ids` (a curated list) or
`match` (a regex over text, tags, and component), plus an optional `exclude`
regex:

```json
[
  {
    "name": "timeout runner",
    "match": "GNU timeout|timeout: command not found",
    "exclude": "capture-loop",
    "resolution": "resolve the timeout runner via command -v timeout -> gtimeout -> python3 subprocess.run(timeout=...)",
    "resolved_by": "dlorp",
    "verification_method": "ran the batch wrapper on the macOS host and checked the exit code",
    "verification_result": "wrapper completes; no exit 127",
    "change": "tools/batch.sh",
    "class": "code"
  }
]
```

`resolution`, `resolved_by`, `verification_method`, and `verification_result` are
required. A rule missing any of them is skipped with a message rather than
writing a resolution with no evidence. A rule that names an id that is not open
says so and leaves it alone.

Real run against the same production log, with the store and ledger copied into
a scratch directory first:

```
$ ./papercuts-route.py --resolve rules.json --store /tmp/dogfood/.papercuts.jsonl \
    --resolutions /tmp/dogfood/.feedback-resolutions.jsonl --papercuts-bin ./target/debug/papercuts
[timeout runner] 5 unresolved cut(s) match
    e.g. pc_3a5b82551f0e 'timeout' command not found on macOS (BSD); must run bare with a fallback
    e.g. pc_33b84c6d0603 GNU timeout not installed on macOS host; scripts using `timeout N cmd` fail with command not found
    e.g. pc_abb99fc4b861 GNU timeout is not on this Mac by default (macOS ships no coreutils); a wrapper exits 127
dry run: 0 cut(s)

$ ./papercuts-route.py --resolve rules.json --apply --store /tmp/dogfood/.papercuts.jsonl \
    --resolutions /tmp/dogfood/.feedback-resolutions.jsonl --papercuts-bin ./target/debug/papercuts
[timeout runner] 5 unresolved cut(s) match
resolved: 5 cut(s)
```

Each resolution written by that run carries the rule's verification method and
result. The digest afterwards reported 416 unresolved cuts instead of 420,
because one of the five matched cuts fell outside the seven-day window.

## Paths

Everything is a flag or an environment variable. Nothing assumes a directory
layout.

| Flag | Environment | Default |
|------|-------------|---------|
| `--store` | `PAPERCUTS_FILE` | `./.papercuts.jsonl` |
| `--resolutions` | `FEEDBACK_RESOLUTIONS_FILE` | `.feedback-resolutions.jsonl` beside the store |
| `--papercuts-bin` | `PAPERCUTS_BIN` | `papercuts` on `PATH` |
| `--report-dir` | `PAPERCUTS_DIGEST_DIR` | `./.papercuts-digests` |

When `--resolve` runs, it passes `--file <store>` and exports
`FEEDBACK_RESOLUTIONS_FILE`, so a store outside the working directory still
resolves against the log you named.
