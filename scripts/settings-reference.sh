#!/usr/bin/env bash
#
# Writes the wiki's settings reference, reference/settings.md: every settings key, category by
# category and section by section in the settings window's order, with its label, values,
# default, scopes and notes (local only, protected from MCP, the plugin it comes from) and its
# deep link. Generated, never kept by hand; the rules and the sources are those of the settings
# lint (scripts/check-settings.sh). It compiles nothing, so --check runs in
# scripts/check.sh --preflight.
#
# The wiki is RB_WIKI, else the checkout beside the main repository (../rbrowser.wiki, found
# from a worktree too). --check exits 1 when the page is stale, and passes without a wiki or
# without the page (nothing generated yet); exit 2 when the data does not read or the lint finds
# a problem.
#
# Usage:
#   scripts/settings-reference.sh [<root>]            # rewrite the page
#   scripts/settings-reference.sh --check [<root>]    # is the page current?
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
check=()
if [[ "${1:-}" == --check ]]; then
    check=(--check)
    shift
fi
[[ $# -le 1 ]] || { echo "usage: scripts/settings-reference.sh [--check] [<root>]" >&2; exit 2; }
if [[ -z "${RB_WIKI:-}" ]]; then
    # The main checkout, also from a worktree: the parent of the common git directory.
    common="$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2> /dev/null || echo "$ROOT/.git")"
    RB_WIKI="$(dirname "$(dirname "$common")")/rbrowser.wiki"
fi
exec python3 -I "$ROOT/scripts/lib/settings_reference.py" "${1:-$ROOT}" "$RB_WIKI" "${check[@]}"
