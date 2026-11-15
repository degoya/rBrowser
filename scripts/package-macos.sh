#!/usr/bin/env bash
#
# Builds the macOS test build on a Mac - the macos-15 (Apple Silicon) and macos-15-intel runners
# of ci.yml; nothing builds it locally (RB-090-15) - and packs it:
#   artifacts/macos/rbrowser-<version>-macos-<aarch64|x86_64>.zip   (ditto, the app at the top)
#   artifacts/macos/rbrowser-<version>-macos-<aarch64|x86_64>.dmg   (hdiutil, with /Applications)
# holding rBrowser.app (scripts/lib/macos-bundle.sh):
#   Contents/MacOS/rbrowser, Contents/Frameworks/Chromium Embedded Framework.framework and the
#   five helper apps, rbrowser Helper (GPU|Renderer|Plugin|Alerts).app (the crate's
#   rbrowser-helper), each with its Info.plist;
#   Contents/Resources/ ui/ (the shell, built here), rbrowser.icns, plugins/ (the bundled
#   plugins), README.txt (start, Gatekeeper), LICENSE, NOTICE, CHANGELOG.md, VERSION.txt and
#   licenses/ (Chromium's credits, CEF's licence);
# signed ad hoc (`codesign --sign -`, inside out: the framework's libraries, the framework, the
# helpers, the app), not notarised: Gatekeeper asks once (README.txt, the wiki's installation
# page); Developer ID signing and notarisation are RB-110-06. The structure test
# (rb_macos_check) and `codesign --verify --deep --strict` run before anything is packed.
#
# Not in the test build yet: the Node runtime and the sidecar (WebKit, flows, Lighthouse), the
# CEF sandbox of the helpers. One bundle per architecture: CEF ships one distribution per
# architecture, so a universal app would be twice the size (docs/adr/0001-cef-and-engines.md).
#
# Usage:
#   scripts/package-macos.sh                          # the release profile, this Mac's architecture
#   scripts/package-macos.sh --arch x86_64            # for Intel Macs (arm64 = aarch64)
#   scripts/package-macos.sh --profile release-test   # a test package, built much faster
#   scripts/package-macos.sh --plugins <dir>          # the packages in <dir> (relative to the
#                                                     # checkout), not built here
#   scripts/package-macos.sh --allow-dirty            # a dirty tree, for a local test only
#
# The version is held by scripts/set-version.sh --check first; a dirty tree, or a package of the
# same version from another commit, stops the run before anything is built (scripts/lib/package.sh;
# RB_REPACK=1 replaces it, --allow-dirty takes a dirty tree).
#
# Takes the build lock (scripts/lib/lock.sh) - never wrap it in flock. Parallelism:
# scripts/lib/jobs.sh (RB_JOBS).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/jobs.sh
source "$ROOT/scripts/lib/jobs.sh"
# Sourced before the `cd`, because the lock library resolves this script's own path from $0.
# shellcheck source=lib/lock.sh
source "$ROOT/scripts/lib/lock.sh"
rb_take_lock "$@"
# shellcheck source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"
# shellcheck source=lib/cef.sh
source "$ROOT/scripts/lib/cef.sh"
# shellcheck source=lib/package-stage.sh
source "$ROOT/scripts/lib/package-stage.sh"
# shellcheck source=lib/macos-bundle.sh
source "$ROOT/scripts/lib/macos-bundle.sh"
cd "$ROOT"

profile=release
arch=""
prebuilt=""
allow_dirty=0
usage="usage: scripts/package-macos.sh [--profile release|release-test] [--arch aarch64|arm64|x86_64] [--plugins <dir>] [--allow-dirty]"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile | --arch)
            [[ $# -ge 2 ]] || { echo "$usage" >&2; exit 2; }
            if [[ "$1" == --profile ]]; then profile="$2"; else arch="$2"; fi
            shift 2
            ;;
        --plugins)
            [[ $# -ge 2 && -d "$2" ]] || { echo "$usage (an existing directory)" >&2; exit 2; }
            prebuilt="$(cd "$2" && pwd)"
            shift 2
            ;;
        --allow-dirty) allow_dirty=1; shift ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
case "$profile" in release | release-test) ;; *) echo "!! unknown profile '$profile'" >&2; exit 2 ;; esac
[[ -n "$arch" ]] || arch="$(uname -m)"
case "$arch" in
    arm64 | aarch64) arch=aarch64 ;;
    x86_64) ;;
    *) echo "!! unknown arch '$arch' (aarch64, arm64 or x86_64)" >&2; exit 2 ;;
esac
TARGET="$arch-apple-darwin"
for tool in codesign ditto hdiutil; do
    command -v "$tool" > /dev/null || { echo "!! $tool is missing: this script runs on macOS" >&2; exit 1; }
done

version="$(rb_package_version "$ROOT")"
OUT="$ROOT/artifacts/macos"
NAME="rbrowser-$version-macos-$arch"
STAGE="$OUT/$NAME"
APP="$STAGE/rBrowser.app"
ZIP="$OUT/$NAME.zip"
DMG="$OUT/$NAME.dmg"
echo "==> packaging rBrowser $version for $TARGET (profile: $profile, jobs: $JOBS) into $ZIP"

