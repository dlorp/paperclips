#!/usr/bin/env bash
# papercuts-digest.sh — print a routing digest of unresolved cuts and write the
# full report to a file. Thin wrapper over papercuts-route.py --digest, usable
# from a scheduler, a shell profile, or by hand.
#
# Usage:
#   ./papercuts-digest.sh [--days N] [--store PATH] [--report-dir PATH] [--highlight TERM]
#
# Every papercuts-route.py flag passes through. Paths come from the environment
# when not given: $PAPERCUTS_FILE, $FEEDBACK_RESOLUTIONS_FILE,
# $PAPERCUTS_DIGEST_DIR. Store defaults to ./.papercuts.jsonl.
#
# Real example (7-day digest over a production log, agent field redacted):
#   $ ./papercuts-digest.sh --days 7
#   Papercut routing digest: last 7d: 41 unresolved cut(s), 18 cluster(s), 2 highlighted.
#   report: .papercuts-digests/digest-20261018.md
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$script_dir/papercuts-route.py" --digest "$@"
