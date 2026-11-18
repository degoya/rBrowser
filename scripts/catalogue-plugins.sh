#!/usr/bin/env bash
#
# The first-party plugins the official plugin catalogue lists (RB-091-01, RB-091-04): every
# plugin folder but a `TEST-ONLY` one. As rDownloader does it, the package still carries the
# bundled ones in `plugins/` - the offline way - but a start installs only what the person chose,
# and each chosen plugin comes from the catalogue first when it offers the bundle's version or a
# newer one of the same key (docs/plugins.md, "Bundle and catalogue"). A folder marked
# `CATALOG-ONLY` is listed here and left out of the package (scripts/lib/package.sh).
# .github/workflows/release-plugins.yml signs and indexes these.
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
    basename "$dir"
done
