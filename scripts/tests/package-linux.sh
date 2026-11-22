#!/usr/bin/env bash
#
# scripts/lib/package-linux.sh without the network (RB-097-05): the Node runtime staged from a
# stand-in linux-x64 tarball held to its pin, a pin for another version refused, the stage check
# naming what a tarball lacks; and scripts/package-deb-rpm.sh --render-only on a stand-in
# release tarball - the nfpm configs of both formats, the setuid sandbox helper, the dependencies.
#
#   scripts/tests/package-linux.sh
# shellcheck disable=SC2016,SC2034  # single-quoted on purpose: expect_true evaluates them (and reads DEB, RPM)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=../lib/package-linux.sh
source "$ROOT/scripts/lib/package-linux.sh"

VERSION="$RB_NODE_LINUX_X64_VERSION"
CHECKOUT="$SCRATCH/checkout"
mkdir -p "$CHECKOUT"
echo "v$VERSION" > "$CHECKOUT/.nvmrc"

# A stand-in of the official tarball in the cache: its own SHA-256 becomes the pin.
NAME="node-v$VERSION-linux-x64"
mkdir -p "$SCRATCH/src/$NAME/bin" "$SCRATCH/src/$NAME/include" "$SCRATCH/cache"
printf '#!/bin/sh\necho v%s\n' "$VERSION" > "$SCRATCH/src/$NAME/bin/node"
printf 'npm' > "$SCRATCH/src/$NAME/bin/npm"
printf 'Node.js is licensed for use as follows' > "$SCRATCH/src/$NAME/LICENSE"
printf 'header' > "$SCRATCH/src/$NAME/include/node.h"
tar -cJf "$SCRATCH/cache/$NAME.tar.xz" -C "$SCRATCH/src" "$NAME"
real_pin="$RB_NODE_LINUX_X64_SHA256"
RB_NODE_LINUX_X64_SHA256="$(sha256sum "$SCRATCH/cache/$NAME.tar.xz" | cut -d ' ' -f 1)"
run_status rb_stage_node_linux "$CHECKOUT" "$SCRATCH/cache" "$SCRATCH/stage"
expect_status "the pinned tarball is staged" 0
expect_true "node/bin/node and node/LICENSE, executable, nothing else" \
    '[[ -x $SCRATCH/stage/node/bin/node && -f $SCRATCH/stage/node/LICENSE && ! -e $SCRATCH/stage/node/bin/npm && ! -e $SCRATCH/stage/node/include ]]'
expect "the staged node is the tarball's" "v$VERSION" "$("$SCRATCH/stage/node/bin/node")"

RB_NODE_LINUX_X64_SHA256="$real_pin"
run_status rb_stage_node_linux "$CHECKOUT" "$SCRATCH/cache" "$SCRATCH/stage2"
expect_status "a cached tarball that is not the pinned one stops the run" 1
expect_output "the failure names the pin" "$real_pin"

echo "v1.2.3" > "$CHECKOUT/.nvmrc"
run_status rb_stage_node_linux "$CHECKOUT" "$SCRATCH/cache" "$SCRATCH/stage3"
expect_status "a pin for another version than .nvmrc's is refused" 1
expect_output "the refusal names the variable" "RB_NODE_LINUX_X64_VERSION"
expect_true "the Linux pin is the Windows pin's version" '[[ $RB_NODE_LINUX_X64_VERSION == "$RB_NODE_WIN_X64_VERSION" && ${#real_pin} -eq 64 ]]'

# The stage check.
STAGE="$SCRATCH/rbrowser-9.9.9-linux-x64"
mkdir -p "$STAGE/locales" "$STAGE/ui" "$STAGE/node/bin" "$STAGE/sidecar/dist" "$STAGE/licenses" \
    "$STAGE/plugins"
for path in rbrowser libcef.so libEGL.so libGLESv2.so chrome-sandbox icudtl.dat \
    v8_context_snapshot.bin locales/en-US.pak ui/index.html node/bin/node node/LICENSE sidecar/dist/main.js NOTICE \
    LICENSE licenses/chromium-credits.html crash_reporter.cfg; do
    printf '%s' "$path" > "$STAGE/$path"
