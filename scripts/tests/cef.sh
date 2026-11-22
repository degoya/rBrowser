#!/usr/bin/env bash
#
# scripts/lib/cef.sh without CEF: the archive check against a pin, and the app folder of the
# sandboxed layout assembled from a stand-in build output.
#
#   scripts/tests/cef.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=../lib/cef.sh
source "$ROOT/scripts/lib/cef.sh"

expect "the cache defaults to ~/.cache/rbrowser/cef" "$HOME/.cache/rbrowser/cef" \
    "$(env -u CEF_PATH -u XDG_CACHE_HOME bash -c "source '$ROOT/scripts/lib/cef.sh'; rb_cef_path")"
expect "CEF_PATH wins" "/x/cef" "$(CEF_PATH=/x/cef bash -c "source '$ROOT/scripts/lib/cef.sh'; rb_cef_path")"

# The archive check, against a stand-in archive and its own pin.
CACHE="$SCRATCH/cef"
mkdir -p "$CACHE/$RB_CEF_VERSION"
run_status rb_cef_verify "$CACHE"
expect_status "a cache without the archive fails" 1
run_status rb_cef_verify "$CACHE" --if-present
expect_status "a cache without the archive passes before the build" 0
printf 'stand-in archive' > "$CACHE/$RB_CEF_VERSION/$RB_CEF_ARCHIVE"
run_status rb_cef_verify "$CACHE"
expect_status "an archive that differs from the pin fails" 1
expect_output "the failure names the pin" "$RB_CEF_ARCHIVE_SHA256"
RB_CEF_ARCHIVE_SHA256="$(sha256sum "$CACHE/$RB_CEF_VERSION/$RB_CEF_ARCHIVE" | cut -d ' ' -f 1)"
run_status rb_cef_verify "$CACHE"
expect_status "the pinned archive passes" 0

# The macOS archives (RB-090-15): each architecture against its own pin.
run_status rb_cef_verify_macos "$CACHE" aarch64
expect_status "a cache without the macOS archive fails" 1
run_status rb_cef_verify_macos "$CACHE" x86_64 --if-present
expect_status "a cache without the macOS archive passes before the build" 0
run_status rb_cef_verify_macos "$CACHE" ppc
expect_status "an unknown architecture has no pin" 2
printf 'stand-in arm64 archive' > "$CACHE/$RB_CEF_VERSION/$RB_CEF_ARCHIVE_MACOS_ARM64"
run_status rb_cef_verify_macos "$CACHE" aarch64
expect_status "a macOS archive that differs from the pin fails" 1
expect_output "the failure names the arm64 pin" "$RB_CEF_ARCHIVE_MACOS_ARM64_SHA256"
RB_CEF_ARCHIVE_MACOS_ARM64_SHA256="$(rb_sha256 "$CACHE/$RB_CEF_VERSION/$RB_CEF_ARCHIVE_MACOS_ARM64")"
run_status rb_cef_verify_macos "$CACHE" aarch64
expect_status "the pinned macOS archive passes" 0
expect "the extracted distribution's folder" "$CACHE/$RB_CEF_VERSION/cef_macos_x86_64" \
    "$(rb_cef_macos_dir "$CACHE" x86_64)"
expect_true "both macOS archives are pinned to the CEF build" \
    '[[ $RB_CEF_ARCHIVE_MACOS_ARM64 == *"$RB_CEF_VERSION+"*_macosarm64_minimal.tar.bz2 && $RB_CEF_ARCHIVE_MACOS_X64 == *"$RB_CEF_VERSION+"*_macosx64_minimal.tar.bz2 && ${#RB_CEF_ARCHIVE_MACOS_X64_SHA256} -eq 64 ]]'
expect "the macOS archives are the Windows build's" "${RB_CEF_ARCHIVE%_windows64_minimal.tar.bz2}" \
    "${RB_CEF_ARCHIVE_MACOS_ARM64%_macosarm64_minimal.tar.bz2}"

# The app folder.
BUILD="$SCRATCH/build"
mkdir -p "$BUILD/locales"
for file in bootstrap.exe bootstrapc.exe rbrowser.dll rbrowser.dll.lib libcef.dll libcef.lib \
    chrome_elf.dll resources.pak icudtl.dat v8_context_snapshot.bin vk_swiftshader_icd.json \
    archive.json rbrowser-nosandbox.exe locales/en-US.pak; do
    printf '%s' "$file" > "$BUILD/$file"
