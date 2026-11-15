#!/usr/bin/env bash
#
# The plugin scripts (RB-010-03) on small trees in a temp directory: check-sdk-templates.sh passes
# on the repository and names a template whose contract differs, whose panel runtime is no longer
# plugins/web-common/'s (RB-080-87) or that belongs to no world;
# check-plugin-imports.sh refuses a missing file and, in its byte-scan fallback, a `wasi:` text;
# build-plugins.sh refuses to sign without a key and has nothing to build without plugins; the
# packing stage (scripts/lib/plugin-stage.sh) adds the web-common panel files a plugin lists.
#
# Pure bash, builds nothing. check.sh --preflight runs it.
#
#   scripts/tests/plugin-scripts.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

run_status bash "$ROOT/scripts/check-sdk-templates.sh" "$ROOT"
expect_status "the repository's templates fit the contract" 0

# A copy of what the template check reads.
TREE="$SCRATCH/tree"
mkdir -p "$TREE/crates/rb-plugin-api/src"
cp -r "$ROOT/crates/rb-plugin-api/wit" "$TREE/crates/rb-plugin-api/"
cp "$ROOT/crates/rb-plugin-api/src/lib.rs" "$TREE/crates/rb-plugin-api/src/"
cp "$ROOT/Cargo.toml" "$TREE/"
cp -r "$ROOT/sdk" "$TREE/"
mkdir -p "$TREE/plugins/web-common/panel"
cp -r "$ROOT/plugins/web-common/panel-ui" "$TREE/plugins/web-common/"
cp "$ROOT/plugins/web-common/panel/texts.js" "$TREE/plugins/web-common/panel/"
run_status bash "$ROOT/scripts/check-sdk-templates.sh" "$TREE"
expect_status "the copy passes as the repository does" 0

echo "// changed" >> "$TREE/sdk/templates/check/wit/rbrowser-plugin.wit"
run_status bash "$ROOT/scripts/check-sdk-templates.sh" "$TREE"
expect_status "a template whose contract differs is refused" 1
expect_output "the finding names the template" "check: wit/rbrowser-plugin.wit differs"
cp "$ROOT/sdk/templates/check/wit/rbrowser-plugin.wit" "$TREE/sdk/templates/check/wit/"

PANEL_COPY="$TREE/sdk/templates/feature/web/web-common"
echo "// changed" >> "$PANEL_COPY/panel-ui/src/mount.ts"
run_status bash "$ROOT/scripts/check-sdk-templates.sh" "$TREE"
expect_status "a template whose panel runtime differs is refused" 1
expect_output "the finding names the file" "feature: web/web-common/panel-ui/src/mount.ts differs"
cp "$ROOT/plugins/web-common/panel-ui/src/mount.ts" "$PANEL_COPY/panel-ui/src/"

sed -i 's/"vue": "[^"]*"/"vue": "0.0.1"/' "$TREE/sdk/templates/feature/web/package.json"
run_status bash "$ROOT/scripts/check-sdk-templates.sh" "$TREE"
expect_status "a template pinning another Vue than panel-ui is refused" 1
expect_output "the finding names the package" "pins differ from panel-ui's: vue 0.0.1"
cp "$ROOT/sdk/templates/feature/web/package.json" "$TREE/sdk/templates/feature/web/"

mkdir -p "$TREE/sdk/templates/viewer"
run_status bash "$ROOT/scripts/check-sdk-templates.sh" "$TREE"
expect_status "a template of no world is refused" 1
expect_output "the finding names the folder" "viewer: no world of the contract"

run_status bash "$ROOT/scripts/check-plugin-imports.sh"
expect_status "the import check wants a component" 2
printf 'module bytes wasi:filesystem/types' > "$SCRATCH/wasi.wasm"
printf 'module bytes rbrowser:plugin/host' > "$SCRATCH/clean.wasm"
WASM_TOOLS="$SCRATCH/no-wasm-tools" run_status bash "$ROOT/scripts/check-plugin-imports.sh" \
    "$SCRATCH/clean.wasm"
expect_status "the byte scan passes a component without wasi" 0
WASM_TOOLS="$SCRATCH/no-wasm-tools" run_status bash "$ROOT/scripts/check-plugin-imports.sh" \
    "$SCRATCH/wasi.wasm"
expect_status "the byte scan refuses a wasi text" 1
WASM_TOOLS="$SCRATCH/no-wasm-tools" run_status bash "$ROOT/scripts/check-plugin-imports.sh" \
    "$SCRATCH/missing.wasm"
