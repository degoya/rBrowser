#!/usr/bin/env bash
#
# scripts/lib/macos-bundle.sh without a Mac (RB-090-15): rBrowser.app assembled from stand-in
# binaries and a stand-in CEF framework, its structure test, the Info.plists of the app and the
# five helpers, and the structure test failing on a bundle that lacks a part.
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
expect_status "the structure test passes" 0
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