done
UI="$SCRATCH/ui-dist"
mkdir -p "$UI/assets"
printf '<!doctype html>' > "$UI/index.html"
printf 'export {}' > "$UI/assets/app.js"
APP="$SCRATCH/app"
run_status rb_stage_app "$BUILD" "$APP" "$UI"
expect_status "the app folder is assembled" 0
expect "rbrowser.exe is CEF's bootstrap" "bootstrap.exe" "$(cat "$APP/rbrowser.exe")"
expect_true "the DLL and the runtime are beside it" \
    '[[ -f $APP/rbrowser.dll && -f $APP/libcef.dll && -f $APP/resources.pak && -f $APP/icudtl.dat && -f $APP/v8_context_snapshot.bin && -f $APP/vk_swiftshader_icd.json && -f $APP/locales/en-US.pak ]]'
expect_true "the shell is ui\\" '[[ -f $APP/ui/index.html && -f $APP/ui/assets/app.js ]]'
expect_true "no build leftovers, no second executable" \
    '[[ ! -e $APP/bootstrap.exe && ! -e $APP/bootstrapc.exe && ! -e $APP/rbrowser-nosandbox.exe && ! -e $APP/libcef.lib && ! -e $APP/rbrowser.dll.lib && ! -e $APP/archive.json ]]'

rm "$UI/index.html"
run_status rb_stage_app "$BUILD" "$APP" "$UI"
expect_status "a second run updates the folder" 0
expect_true "without a shell build the folder has no ui\\" '[[ ! -e $APP/ui ]]'

# A build output without the runtime (the build script's copy went to another profile): the
# runtime comes from the extracted distribution in the CEF cache, rbrowser.dll from the build.
DIST="$(rb_cef_windows_dir "$SCRATCH/cef-cache")"
mkdir -p "$DIST/locales" "$SCRATCH/bare"
for file in bootstrap.exe libcef.dll chrome_elf.dll resources.pak icudtl.dat locales/de.pak; do
    printf 'dist %s' "$file" > "$DIST/$file"
done
printf 'rbrowser.dll' > "$SCRATCH/bare/rbrowser.dll"
CEF_PATH="$SCRATCH/cef-cache" run_status rb_stage_app "$SCRATCH/bare" "$SCRATCH/app3"
expect_status "a build without the runtime takes it from the CEF cache" 0
expect "rbrowser.exe is the distribution's bootstrap" "dist bootstrap.exe" "$(cat "$SCRATCH/app3/rbrowser.exe")"
expect_true "the DLL from the build, the runtime and locales from the distribution" \
    '[[ $(cat $SCRATCH/app3/rbrowser.dll) == rbrowser.dll && -f $SCRATCH/app3/libcef.dll && -f $SCRATCH/app3/locales/de.pak ]]'

rm "$BUILD/rbrowser.dll"
run_status rb_stage_app "$BUILD" "$SCRATCH/app2"
expect_status "a build without rbrowser.dll fails" 1
expect_output "the failure names the file" "rbrowser.dll"

# The Linux part (RB-097-05): the archive against its own pin, the app folder of the package.
LINUX_CACHE="$SCRATCH/cef-linux"
mkdir -p "$LINUX_CACHE/$RB_CEF_VERSION"
run_status rb_cef_verify_linux "$LINUX_CACHE"
expect_status "a cache without the Linux archive fails" 1
run_status rb_cef_verify_linux "$LINUX_CACHE" --if-present
expect_status "a cache without the Linux archive passes before the build" 0
printf 'stand-in linux archive' > "$LINUX_CACHE/$RB_CEF_VERSION/$RB_CEF_ARCHIVE_LINUX_X64"
run_status rb_cef_verify_linux "$LINUX_CACHE"
expect_status "a Linux archive that differs from the pin fails" 1
expect_output "the failure names the Linux pin" "$RB_CEF_ARCHIVE_LINUX_X64_SHA256"
linux_pin="$RB_CEF_ARCHIVE_LINUX_X64_SHA256"
RB_CEF_ARCHIVE_LINUX_X64_SHA256="$(rb_sha256 "$LINUX_CACHE/$RB_CEF_VERSION/$RB_CEF_ARCHIVE_LINUX_X64")"
run_status rb_cef_verify_linux "$LINUX_CACHE"
expect_status "the pinned Linux archive passes" 0
RB_CEF_ARCHIVE_LINUX_X64_SHA256="$linux_pin"
expect "the extracted Linux distribution's folder" "$LINUX_CACHE/$RB_CEF_VERSION/cef_linux_x86_64" \
    "$(rb_cef_linux_dir "$LINUX_CACHE")"