# The commit and the time, exported so the binary and VERSION.txt name the same ones (build.rs
# takes them verbatim). An existing package of this version from another build stops the run.
commit="$(rb_package_commit)"
rb_package_clean "$commit" "$allow_dirty"
rb_package_guard "$ZIP" "rBrowser.app/Contents/Resources/VERSION.txt" "$commit"
RB_BUILD_COMMIT="$commit"
RB_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
export RB_BUILD_COMMIT RB_BUILD_TIME

# The bundled plugins, as the Windows package has them.
plugins=()
if [[ -n "$prebuilt" || -x scripts/build-plugins.sh ]]; then
    plugin_list="$(rb_package_plugins "$ROOT" "$prebuilt")"
    while IFS= read -r package; do [[ -z "$package" ]] || plugins+=("$package"); done <<< "$plugin_list"
fi

# The shell and the sync agent (embedded by build.rs), with the node .nvmrc pins.
rb_use_pinned_node "$ROOT"
for dir in ui agent; do rb_build_frontend "$ROOT/$dir"; done
for built in ui/dist/index.html agent/dist/rb-agent.js agent/dist/rb-page-api.js; do
    [[ -s "$built" ]] || { echo "!! $built is missing after the build" >&2; exit 1; }
done

CEF_PATH="$(rb_cef_path)"
export CEF_PATH
rb_cef_verify_macos "$CEF_PATH" "$arch" --if-present
# CEF's own deployment target, so the app, the helpers and CEF's wrapper agree.
export MACOSX_DEPLOYMENT_TARGET="$RB_MACOS_MINIMUM_SYSTEM"
echo "==> building rBrowser (CEF in $CEF_PATH)"
CARGO_BUILD_JOBS="$JOBS" cargo build --locked --profile "$profile" --target "$TARGET" \
    -j "$JOBS" -p rbrowser --bin rbrowser-nosandbox --bin rbrowser-helper
rb_cef_verify_macos "$CEF_PATH" "$arch"
cef_dir="$(rb_cef_macos_dir "$CEF_PATH" "$arch")"

echo "==> assembling $APP"
rm -rf "$STAGE" "$ZIP" "$DMG"
mkdir -p "$STAGE"
rb_macos_stage "$cef_dir" "${CARGO_TARGET_DIR:-$ROOT/target}/$TARGET/$profile" "$APP" "$version" \
    "$ROOT/ui/dist"
resources="$APP/Contents/Resources"
python3 -I scripts/lib/icns.py "$resources/rbrowser.icns" crates/rbrowser/resources/rbrowser.ico
mkdir -p "$resources/licenses"
[[ -f "$cef_dir/CREDITS.html" ]] || { echo "!! no $cef_dir/CREDITS.html: the licences cannot be shipped" >&2; exit 1; }
install -m 644 "$cef_dir/CREDITS.html" "$resources/licenses/chromium-credits.html"
# The cef crate extracts no LICENSE.txt; CEF's own licence comes from the archive, checked
# against its pin above.
cef_pin="$(rb_cef_macos_pin "$arch")"
cef_archive="${cef_pin% *}"
if [[ -f "$cef_dir/LICENSE.txt" ]]; then
    install -m 644 "$cef_dir/LICENSE.txt" "$resources/licenses/cef-license.txt"
elif ! tar -xjOf "$CEF_PATH/$RB_CEF_VERSION/$cef_archive" "${cef_archive%.tar.bz2}/LICENSE.txt" \
    > "$resources/licenses/cef-license.txt" || [[ ! -s "$resources/licenses/cef-license.txt" ]]; then
    echo "!! no LICENSE.txt in $cef_dir or $cef_archive: the licences cannot be shipped" >&2
    exit 1
fi
install -m 644 packaging/macos/README.txt packaging/macos/NOTICE LICENSE CHANGELOG.md "$resources/"
if ((${#plugins[@]})); then
    mkdir -p "$resources/plugins"
    install -m 644 "${plugins[@]}" "$resources/plugins/"
fi
cef_build="${cef_archive#cef_binary_}"
printf '%s\n' "rBrowser $version" "commit $RB_BUILD_COMMIT" "built $RB_BUILD_TIME" \
    "target $TARGET, profile $profile" "cef ${cef_build%_macos*}" \
    > "$resources/VERSION.txt"

echo "==> checking the bundle"
rb_macos_check "$APP"
rb_package_unportable "$APP"
while IFS= read -r -d '' plist; do
    plutil -lint "$plist"
done < <(find "$APP" -path '*/Contents/Info.plist' -print0)

# Inside out, as codesign wants nested code signed before what contains it.
echo "==> signing ad hoc"
framework="$APP/Contents/Frameworks/$RB_MACOS_FRAMEWORK"
while IFS= read -r -d '' library; do
    codesign --force --sign - "$library"
done < <(find "$framework/Libraries" -name '*.dylib' -print0)
codesign --force --sign - "$framework"
for suffix in "${RB_MACOS_HELPERS[@]}"; do
    codesign --force --sign - "$APP/Contents/Frameworks/$RB_MACOS_EXECUTABLE Helper$suffix.app"
done
codesign --force --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

echo "==> writing $ZIP and $DMG"
(cd "$STAGE" && ditto -c -k --keepParent rBrowser.app "$ZIP")
ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname "rBrowser $version" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
du -sh "$APP" "$ZIP" "$DMG"
rm -rf "$STAGE"
echo "==> done"
