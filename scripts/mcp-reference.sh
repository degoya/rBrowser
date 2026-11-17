#!/usr/bin/env bash
#
# Writes the wiki's MCP tool reference, reference/mcp-tools.md: every tool of the MCP server area
# by area, with its description, a table of its parameters, the views it acts on (target), the
# command it runs and its guard (a setting that must be on, keys it refuses), then the tools still
# to come with their jobs and the commands no tool runs, with the reasons (RB-030-08). Generated,
# never kept by hand. The tools come from schema/mcp-tools.v1.json, which a test of rb-mcp writes
# from the tool definitions (RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-mcp mcp_tools_file) and
# holds current; the commands from schema/commands.v1.json; the bundled plugins' tools and
# commands from plugins/*/manifest.toml, marked as a plugin's. It compiles nothing, so --check
# runs in scripts/check.sh --preflight.
#
# The wiki is RB_WIKI, else the checkout beside the main repository (../rbrowser.wiki, found
# from a worktree too). --check exits 1 when the page is stale, and passes without a wiki or
# without the page (nothing generated yet); exit 2 when the data does not read.
#
# Usage:
#   scripts/mcp-reference.sh [<root>]            # rewrite the page
#   scripts/mcp-reference.sh --check [<root>]    # is the page current?
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
check=()
if [[ "${1:-}" == --check ]]; then
    check=(--check)
    shift
fi
[[ $# -le 1 ]] || { echo "usage: scripts/mcp-reference.sh [--check] [<root>]" >&2; exit 2; }
if [[ -z "${RB_WIKI:-}" ]]; then
    # The main checkout, also from a worktree: the parent of the common git directory.
    common="$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2> /dev/null || echo "$ROOT/.git")"
    RB_WIKI="$(dirname "$(dirname "$common")")/rbrowser.wiki"
fi
exec python3 -I "$ROOT/scripts/lib/mcp_reference.py" "${1:-$ROOT}" "$RB_WIKI" "${check[@]}"