done
printf 'rBrowser 9.9.9\ncommit abc\nbuilt now\ntarget x86_64-unknown-linux-gnu, profile release\n' \
    > "$STAGE/VERSION.txt"
run_status rb_check_linux_stage "$STAGE"
expect_status "a stage without executables fails" 1
expect_output "it says what must be executable" "must be executable"
chmod 755 "$STAGE/rbrowser" "$STAGE/node/bin/node" "$STAGE/chrome-sandbox"
run_status rb_check_linux_stage "$STAGE"
expect_status "a complete stage passes" 0
mv "$STAGE/libcef.so" "$SCRATCH/libcef.so"
rm "$STAGE/ui/index.html"
run_status rb_check_linux_stage "$STAGE"
expect_status "a stage without the CEF library and the shell fails" 1
expect_output "it names the library" "libcef.so"
expect_output "it names the shell" "ui/index.html"
mv "$SCRATCH/libcef.so" "$STAGE/libcef.so"
printf '<!doctype html>' > "$STAGE/ui/index.html"
mv "$STAGE/libGLESv2.so" "$SCRATCH/libGLESv2.so"
run_status rb_check_linux_stage "$STAGE"
expect_status "a stage without ANGLE's GL libraries fails" 1
expect_output "it names ANGLE's library" "libGLESv2.so"
mv "$SCRATCH/libGLESv2.so" "$STAGE/libGLESv2.so"

# scripts/package-deb-rpm.sh --render-only on the stand-in release tarball.
tar -C "$SCRATCH" -czf "$SCRATCH/rbrowser-9.9.9-linux-x64.tar.gz" "$(basename "$STAGE")"
run_status "$ROOT/scripts/package-deb-rpm.sh" --render-only "$SCRATCH/rbrowser-9.9.9-linux-x64.tar.gz" "$SCRATCH/out"
expect_status "the nfpm configs are rendered" 0
DEB="$SCRATCH/out/nfpm-deb-x64.yaml"
RPM="$SCRATCH/out/nfpm-rpm-x64.yaml"
expect_true "one config per format" '[[ -f $DEB && -f $RPM ]]'
expect_true "the version and the architecture are filled in" \
    'grep -q "^version: \"9.9.9\"$" "$DEB" && grep -q "^arch: \"amd64\"$" "$DEB"'
expect_true "no placeholder is left" '! grep -q "@[A-Z_]*@" "$DEB" "$RPM"'
expect_true "the program goes to /usr/lib/rbrowser, linked into /usr/bin" \
    'grep -q "dst: /usr/lib/rbrowser$" "$DEB" && grep -q "dst: /usr/bin/rbrowser$" "$DEB"'
expect_true "the sandbox helper is root's and setuid" \
    'grep -A3 "dst: /usr/lib/rbrowser/chrome-sandbox$" "$DEB" | grep -q "mode: 04755"'
expect_true "the tree holds the program folder, the helper comes on its own" \
    'grep -B1 "dst: /usr/lib/rbrowser/chrome-sandbox$" "$DEB" | grep -q "src: .*/helper/chrome-sandbox\"$" && grep -q "type: tree" "$DEB"'
expect_true "the deb depends on the CEF runtime's libraries" \
    'grep -q "libnss3" "$DEB" && grep -q "libgbm1" "$DEB" && grep -q "libasound2t64 | libasound2" "$DEB"'
expect_true "the rpm takes Fedora's names" \
    'grep -q "    - nss$" "$RPM" && grep -q "mesa-libgbm" "$RPM" && grep -q "alsa-lib" "$RPM"'
expect_true "a menu entry and the icon" \
    'grep -q "dst: /usr/share/applications/rbrowser.desktop" "$DEB" && grep -q "hicolor/scalable/apps/rbrowser.svg" "$DEB"'
expect_true "the install-kind marker names the format" \
    'grep -q "install-kind.deb\"$" "$DEB" && grep -q "install-kind.rpm\"$" "$RPM" && grep -q "dst: /usr/lib/rbrowser/install-kind$" "$DEB"'

run_status "$ROOT/scripts/package-deb-rpm.sh" --render-only "$SCRATCH/not-a-release.tar.gz" "$SCRATCH/out"
expect_status "a file not named like a release tarball is refused" 2

finish_tests package-linux
