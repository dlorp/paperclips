#!/usr/bin/env python3
"""papercuts-route.py — turn an unresolved cut log into a routing digest, and
close resolved cuts from a rules file, without spending agent inference.

Two modes over a papercuts JSONL log plus its feedback-resolutions ledger:

  --digest [--days N] [--report PATH]
      Unresolved cuts from the last N days, clustered by tag and component.
      Highlighted terms and recurring clusters (3+ hits) print first, because
      a cluster that keeps coming back is a candidate for a guard or lint fix
      rather than another one-off repair. Prints a short summary to stdout and
      writes the full report to a file. Never creates tasks; it surfaces what
      to route.

  --resolve RULES.json [--apply]
      Resolve open cuts matching each rule (regex over text, tags, component,
      or an explicit id list) by calling the papercuts CLI, so the ledger
      schema stays the CLI's own. Dry run unless --apply.

Rules file: a JSON array. Each entry takes either `ids` (curated list) or
`match` (regex) plus an optional `exclude` (regex), and must carry the
resolution fields the CLI requires:

  [
    {
      "name": "timeout runner",
      "match": "timeout|gtimeout",
      "resolution": "resolve the timeout runner via command -v timeout -> gtimeout",
      "resolved_by": "dlorp",
      "verification_method": "ran the batch wrapper on macOS and checked the exit code",
      "verification_result": "wrapper completes; no exit 127",
      "change": "tools/batch.sh",
      "class": "code"
    }
  ]

Usage:

  ./papercuts-route.py --digest --days 7
  ./papercuts-route.py --digest --store ../my-project/.papercuts.jsonl --days 30
  ./papercuts-route.py --resolve rules.json            # dry run
  ./papercuts-route.py --resolve rules.json --apply

Real example (weekly digest over a production log, agent field redacted):

  $ ./papercuts-digest.sh --days 7
  Papercut routing digest — last 7d: 41 unresolved cut(s), 18 cluster(s), 2 highlighted.
  Recurring clusters (>=3, candidates for a script/lint/guard fix):
    5x shell|batch-wrapper — e.g. pc_8ac1...

Paths default to the environment, so the helper works in any repo:

  --store         $PAPERCUTS_FILE            (default: ./.papercuts.jsonl)
  --resolutions   $FEEDBACK_RESOLUTIONS_FILE (default: <store dir>/.feedback-resolutions.jsonl)
  --papercuts-bin $PAPERCUTS_BIN             (default: papercuts on PATH)
  --report-dir    $PAPERCUTS_DIGEST_DIR      (default: ./.papercuts-digests)
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import re
import subprocess
import sys
from collections.abc import Iterable
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

TERMINAL_DISPOSITIONS = ("resolved", "wontfix", "duplicate", "already_fixed")


def load_cuts(store: Path) -> list[dict[str, Any]]:
    """Read cut records. Malformed lines are skipped, never fatal."""
    cuts: list[dict[str, Any]] = []
    if not store.is_file():
        return cuts
    with store.open(encoding="utf-8", errors="replace") as fh:
        for line in fh:
            try:
                entry = json.loads(line)
            except ValueError:
                continue
            if isinstance(entry, dict) and entry.get("kind", "cut") == "cut" and entry.get("id"):
                cuts.append(entry)
    return cuts


def unresolved_ids(store: Path, resolutions: Path) -> set[str]:
    """Ids without a terminal disposition.

    A resolve event written into the source log closes the item; the feedback
    ledger is checked after it, and the latest ledger record for an id wins.
    """
    closed: set[str] = set()
    if store.is_file():
        with store.open(encoding="utf-8", errors="replace") as fh:
            for line in fh:
                try:
                    entry = json.loads(line)
                except ValueError:
                    continue
                if isinstance(entry, dict) and entry.get("kind") == "resolve" and entry.get("id"):
                    closed.add(entry["id"])

    latest: dict[str, str] = {}
    if resolutions.is_file():
        with resolutions.open(encoding="utf-8", errors="replace") as fh:
            for line in fh:
                try:
                    record = json.loads(line)
                except ValueError:
                    continue
                if isinstance(record, dict) and record.get("feedback_id"):
                    latest[record["feedback_id"]] = str(record.get("disposition") or "")

    closed.update(
        item for item, disposition in latest.items() if disposition in TERMINAL_DISPOSITIONS
    )
    return {cut["id"] for cut in load_cuts(store) if cut["id"] not in closed}


def parse_ts(entry: dict[str, Any]) -> datetime | None:
    raw = str(entry.get("ts") or "")
    try:
        return datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError:
        return None


def haystack(entry: dict[str, Any]) -> str:
    tags = entry.get("tags") or ([entry["tag"]] if entry.get("tag") else [])
    fields = [str(entry.get("text") or ""), " ".join(map(str, tags)), str(entry.get("where") or "")]
    return " ".join(fields)


def cluster_key(entry: dict[str, Any]) -> str:
    tags = entry.get("tags") or ([entry["tag"]] if entry.get("tag") else [])
    tag = str(tags[0]) if tags else "untagged"
    where = str(entry.get("where") or "").split("/")[-1][:40]
    return f"{tag}|{where}" if where else tag


def resolve(
    store: Path, resolutions: Path, rules_path: Path, apply: bool, binary: str
) -> int:
    rules = json.loads(rules_path.read_text(encoding="utf-8"))
    open_ids = unresolved_ids(store, resolutions)
    cuts = [cut for cut in load_cuts(store) if cut["id"] in open_ids]
    total = 0
    for rule in rules:
        name = rule.get("name") or "rule"
        required = ("resolution", "resolved_by", "verification_method", "verification_result")
        missing_fields = [field for field in required if not rule.get(field)]
        if missing_fields:
            print(f"[{name}] skipping rule, missing required field(s): {', '.join(missing_fields)}")
            continue
        if rule.get("ids"):
            wanted = set(rule["ids"])
            hits = [cut for cut in cuts if cut["id"] in wanted]
            not_open = wanted - {cut["id"] for cut in hits}
            if not_open:
                print(f"[{name}] skipping {len(not_open)} id(s) not open: {sorted(not_open)}")
        else:
            if not rule.get("match"):
                print(f"[{name}] skipping rule, needs `ids` or `match`")
                continue
            regex = re.compile(rule["match"], re.IGNORECASE)
            exclude = re.compile(rule["exclude"], re.IGNORECASE) if rule.get("exclude") else None
            hits = [
                cut
                for cut in cuts
                if regex.search(haystack(cut)) and not (exclude and exclude.search(haystack(cut)))
            ]
        print(f"[{name}] {len(hits)} unresolved cut(s) match")
        for cut in hits[:3]:
            preview = str(cut.get("text"))[:110]
            print(f"    e.g. {cut['id']} {preview}")
        if not apply:
            continue
        for cut in hits:
            command = [
                binary,
                "--file",
                str(store),
                "resolve",
                cut["id"],
                "--resolution",
                rule["resolution"],
                "--resolved-by",
                rule["resolved_by"],
                "--verification-method",
                rule["verification_method"],
                "--verification-result",
                rule["verification_result"],
            ]
            if rule.get("change"):
                command += ["--change", rule["change"]]
            if rule.get("class"):
                command += ["--class", rule["class"]]
            if rule.get("linked_task"):
                command += ["--linked-task", rule["linked_task"]]
            env = dict(os.environ)
            env["FEEDBACK_RESOLUTIONS_FILE"] = str(resolutions)
            result = subprocess.run(
                command, capture_output=True, text=True, timeout=60, check=False, env=env
            )
            if result.returncode:
                message = (result.stderr or result.stdout or "").strip()[:200]
                print(f"[{name}] resolve failed for {cut['id']}: {message}", file=sys.stderr)
            else:
                total += 1
        cuts = [cut for cut in cuts if cut not in hits]
    print(f"{'resolved' if apply else 'dry run'}: {total} cut(s)")
    return 0


def show_resolutions(store: Path) -> Path:
    """The ledger path paired with this store: explicit override or a sibling."""
    override = os.environ.get("FEEDBACK_RESOLUTIONS_FILE")
    return Path(override) if override else store.parent / ".feedback-resolutions.jsonl"


def digest(
    store: Path,
    resolutions: Path,
    report_dir: Path,
    days: int,
    report_path: Path | None,
    highlights: list[str],
) -> int:
    since = datetime.now(timezone.utc) - timedelta(days=days)
    open_ids = unresolved_ids(store, resolutions)
    recent = [
        cut for cut in load_cuts(store) if cut["id"] in open_ids and (parse_ts(cut) or since) >= since
    ]
    clusters: dict[str, list[dict[str, Any]]] = collections.defaultdict(list)
    for cut in recent:
        clusters[cluster_key(cut)].append(cut)

    watched = [
        cut
        for cut in recent
        if any(term.lower() in haystack(cut).lower() for term in highlights)
    ]
    recurring = sorted(
        ((key, cuts) for key, cuts in clusters.items() if len(cuts) >= 3),
        key=lambda pair: -len(pair[1]),
    )

    lines = [
        f"Papercut routing digest: last {days}d: {len(recent)} unresolved cut(s), "
        f"{len(clusters)} cluster(s), {len(watched)} highlighted."
    ]
    if watched:
        lines.append("Highlighted findings:")
        lines += [f"  {cut['id']} {str(cut.get('text'))[:120]}" for cut in watched[:5]]
    if recurring:
        lines.append("Recurring clusters (>=3, candidates for a script/lint/guard fix):")
        lines += [f"  {len(cuts):3}x {key}: e.g. {cuts[0]['id']}" for key, cuts in recurring[:8]]
    if not watched and not recurring:
        lines.append("Nothing highlighted; no recurring clusters.")
    lines.append(
        "Resolve with: papercuts resolve <id> --resolution ... --resolved-by ... "
        "--verification-method ... --verification-result ..."
    )
    print("\n".join(lines))

    target = report_path or report_dir / f"digest-{datetime.now():%Y%m%d}.md"
    target.parent.mkdir(parents=True, exist_ok=True)
    body = ["# Papercut routing digest", "", *lines, "", "## All clusters", ""]
    for key, cuts in sorted(clusters.items(), key=lambda pair: -len(pair[1])):
        body.append(f"### {key} ({len(cuts)})")
        body += [f"- {cut['id']} {str(cut.get('text'))[:200]}" for cut in cuts]
        body.append("")
    target.write_text("\n".join(body), encoding="utf-8")
    print(f"report: {target}")
    return 0


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--resolve", type=Path, metavar="RULES.json")
    mode.add_argument("--digest", action="store_true")
    parser.add_argument("--apply", action="store_true", help="with --resolve: write the resolutions")
    parser.add_argument("--days", type=int, default=7)
    parser.add_argument("--report", type=Path)
    parser.add_argument(
        "--store",
        type=Path,
        default=Path(os.environ.get("PAPERCUTS_FILE", ".papercuts.jsonl")),
    )
    parser.add_argument("--resolutions", type=Path)
    parser.add_argument("--report-dir", type=Path)
    parser.add_argument(
        "--papercuts-bin",
        default=os.environ.get("PAPERCUTS_BIN", "papercuts"),
        help="papercuts CLI to call with --resolve (default: $PAPERCUTS_BIN or papercuts)",
    )
    parser.add_argument(
        "--highlight",
        action="append",
        default=[],
        metavar="TERM",
        help="term to surface first in the digest; repeatable",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    store = args.store.expanduser()
    if not store.is_file():
        print(f"papercuts log not found: {store}", file=sys.stderr)
        return 1
    resolutions = (args.resolutions or show_resolutions(store)).expanduser()
    report_dir = (
        args.report_dir
        or Path(os.environ.get("PAPERCUTS_DIGEST_DIR", ".papercuts-digests"))
    ).expanduser()

    if args.resolve:
        return resolve(store, resolutions, args.resolve, args.apply, args.papercuts_bin)
    return digest(
        store, resolutions, report_dir, args.days, args.report, args.highlight
    )


if __name__ == "__main__":
    raise SystemExit(main())
