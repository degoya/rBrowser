#!/usr/bin/env bash
#
# Writes the wiki pages generated from the repository's data (pattern: rCommander's generated
# references). So far reference/configuration.md, the file format of the project config and the
# user settings, from schema/config.v1.json and schema/settings.v1.json (RB-010-01). The MCP
# tool reference has its own generator, scripts/mcp-reference.sh (RB-030-08), as the keyboard
# reference has scripts/keyboard-reference.sh. Generated pages are never edited by hand.
#
# Compiles nothing - scripts/lib/export_wiki.py reads the checked-in schemas, which a rb-config
# test holds to the types. It writes the files only; committing the wiki is a step of its own.
# The wiki is the checkout beside the main repository (../rbrowser.wiki, found from a worktree
# too) or RB_WIKI; without one, --check and --dry-run still read the data and skip the pages.
#
# Usage:
#   scripts/export-wiki.sh             # rewrite the pages that changed
#   scripts/export-wiki.sh --check     # exit 1 if a page is stale or missing
#   scripts/export-wiki.sh --dry-run   # name the pages that would change, write nothing
#
# Exit 2 when the data does not read (a schema or the example config missing or not JSON).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# -le 1 ]] || { echo "usage: scripts/export-wiki.sh [--check|--dry-run]" >&2; exit 2; }
if [[ -z "${RB_WIKI:-}" ]]; then
    # The main checkout, also from a worktree: the parent of the common git directory.
    common="$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2> /dev/null || echo "$ROOT/.git")"
    RB_WIKI="$(dirname "$(dirname "$common")")/rbrowser.wiki"
fi
exec python3 -I -B "$ROOT/scripts/lib/export_wiki.py" "$ROOT" "$RB_WIKI" "$@"
