#!/usr/bin/env bash
#
# The first-party plugins the official plugin catalogue lists (RB-091-01): the plugin folders
# marked `CATALOG-ONLY` - published in the catalogue instead of shipped with the app. A bundled
# plugin is never listed (coordinator, 2026-11-16): with the same id the bundled copy wins and its
# updates come with the app, so the catalogue offering it too would only compete with the app.
# Every other plugin folder is bundled (scripts/lib/package.sh skips `CATALOG-ONLY`); a
# `TEST-ONLY` folder is in neither. .github/workflows/release-plugins.yml signs and indexes these.
#
# Pure bash, builds nothing.
#
#   scripts/catalogue-plugins.sh [<root>]    # one plugin id per line
set -euo pipefail

ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
[[ -d "$ROOT/plugins" ]] || exit 0
for dir in "$ROOT"/plugins/*/; do
    dir="${dir%/}"
    [[ -f "$dir/Cargo.toml" && -f "$dir/manifest.toml" ]] || continue
    [[ -f "$dir/TEST-ONLY" ]] && continue
    [[ -f "$dir/CATALOG-ONLY" ]] || continue
    basename "$dir"
done