expect "the Linux archive is the Windows build's" "${RB_CEF_ARCHIVE%_windows64_minimal.tar.bz2}" \
    "${RB_CEF_ARCHIVE_LINUX_X64%_linux64_minimal.tar.bz2}"
expect_true "the Linux archive is pinned with a SHA-256" '[[ ${#RB_CEF_ARCHIVE_LINUX_X64_SHA256} -eq 64 ]]'

LBUILD="$SCRATCH/linux-build"
mkdir -p "$LBUILD/locales"
for file in rbrowser-nosandbox rbrowser-helper librbrowser.so librbrowser.rlib libcef.so \
    chrome-sandbox libvk_swiftshader.so libvulkan.so.1 resources.pak chrome_100_percent.pak \
    icudtl.dat v8_context_snapshot.bin vk_swiftshader_icd.json archive.json CMakeLists.txt \
    locales/en-US.pak; do
    printf '%s' "$file" > "$LBUILD/$file"
done
chmod 700 "$LBUILD/chrome-sandbox"
UI="$SCRATCH/ui-dist-linux"
mkdir -p "$UI"
printf '<!doctype html>' > "$UI/index.html"
LAPP="$SCRATCH/linux-app"
CEF_PATH="$SCRATCH/no-cache" run_status rb_stage_app_linux "$LBUILD" "$LAPP" "$UI"
expect_status "the Linux app folder is assembled" 0
expect "rbrowser is the build's executable" "rbrowser-nosandbox" "$(cat "$LAPP/rbrowser")"
expect_true "the runtime and the locales are beside it" \
    '[[ -f $LAPP/libcef.so && -f $LAPP/libvk_swiftshader.so && -f $LAPP/libvulkan.so.1 && -f $LAPP/resources.pak && -f $LAPP/chrome_100_percent.pak && -f $LAPP/icudtl.dat && -f $LAPP/v8_context_snapshot.bin && -f $LAPP/vk_swiftshader_icd.json && -f $LAPP/locales/en-US.pak ]]'
expect_true "the program and the sandbox helper are executable, the helper not setuid" \
    '[[ -x $LAPP/rbrowser && $(stat -c %a "$LAPP/chrome-sandbox") == 755 ]]'
expect_true "the shell is ui/" '[[ -f $LAPP/ui/index.html ]]'
expect_true "no build leftovers, no helper app, no library of ours" \
    '[[ ! -e $LAPP/rbrowser-nosandbox && ! -e $LAPP/rbrowser-helper && ! -e $LAPP/librbrowser.so && ! -e $LAPP/librbrowser.rlib && ! -e $LAPP/archive.json && ! -e $LAPP/CMakeLists.txt ]]'

LDIST="$(rb_cef_linux_dir "$SCRATCH/cef-linux-cache")"
mkdir -p "$LDIST/locales" "$SCRATCH/linux-bare"
for file in libcef.so chrome-sandbox resources.pak locales/de.pak; do
    printf 'dist %s' "$file" > "$LDIST/$file"
done
printf 'rbrowser-nosandbox' > "$SCRATCH/linux-bare/rbrowser-nosandbox"
CEF_PATH="$SCRATCH/cef-linux-cache" run_status rb_stage_app_linux "$SCRATCH/linux-bare" "$SCRATCH/linux-app2"
expect_status "a Linux build without the runtime takes it from the CEF cache" 0
expect_true "the executable from the build, the runtime from the distribution" \
    '[[ $(cat $SCRATCH/linux-app2/rbrowser) == rbrowser-nosandbox && $(cat $SCRATCH/linux-app2/libcef.so) == "dist libcef.so" && -f $SCRATCH/linux-app2/locales/de.pak && ! -e $SCRATCH/linux-app2/ui ]]'

rm "$LBUILD/rbrowser-nosandbox"
run_status rb_stage_app_linux "$LBUILD" "$SCRATCH/linux-app3"
expect_status "a Linux build without the executable fails" 1
expect_output "the failure names the executable" "rbrowser-nosandbox"

finish_tests cef
