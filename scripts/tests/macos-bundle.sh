#!/usr/bin/env bash
#
# scripts/lib/macos-bundle.sh without a Mac (RB-090-15): rBrowser.app assembled from stand-in
# binaries and a stand-in CEF framework, its structure test, the Info.plists of the app and the
# five helpers, and the structure test failing on a bundle that lacks a part - the Node runtime
# and the sidecar included (RB-097-01), whose smoke and the size report run on stand-ins too.
#
#   scripts/tests/macos-bundle.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=../lib/macos-bundle.sh
source "$ROOT/scripts/lib/macos-bundle.sh"

expect "the plain helper has no id suffix" "" "$(rb_macos_helper_id "")"
expect "the GPU helper's id suffix" ".gpu" "$(rb_macos_helper_id " (GPU)")"
expect "the renderer helper's id suffix" ".renderer" "$(rb_macos_helper_id " (Renderer)")"

# Stand-ins: the build output, the CEF distribution, the shell.
BUILD="$SCRATCH/build"
mkdir -p "$BUILD"
printf 'app' > "$BUILD/rbrowser-nosandbox"
printf 'helper' > "$BUILD/rbrowser-helper"
CEF="$SCRATCH/cef_macos_aarch64"
FRAMEWORK="$CEF/Chromium Embedded Framework.framework"
mkdir -p "$FRAMEWORK/Libraries" "$FRAMEWORK/Resources"
printf 'cef' > "$FRAMEWORK/Chromium Embedded Framework"
printf 'gl' > "$FRAMEWORK/Libraries/libvulkan.dylib"
printf '<plist/>' > "$FRAMEWORK/Resources/Info.plist"
UI="$SCRATCH/ui-dist"
mkdir -p "$UI/assets"
printf '<!doctype html>' > "$UI/index.html"
printf 'export {}' > "$UI/assets/app.js"

APP="$SCRATCH/out/rBrowser.app"
run_status rb_macos_stage "$CEF" "$BUILD" "$APP" "0.9.0-beta.1+build" "$UI"
expect_status "the bundle is assembled" 0
run_status rb_macos_check "$APP"
expect_status "the structure test fails without the Node runtime and the sidecar" 1
expect_output "naming the runtime" "Contents/Resources/node/bin/node"
expect_output "and the sidecar" "Contents/Resources/sidecar/dist/main.js"

# Stand-ins for what scripts/package-macos.sh stages into Resources (RB-097-01): the runtime
# (a script that answers one request as the sidecar would, reading the version from $NODE_ANSWER
# beside it) and the sidecar.
RES="$APP/Contents/Resources"
mkdir -p "$RES/node/bin" "$RES/sidecar/dist" "$RES/sidecar/node_modules/playwright-core"
printf 'MIT\n' > "$RES/node/LICENSE"
cat > "$RES/node/bin/node" << 'SH'
#!/usr/bin/env bash
here="$(cd "$(dirname "$0")" && pwd)"
answer="$(cat "$here/answer")"
[[ "$answer" == none ]] && exit 3
IFS= read -r line
[[ "$line" == *'"method":"sidecar.versions"'* ]] || { echo "unexpected: $line" >&2; exit 4; }
printf '%s\n' "$answer"
cat > /dev/null
SH
chmod 755 "$RES/node/bin/node"
printf '%s' '{"jsonrpc":"2.0","id":1,"result":{"node":"9.9.9","playwright":"1.65.1","browsers":[]}}' \
    > "$RES/node/bin/answer"
for file in dist/main.js dist/flow-worker.js package.json node_modules/playwright-core/package.json; do
    printf '{}\n' > "$RES/sidecar/$file"
done
run_status rb_macos_check "$APP"
expect_status "the structure test passes" 0

# --- the sidecar smoke: the bundled runtime starts the bundled sidecar, which answers.
run_status rb_macos_sidecar_smoke "$APP" 9.9.9
expect_status "the bundled sidecar answers with the pinned Node" 0
expect_output "and names both versions" "node 9.9.9, playwright-core 1.65.1"
run_status rb_macos_sidecar_smoke "$APP" 9.9.8
expect_status "another Node version fails the smoke" 1
expect_output "and is named" "9.9.8"
printf 'none' > "$RES/node/bin/answer"
run_status rb_macos_sidecar_smoke "$APP" 9.9.9
expect_status "a sidecar that ends without an answer fails the smoke" 1
expect_output "and says so" "no answer"
rm "$RES/node/bin/answer"

# --- the size report names the parts, the runtime and the sidecar among them.
printf 'zip' > "$SCRATCH/out/rb.zip"
run_status rb_macos_sizes "$APP" "$SCRATCH/out/rb.zip"
expect_status "the size report runs" 0
for part in "rbrowser" "CEF framework" "helper apps" "shell UI" "Node runtime" "sidecar" "total, unpacked" "ZIP"; do
    expect_output "it names $part" "$part"
