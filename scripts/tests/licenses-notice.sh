#!/usr/bin/env bash
#
# scripts/lib/licenses_notice.py (RB-099-12): the three NOTICE files of the packages
# (packaging/windows|linux|macos/NOTICE) are one source, packaging/NOTICE.in, rendered per
# platform. Against a fixture: a line without a tag is in every file, `[windows]` and
# `[linux,macos]` lines only in theirs, a `#` line in none; --check passes on current files and
# finds one edited by hand; an unknown platform stops it. The CEF placeholders (RB-099-13) are
# filled from the CEF pin in scripts/lib/tool-versions.sh, so a CEF update leaves the NOTICE
# files stale until they are rendered again; pins of two builds, a pin that does not read, no pin
# and an unknown placeholder stop the run. Then the repository's own: the three files are the
# rendering of the one source, so they say the same, and each names THIRD-PARTY-LICENSES.txt,
# the copyright holder, the pinned CEF and Chromium revisions and the written source offer.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/licenses-notice.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
WORKER="$ROOT/scripts/lib/licenses_notice.py"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

repo="$SCRATCH/repo"
mkdir -p "$repo/packaging"
cat > "$repo/packaging/NOTICE.in" << 'EOF'
# A comment for the maintainers only.
Everywhere
[windows] Only on Windows: rbrowser.exe
[linux,macos] On Linux and macOS: rbrowser
[macos]
  Everywhere, indented
EOF

run_status python3 -B "$WORKER" "$repo"
expect_status "the three files are written" 0
expect "Windows" $'Everywhere\nOnly on Windows: rbrowser.exe\n  Everywhere, indented' "$(cat "$repo/packaging/windows/NOTICE")"
expect "Linux" $'Everywhere\nOn Linux and macOS: rbrowser\n  Everywhere, indented' "$(cat "$repo/packaging/linux/NOTICE")"
expect "macOS, with its empty line" $'Everywhere\nOn Linux and macOS: rbrowser\n\n  Everywhere, indented' \
    "$(cat "$repo/packaging/macos/NOTICE")"

run_status python3 -B "$WORKER" "$repo" --check
expect_status "--check passes on current files" 0
echo "edited by hand" >> "$repo/packaging/linux/NOTICE"
run_status python3 -B "$WORKER" "$repo" --check
expect_status "--check finds a file edited by hand" 1
expect_output "and names it" "packaging/linux/NOTICE is stale"

echo "[solaris] nowhere" >> "$repo/packaging/NOTICE.in"
run_status python3 -B "$WORKER" "$repo"
expect_status "an unknown platform stops the run" 2
expect_output "and names the line" "NOTICE.in:7: unknown platform 'solaris'"

# The CEF placeholders, from the pin (RB-099-13).
pinned="$SCRATCH/pinned"
pins="$pinned/scripts/lib/tool-versions.sh"
mkdir -p "$pinned/packaging" "$pinned/scripts/lib"
{
    echo 'RB_CEF_VERSION="150.1.2"'
    echo 'RB_CEF_ARCHIVE="cef_binary_150.1.2+gabc1234+chromium-150.0.7000.42_windows64_minimal.tar.bz2"'
    echo 'RB_CEF_ARCHIVE_SHA256="c691636862b5f6bab05e76d77bd1b57799659552a25a5bc8b7eaa365de6e63fd"'
    echo 'RB_CEF_ARCHIVE_LINUX_X64="cef_binary_150.1.2+gabc1234+chromium-150.0.7000.42_linux64_minimal.tar.bz2"'
} > "$pins"
printf '%s\n' 'CEF @CEF_VERSION@ (@CEF_COMMIT@), Chromium @CHROMIUM_VERSION@' 'axe-core@4.14.1 <a@b.c>' \
    > "$pinned/packaging/NOTICE.in"
