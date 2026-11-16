#!/usr/bin/env bash
#
# Writes the wiki's keyboard reference, reference/keyboard-shortcuts.md: every command group by
# group in the leader's order, with its title, id, default keys and MCP tool, then the reserved
# combinations the shortcut editor refuses, with their reasons (RB-010-17). Generated, never kept
# by hand. The commands come from schema/commands.v1.json, which a test of crates/rbrowser writes
# from the command registry (RB_UPDATE_SCHEMA=1 cargo nextest run -p rbrowser commands_file) and
# holds current; the bundled plugins' commands from plugins/*/manifest.toml, marked as a
# plugin's; the reservations from crates/rb-commands/data/reserved-shortcuts.json. It compiles
# nothing, so --check runs in scripts/check.sh --preflight.
#
# The wiki is RB_WIKI, else the checkout beside the main repository (../rbrowser.wiki, found
# from a worktree too). --check exits 1 when the page is stale, and passes without a wiki or
# without the page (nothing generated yet); exit 2 when the data does not read.
#
# Usage:
#   scripts/keyboard-reference.sh [<root>]            # rewrite the page
#   scripts/keyboard-reference.sh --check [<root>]    # is the page current?
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
check=()
if [[ "${1:-}" == --check ]]; then
    check=(--check)
    shift
fi
[[ $# -le 1 ]] || { echo "usage: scripts/keyboard-reference.sh [--check] [<root>]" >&2; exit 2; }
if [[ -z "${RB_WIKI:-}" ]]; then
    # The main checkout, also from a worktree: the parent of the common git directory.
    common="$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2> /dev/null || echo "$ROOT/.git")"
    RB_WIKI="$(dirname "$(dirname "$common")")/rbrowser.wiki"
fi
exec python3 -I "$ROOT/scripts/lib/keyboard_reference.py" "${1:-$ROOT}" "$RB_WIKI" "${check[@]}"
