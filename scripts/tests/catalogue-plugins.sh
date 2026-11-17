#!/usr/bin/env bash
#
# The official catalogue never lists a bundled plugin (RB-091-01, coordinator 2026-11-16):
# scripts/catalogue-plugins.sh names only the `CATALOG-ONLY` plugin folders, and the package's
# plugin list (scripts/lib/package.sh, rb_package_plugins) leaves exactly those out, so no id is
# in both. On a small tree in a temp directory; builds nothing.
#
#   scripts/tests/catalogue-plugins.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

TREE="$SCRATCH/tree"
mkdir -p "$TREE/scripts/lib" "$TREE/artifacts/plugins"
cp "$ROOT/scripts/build-plugins.sh" "$ROOT/scripts/catalogue-plugins.sh" "$TREE/scripts/"
cp "$ROOT/scripts/lib/"*.sh "$TREE/scripts/lib/"
plugin() {
    mkdir -p "$TREE/plugins/$1"
    printf '[package]\nname = "rbrowser-plugin-%s"\n' "$1" > "$TREE/plugins/$1/Cargo.toml"
    printf 'id = "%s"\nversion = "1.0.0"\n' "$1" > "$TREE/plugins/$1/manifest.toml"
    : > "$TREE/artifacts/plugins/$1-1.0.0.rbplugin"
}
plugin seo
plugin outline
touch "$TREE/plugins/outline/CATALOG-ONLY"
plugin hello
touch "$TREE/plugins/hello/TEST-ONLY" "$TREE/plugins/hello/CATALOG-ONLY"
mkdir -p "$TREE/plugins/web-common"

expect "the catalogue lists the catalogue-only plugin alone" "outline" \
    "$(bash "$TREE/scripts/catalogue-plugins.sh" "$TREE")"

# shellcheck source=../lib/package.sh
source "$ROOT/scripts/lib/package.sh"
bundled="$(rb_package_plugins "$TREE" "$TREE/artifacts/plugins" | xargs -n 1 basename | tr '\n' ' ')"
expect "the package bundles every other plugin but the test-only one" "seo-1.0.0.rbplugin " "$bundled"

expect "the catalogue of a tree without plugins is empty" "" \
    "$(bash "$TREE/scripts/catalogue-plugins.sh" "$SCRATCH/empty")"

finish_tests catalogue-plugins