done
expect "the app is the crate's plain binary" "app" "$(cat "$APP/Contents/MacOS/rbrowser")"
expect_true "the app is executable" '[[ -x $APP/Contents/MacOS/rbrowser ]]'
expect_true "the framework and its libraries are in Frameworks" \
    '[[ -f "$APP/Contents/Frameworks/Chromium Embedded Framework.framework/Libraries/libvulkan.dylib" ]]'
expect_true "the shell is Resources/ui" '[[ -f $APP/Contents/Resources/ui/index.html && -f $APP/Contents/Resources/ui/assets/app.js ]]'
for suffix in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
    helper="$APP/Contents/Frameworks/rbrowser Helper$suffix.app/Contents"
    expect "rbrowser Helper$suffix runs the helper binary" "helper" "$(cat "$helper/MacOS/rbrowser Helper$suffix")"
    expect_true "rbrowser Helper$suffix has no Dock icon" 'grep -q "<key>LSUIElement</key>" "$helper/Info.plist"'
done
expect_true "the GPU helper's bundle id" \
    'grep -q "<string>net.rbrowser.app.helper.gpu</string>" "$APP/Contents/Frameworks/rbrowser Helper (GPU).app/Contents/Info.plist"'
expect_true "the helpers' display name says rBrowser" \
    'grep -q "<string>rBrowser Helper (Renderer)</string>" "$APP/Contents/Frameworks/rbrowser Helper (Renderer).app/Contents/Info.plist"'
# shellcheck disable=SC2034  # read by the expect_true conditions below
plist="$APP/Contents/Info.plist"
expect_true "the app's bundle id" 'grep -q "<string>net.rbrowser.app</string>" "$plist"'
expect_true "Apple's version has three numbers at most" 'grep -q "<string>0.9.0</string>" "$plist" && ! grep -q "beta" "$plist"'
expect_true "the minimum macOS is CEF's" 'grep -A1 LSMinimumSystemVersion "$plist" | grep -q "<string>13.0</string>"'
expect_true "the app names its icon" 'grep -A1 CFBundleIconFile "$plist" | grep -q "<string>rbrowser</string>"'
expect_true "the app has a Dock icon" '! grep -q LSUIElement "$plist"'
if command -v python3 > /dev/null; then
    expect_true "every Info.plist is a well-formed property list" \
        'find "$APP" -name Info.plist -path "*Contents/Info.plist" -print0 | xargs -0 -n1 python3 -I -c "import plistlib, sys; plistlib.load(open(sys.argv[1], \"rb\"))"'
fi

chmod 644 "$RES/node/bin/node"
run_status rb_macos_check "$APP"
expect_status "a runtime that is not executable fails the structure test" 1
expect_output "the failure names it" "no executable Contents/Resources/node/bin/node"
chmod 755 "$RES/node/bin/node"
rm "$RES/sidecar/node_modules/playwright-core/package.json"
run_status rb_macos_check "$APP"
expect_status "a sidecar without playwright-core fails the structure test" 1
expect_output "the failure names it" "sidecar/node_modules/playwright-core/package.json"
printf '{}\n' > "$RES/sidecar/node_modules/playwright-core/package.json"

rm "$APP/Contents/Frameworks/rbrowser Helper (Plugin).app/Contents/MacOS/rbrowser Helper (Plugin)"
run_status rb_macos_check "$APP"
expect_status "a bundle without a helper fails the structure test" 1
expect_output "the failure names the helper" "rbrowser Helper (Plugin)"

rm "$BUILD/rbrowser-helper"
run_status rb_macos_stage "$CEF" "$BUILD" "$SCRATCH/out2/rBrowser.app" "0.9.0"
expect_status "a build without the helper binary fails" 1
expect_output "the failure names the binary" "rbrowser-helper"

# The icon (scripts/lib/icns.py): the app's .ico's 256 px PNG as the 256 px and 128@2x entries.
run_status python3 -I "$ROOT/scripts/lib/icns.py" "$SCRATCH/rbrowser.icns" "$ROOT/crates/rbrowser/resources/rbrowser.ico"
expect_status "the icon is made from the app's .ico" 0
expect_output "it holds the 256 px entries" "ic08, ic13"
expect "it is an icns file" "icns" "$(head -c 4 "$SCRATCH/rbrowser.icns")"
printf 'no image' > "$SCRATCH/none.png"
run_status python3 -I "$ROOT/scripts/lib/icns.py" "$SCRATCH/none.icns" "$SCRATCH/none.png"
expect_status "an input without a usable PNG fails" 1

finish_tests macos-bundle
