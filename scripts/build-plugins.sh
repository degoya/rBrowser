#!/usr/bin/env bash
#
# Builds, checks and signs the first-party plugins (RB-010-03): every `plugins/<id>/` with a
# Cargo.toml and a manifest.toml becomes `target/plugins/<id>.rbplugin`, copied for the package
# (scripts/lib/package.sh) to `artifacts/plugins/<id>-<version>.rbplugin`. Copied and adapted
# from rCommander's scripts/build-plugins.sh.
#
# Per plugin: `cargo build --release --target wasm32-unknown-unknown`, `wasm-tools component new`,
# scripts/check-plugin-imports.sh, then `rb-plugin-pack package` - which checks the manifest, the
# members and the component's imports against the capabilities and verifies the result.
# A plugin that lists web-common panel files in its web-common.txt is packed from a stage with
# those files added to its panel/ (scripts/lib/plugin-stage.sh, RB-080-65).
#
# Every plugin is a workspace of its own, but they all build into one target directory,
# `target/lanes/plugins` (RB-080-25; the plugin tests of scripts/check.sh use the same one), so
# what they share - serde, wit-bindgen, rb-plugin-api - is compiled once, not per plugin.
#
# The signing key is the release key `rbrowser-plugin-v1` (docs/plugins.md, "Signing keys"): its
# PKCS#8 PEM lives outside the repository, by default the owner's
# ~/.config/rbrowser/rbrowser-plugin.key (as rCommander keeps its key); RB_PLUGIN_SIGNING_KEY names
# another file, and set to an empty value it means no key. Without a key the script refuses, unless
# --unsigned asks for development packages, which load only in development mode and are marked as
# such.
#
#   scripts/build-plugins.sh [--unsigned] [<id>...]
#   scripts/build-plugins.sh --list     # the ids a build makes, one per line; builds nothing
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLUGINS="$ROOT/plugins"
OUT="$ROOT/target/plugins"
ARTIFACTS="$ROOT/artifacts/plugins"
PLUGIN_TARGET="${RB_PLUGIN_TARGET_DIR:-$ROOT/target/lanes/plugins}"

# The first-party plugins: every folder of plugins/ with a Cargo.toml and a manifest.toml.
plugin_ids() {
    local dir
    for dir in "$PLUGINS"/*/; do
        [[ -f "$dir/Cargo.toml" && -f "$dir/manifest.toml" ]] && basename "$dir"
    done
    return 0
}

if [[ "${1:-}" == --list ]]; then
    plugin_ids
    exit 0
fi

# shellcheck source=lib/plugin-stage.sh
source "$ROOT/scripts/lib/plugin-stage.sh"

unsigned=0
if [[ "${1:-}" == --unsigned ]]; then
    unsigned=1
    shift
fi

key_args=()
if [[ $unsigned -eq 0 ]]; then
    key="${RB_PLUGIN_SIGNING_KEY-${XDG_CONFIG_HOME:-$HOME/.config}/rbrowser/rbrowser-plugin.key}"
    if [[ -z "$key" || ! -f "$key" ]]; then
        echo "!! no signing key at ${key:-<none>} (RB_PLUGIN_SIGNING_KEY); --unsigned builds development packages" >&2
        exit 2
    fi
    case "$key" in
        "$ROOT"/*) echo "!! the signing key must live outside the repository" >&2; exit 2 ;;
    esac
    key_args=(--key "$key")
fi

mapfile -t ids < <(
    if [[ $# -gt 0 ]]; then
        printf '%s\n' "$@"
    else
        plugin_ids
    fi
)
if [[ ${#ids[@]} -eq 0 ]]; then
    echo "==> no first-party plugins in plugins/ yet"
    exit 0
fi

mkdir -p "$OUT" "$ARTIFACTS"
cargo build --quiet --release -p rb-plugin-host --bin rb-plugin-pack
pack="$ROOT/target/release/rb-plugin-pack"
for id in "${ids[@]}"; do
    dir="$PLUGINS/$id"
    [[ -f "$dir/manifest.toml" ]] || { echo "!! plugins/$id has no manifest.toml" >&2; exit 1; }
    echo "==> $id"
    crate="$(sed -n 's/^name = "\(.*\)"/\1/p' "$dir/Cargo.toml" | head -n 1 | tr - _)"
    (cd "$dir" && CARGO_TARGET_DIR="$PLUGIN_TARGET" cargo build --quiet --release --target wasm32-unknown-unknown)
    component="$PLUGIN_TARGET/wasm32-unknown-unknown/release/$crate.wasm"
    wasm-tools component new "$component" -o "$OUT/$id.wasm"
    "$ROOT/scripts/check-plugin-imports.sh" "$OUT/$id.wasm"
    members="$(rb_plugin_pack_dir "$PLUGINS" "$id" "$OUT/stage/$id")"
    "$pack" package "$members" --component "$OUT/$id.wasm" --out "$OUT/$id.rbplugin" "${key_args[@]}"
    version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$dir/manifest.toml" | head -n 1)"
    cp "$OUT/$id.rbplugin" "$ARTIFACTS/$id-$version.rbplugin"
done
echo "==> ${#ids[@]} plugin(s) in target/plugins/"