run_status python3 -B "$WORKER" "$pinned"
expect_status "the placeholders are filled" 0
expect "from the pin" $'CEF 150.1.2 (abc1234), Chromium 150.0.7000.42\naxe-core@4.14.1 <a@b.c>' \
    "$(cat "$pinned/packaging/windows/NOTICE")"
sed -i 's/150\.1\.2/150.1.3/g; s/gabc1234/gdef5678/g' "$pins"
run_status python3 -B "$WORKER" "$pinned" --check
expect_status "a new CEF pin leaves the NOTICE files stale" 1
echo 'RB_CEF_ARCHIVE_MACOS_X64="cef_binary_150.1.2+gabc1234+chromium-150.0.7000.42_macosx64_minimal.tar.bz2"' >> "$pins"
run_status python3 -B "$WORKER" "$pinned"
expect_status "pins of two CEF builds stop the run" 2
expect_output "and name it" "two CEF builds"
echo 'RB_CEF_ARCHIVE="cef_binary_150.1.2.tar.bz2"' > "$pins"
run_status python3 -B "$WORKER" "$pinned"
expect_status "a pin that does not read stops the run" 2
rm "$pins"
run_status python3 -B "$WORKER" "$pinned"
expect_status "no pin at all stops the run" 2
echo 'Built @BUILD_DATE@' > "$pinned/packaging/NOTICE.in"
run_status python3 -B "$WORKER" "$pinned"
expect_status "an unknown placeholder stops the run" 2
expect_output "and names it" "unknown placeholder @BUILD_DATE@"

# The repository's own NOTICE files.
run_status python3 -B "$WORKER" "$ROOT" --check
expect_status "the three NOTICE files are the rendering of packaging/NOTICE.in" 0
# shellcheck source=../lib/tool-versions.sh
source "$ROOT/scripts/lib/tool-versions.sh"
chromium="${RB_CEF_ARCHIVE#*+chromium-}"
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
chromium="${chromium%%_*}"
commit="${RB_CEF_ARCHIVE#*+g}"
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
commit="${commit%%+*}"
for platform in windows linux macos; do
    # shellcheck disable=SC2034  # read by the conditions expect_true evaluates
    notice="$(cat "$ROOT/packaging/$platform/NOTICE")"
    expect_true "$platform: one copyright holder" \
        '[[ "$notice" == *"Copyright (c) 2026 Alexander Herling / DEGOYA.digital"* ]]'
    expect_true "$platform: points to THIRD-PARTY-LICENSES.txt" '[[ "$notice" == *"THIRD-PARTY-LICENSES.txt"* ]]'
    expect_true "$platform: the plugins' third-party code" \
        '[[ "$notice" == *"axe-core"* && "$notice" == *"Noto Sans Mono"* && "$notice" == *"Lucide"* ]]'
    expect_true "$platform: the DevTools backends" \
        '[[ "$notice" == *"React DevTools"* && "$notice" == *"Vue DevTools"* && "$notice" == *"birpc 2.9.0"* ]]'
    expect_true "$platform: the pinned CEF and Chromium revisions (RB-099-13)" \
        '[[ "$notice" == *"CEF $RB_CEF_VERSION, commit $commit"* && "$notice" == *"Chromium $chromium"* ]]'
    expect_true "$platform: the written offer of the LGPL source, three years, with the contact" \
        '[[ "$notice" == *"Written offer"* && "$notice" == *"three years"* && "$notice" == *"https://degoya.digital/impressum"* ]]'
    expect_true "$platform: no placeholder left" '[[ "$notice" != *"@CEF_"* && "$notice" != *"@CHROMIUM_"* ]]'
done
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
windows="$(cat "$ROOT/packaging/windows/NOTICE")"
expect_true "windows: adb among the downloads on request, under Google's terms" \
    '[[ "$windows" == *"Android SDK Platform-Tools"* && "$windows" == *"Android Software Development Kit License Agreement"* ]]'

finish_tests licenses-notice
