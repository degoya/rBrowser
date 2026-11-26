#!/usr/bin/env bash
#
# The shell's bundle budget (RB-099-20): every page of the ui/ build loads its entry, its
# preloaded chunks and its stylesheets within the JS and CSS bytes ui/bundle-budget.json gives
# it. Reads the build, builds nothing: run it after `npm run build` in ui/ (check.sh --preflight
# does, right after the npm-ui stage). The rules are scripts/lib/bundle_budget.py.
#
#   scripts/bundle-budget.sh [--dist <dir>] [--budget <file>]   # default ui/dist, ui/bundle-budget.json
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dist="$ROOT/ui/dist"
budget="$ROOT/ui/bundle-budget.json"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dist) dist="${2:?--dist needs a directory}"; shift 2 ;;
        --budget) budget="${2:?--budget needs a file}"; shift 2 ;;
        *) echo "usage: scripts/bundle-budget.sh [--dist <dir>] [--budget <file>]" >&2; exit 2 ;;
    esac
done
exec python3 "$ROOT/scripts/lib/bundle_budget.py" "$dist" "$budget"