expect_status "a missing component is refused" 1

RB_PLUGIN_SIGNING_KEY="" run_status bash "$ROOT/scripts/build-plugins.sh"
expect_status "no signing without a key" 2
mkdir -p "$SCRATCH/home"
run_status env -u RB_PLUGIN_SIGNING_KEY -u XDG_CONFIG_HOME HOME="$SCRATCH/home" \
    bash "$ROOT/scripts/build-plugins.sh"
expect_status "no signing without the default key file" 2
expect_output "the default key path is named" "$SCRATCH/home/.config/rbrowser/rbrowser-plugin.key"
RB_PLUGIN_SIGNING_KEY="$ROOT/Cargo.toml" run_status bash "$ROOT/scripts/build-plugins.sh"
expect_status "a key inside the repository is refused" 2

# --list names the plugins a build would make, one id per line, without a key and without
# building: scripts/lib/package.sh reads it (RB-010-15).
LISTED="$SCRATCH/listed"
mkdir -p "$LISTED/scripts" "$LISTED/plugins/alpha" "$LISTED/plugins/no-manifest"
cp "$ROOT/scripts/build-plugins.sh" "$LISTED/scripts/"
touch "$LISTED/plugins/alpha/Cargo.toml" "$LISTED/plugins/alpha/manifest.toml" \
    "$LISTED/plugins/no-manifest/Cargo.toml"
RB_PLUGIN_SIGNING_KEY="" run_status bash "$LISTED/scripts/build-plugins.sh" --list
expect_status "--list needs no key" 0
expect_true "--list names exactly the plugins with a manifest" '[[ "$output" == alpha ]]'

# The web-common panel files a plugin lists in web-common.txt go into the packed panel/
# (RB-080-65): one source, no checked-in copies.
# shellcheck source=../lib/plugin-stage.sh
source "$ROOT/scripts/lib/plugin-stage.sh"
SHARED="$SCRATCH/shared"
mkdir -p "$SHARED/web-common/panel" "$SHARED/beta/panel" "$SHARED/beta/locales" "$SHARED/gamma/panel"
printf 'tokens' > "$SHARED/web-common/panel/highlight-tokens.js"
printf 'colours' > "$SHARED/web-common/panel/highlight.js"
printf 'id = "beta"' > "$SHARED/beta/manifest.toml"
printf '{}' > "$SHARED/beta/locales/en.json"
printf 'main' > "$SHARED/beta/panel/panel.js"
printf '# shared\nhighlight-tokens.js\n\nhighlight.js  # colours\n' > "$SHARED/beta/web-common.txt"
touch "$SHARED/gamma/manifest.toml"
run_status rb_plugin_pack_dir "$SHARED" gamma "$SCRATCH/stage-gamma"
expect_status "a plugin without web-common.txt packs its own folder" 0
expect "the own folder is named" "$SHARED/gamma" "$output"
run_status rb_plugin_pack_dir "$SHARED" beta "$SCRATCH/stage-beta"
expect_status "a plugin with web-common.txt packs a stage" 0
expect "the stage is named" "$SCRATCH/stage-beta" "$output"
expect "the listed file is in the stage's panel" "colours" "$(cat "$SCRATCH/stage-beta/panel/highlight.js")"
expect "and the other one" "tokens" "$(cat "$SCRATCH/stage-beta/panel/highlight-tokens.js")"
expect "the plugin's own panel file is kept" "main" "$(cat "$SCRATCH/stage-beta/panel/panel.js")"
expect "its locales are kept" "{}" "$(cat "$SCRATCH/stage-beta/locales/en.json")"
expect_true "the list itself is not packed" '[[ ! -e "$SCRATCH/stage-beta/web-common.txt" ]]'
expect_true "the source folder is untouched" '[[ ! -e "$SHARED/beta/panel/highlight.js" ]]'
printf 'colours' > "$SHARED/beta/panel/highlight.js"
run_status rb_plugin_pack_dir "$SHARED" beta "$SCRATCH/stage-beta"
expect_status "a checked-in copy of a listed file is refused" 1
expect_output "the copy is named" "plugins/beta/panel/highlight.js"
rm "$SHARED/beta/panel/highlight.js"
for bad in '../secret.js' 'sub/file.js' 'missing.js'; do
    printf '%s\n' "$bad" > "$SHARED/beta/web-common.txt"
    run_status rb_plugin_pack_dir "$SHARED" beta "$SCRATCH/stage-beta"
    expect_status "the listed name '$bad' is refused" 1
done

finish_tests plugin-scripts
