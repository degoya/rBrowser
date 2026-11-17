#!/usr/bin/env bash
#
# Every world of the plugin contract has an SDK template, and every template belongs to a world
# (RB-010-03). Copied and adapted from rCommander's scripts/check-sdk-templates.sh. For each
# `world <name>-plugin` in crates/rb-plugin-api/wit/rbrowser-plugin.wit, sdk/templates/<name>/ has
# to exist and carry:
#
#   - src/guest.rs generating that very world (`world: "<name>-plugin",`),
#   - manifest.toml naming that world in `kinds` and the contract's `api_version`,
#   - wit/rbrowser-plugin.wit, byte for byte the contract,
#   - locales/en.json, README.md and CHANGES.md (a `## <version>` section),
#   - at least one `#[test]` under src/, so `cargo test` in a fresh copy proves something,
#   - Cargo.toml with `license = "MIT"` and the workspace's `wit-bindgen` requirement.
#
# A template with a Nuxt UI panel (RB-080-87) carries rBrowser's panel runtime in
# web/web-common/: every file there is byte for byte its source in plugins/web-common/ (panel-ui
# and panel/texts.js, MIT), and web/package.json pins each package panel-ui pins to the same
# version.
#
# A template folder no world names is refused too, and sdk/LICENSE (MIT) and sdk/ci/plugin.yml
# must exist. Pure bash, no build.
#
# --build additionally builds a copy of each template on its own, as a plugin author would:
# `cargo test`, `cargo build --release --target wasm32-unknown-unknown`, `wasm-tools component
# new`, then scripts/check-plugin-imports.sh; a template with web/ first `npm ci`, `npm run
# build` and `npm test` there. It needs the wasm32 target, wasm-tools and npm.
#
#   scripts/check-sdk-templates.sh [--build] [repo]
#
set -euo pipefail

build=0
if [[ "${1:-}" == --build ]]; then
    build=1
    shift
fi
ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
WIT="$ROOT/crates/rb-plugin-api/wit/rbrowser-plugin.wit"
API_RS="$ROOT/crates/rb-plugin-api/src/lib.rs"
TEMPLATES="$ROOT/sdk/templates"
WEB_COMMON="$ROOT/plugins/web-common"

findings=0
finding() { echo "!! $1" >&2; findings=$((findings + 1)); }

[[ -f "$WIT" ]] || { echo "!! $WIT is missing" >&2; exit 1; }
api_version="$(sed -n 's/^pub const API_VERSION: &str = "\(.*\)";/\1/p' "$API_RS")"
bindgen="$(sed -n 's/^wit-bindgen = "\(.*\)"/\1/p' "$ROOT/Cargo.toml")"
[[ -n "$api_version" ]] || finding "no API_VERSION in $API_RS"
[[ -n "$bindgen" ]] || finding "no wit-bindgen in [workspace.dependencies]"
grep -q '^MIT License' "$ROOT/sdk/LICENSE" 2> /dev/null || finding "sdk/LICENSE (MIT) is missing"
[[ -f "$ROOT/sdk/ci/plugin.yml" ]] || finding "sdk/ci/plugin.yml is missing"

# The panel runtime a template carries in web/web-common/ is plugins/web-common/'s, and its
# web/package.json pins what panel-ui pins.
check_panel_copy() {
    local folder="$1" web="$2" file rel pins
    while IFS= read -r -d '' file; do
        rel="${file#"$web/web-common/"}"
        if [[ ! -f "$WEB_COMMON/$rel" ]]; then
            finding "$folder: web/web-common/$rel has no source in plugins/web-common/"
        elif ! cmp -s "$file" "$WEB_COMMON/$rel"; then
            finding "$folder: web/web-common/$rel differs from plugins/web-common/$rel"
        fi
    done < <(find "$web/web-common" -type f -not -path '*/.nuxt-ui/*' -print0)
    pins="$(python3 - "$WEB_COMMON/panel-ui/package.json" "$web/package.json" << 'PY'
import json, sys
def pins(path):
    data = json.load(open(path, encoding="utf-8"))
    return {**data.get("dependencies", {}), **data.get("devDependencies", {})}
source, copy = pins(sys.argv[1]), pins(sys.argv[2])
for name in sorted(source.keys() & copy.keys()):
    if source[name] != copy[name]:
        print(f"{name} {copy[name]} (panel-ui: {source[name]})")
PY
)" || pins="web/package.json does not read"
    [[ -z "$pins" ]] || finding "$folder: web/package.json pins differ from panel-ui's: $pins"
}

mapfile -t worlds < <(sed -n 's/^world \([a-z-]*\)-plugin {.*/\1/p' "$WIT")
[[ ${#worlds[@]} -gt 0 ]] || finding "the contract has no world"

expected=()
for world in "${worlds[@]}"; do
    folder="$world"
    expected+=("$folder")
    dir="$TEMPLATES/$folder"
    [[ -d "$dir" ]] || { finding "$world-plugin has no template ($dir)"; continue; }
    grep -q "world: \"$world-plugin\"," "$dir/src/guest.rs" 2> /dev/null \
        || finding "$folder: src/guest.rs does not generate $world-plugin"
    grep -q "^kinds = \[.*\"$world-plugin\"" "$dir/manifest.toml" 2> /dev/null \
        || finding "$folder: manifest.toml does not name $world-plugin in kinds"
    grep -q "^api_version = \"$api_version\"" "$dir/manifest.toml" 2> /dev/null \
        || finding "$folder: manifest.toml does not name api_version $api_version"
    cmp -s "$WIT" "$dir/wit/rbrowser-plugin.wit" \
        || finding "$folder: wit/rbrowser-plugin.wit differs from the contract"
    for file in locales/en.json README.md CHANGES.md; do
        [[ -f "$dir/$file" ]] || finding "$folder: $file is missing"
    done
    grep -q '^## ' "$dir/CHANGES.md" 2> /dev/null || finding "$folder: CHANGES.md has no version"
    grep -rq '#\[test\]' "$dir/src" 2> /dev/null || finding "$folder: no #[test] under src/"
    grep -q '^license = "MIT"' "$dir/Cargo.toml" 2> /dev/null \
        || finding "$folder: Cargo.toml is not MIT"
    grep -q "^wit-bindgen = \"$bindgen\"" "$dir/Cargo.toml" 2> /dev/null \
        || finding "$folder: wit-bindgen is not the workspace's $bindgen"
    [[ -d "$dir/web/web-common" ]] && check_panel_copy "$folder" "$dir/web"
done

for dir in "$TEMPLATES"/*/; do
    folder="$(basename "$dir")"
    [[ " ${expected[*]} " == *" $folder "* ]] || finding "$folder: no world of the contract"
done

if [[ $findings -gt 0 ]]; then
    echo "!! $findings finding(s) in the SDK templates" >&2
    exit 1
fi
echo "==> sdk templates: ${#worlds[@]} worlds, ${#worlds[@]} templates"

[[ $build -eq 1 ]] || exit 0

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
for world in "${worlds[@]}"; do
    folder="$world"
    copy="$scratch/$folder"
    cp -r "$TEMPLATES/$folder" "$copy"
    echo "==> build $folder"
    crate="$(sed -n 's/^name = "\(.*\)"/\1/p' "$copy/Cargo.toml" | head -n 1 | tr - _)"
    if [[ -f "$copy/web/package.json" ]]; then
        (cd "$copy/web" && npm ci --no-audit --no-fund && npm run build && npm test)
    fi
    (
        cd "$copy"
        cargo test --quiet
        cargo build --quiet --release --target wasm32-unknown-unknown
        wasm-tools component new "target/wasm32-unknown-unknown/release/$crate.wasm" \
            -o target/component.wasm
    )
    "$ROOT/scripts/check-plugin-imports.sh" "$copy/target/component.wasm"
done
echo "==> sdk templates build on their own"
