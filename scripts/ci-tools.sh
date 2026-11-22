#!/usr/bin/env bash
#
# The tools the GitHub workflows install beyond the runner image (RB-090-04, adapted from
# rCommander's and rDownloader's script of the same name). Linux x86_64 only.
#
#   scripts/ci-tools.sh linters [--dest DIR]   # shellcheck, gitleaks, actionlint at their pins
#   scripts/ci-tools.sh cross                  # the packages of docs/development.md for the
#                                              # cargo-xwin and musl builds (runner only)
#   scripts/ci-tools.sh linux                  # the native Linux build's (RB-097-05): CEF's
#                                              # run-time libraries, Xvfb, strip and
#                                              # rsvg-convert for the packages (runner only)
#
# `linters` checks each download against its SHA-256 from scripts/lib/tool-versions.sh before it
# is unpacked, so `scripts/check.sh --preflight` runs those stages instead of skipping them. On a
# runner the directory is $RUNNER_TEMP/ci-bin and goes on GITHUB_PATH; elsewhere `--dest` names
# it (nothing is put on PATH). `cross` installs with apt and puts LLVM's own bin/ (clang-cl,
# lld-link, llvm-lib, which Ubuntu does not link into /usr/bin) on GITHUB_PATH.
set -euo pipefail

# shellcheck source=lib/tool-versions.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/tool-versions.sh"

usage() { echo "usage: scripts/ci-tools.sh linters [--dest DIR] | cross | linux" >&2; exit 2; }

# Downloads URL $1 to file $2 and holds it to SHA-256 $3.
fetch() {
    curl -fsSL --retry 3 --max-time 300 -o "$2" "$1"
    echo "$3  $2" | sha256sum --check --strict
}

linters() {
    local bin="$1" archive
    mkdir -p "$bin"
    archive="$bin/shellcheck.tar.xz"
    fetch "https://github.com/koalaman/shellcheck/releases/download/v${RB_SHELLCHECK_VERSION}/shellcheck-v${RB_SHELLCHECK_VERSION}.linux.x86_64.tar.xz" \
        "$archive" "$RB_SHELLCHECK_SHA256"
    tar -xJf "$archive" -C "$bin" --strip-components=1 "shellcheck-v${RB_SHELLCHECK_VERSION}/shellcheck"
    archive="$bin/gitleaks.tar.gz"
    fetch "https://github.com/gitleaks/gitleaks/releases/download/v${RB_GITLEAKS_VERSION}/gitleaks_${RB_GITLEAKS_VERSION}_linux_x64.tar.gz" \
        "$archive" "$RB_GITLEAKS_SHA256"
    tar -xzf "$archive" -C "$bin" gitleaks
    archive="$bin/actionlint.tar.gz"
    fetch "https://github.com/rhysd/actionlint/releases/download/v${RB_ACTIONLINT_VERSION}/actionlint_${RB_ACTIONLINT_VERSION}_linux_amd64.tar.gz" \
        "$archive" "$RB_ACTIONLINT_SHA256"
    tar -xzf "$archive" -C "$bin" actionlint
    rm -f "$bin"/*.tar.*
}

# The apt packages of docs/development.md ("Setup") that a cross build needs, bounded so a stalled
# mirror costs minutes, not the job.
cross() {
    local apt_opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
    timeout 300 sudo apt-get "${apt_opts[@]}" update -q
    # MSVC's STL (cargo-xwin's CRT) refuses a Clang older than 19 (STL1000); ubuntu-24.04's
    # default `clang` is 18, so the versioned packages, their bin/ first on PATH.
    local llvm=19
    timeout 600 sudo apt-get "${apt_opts[@]}" install -y -q --no-install-recommends \
        build-essential "clang-$llvm" cmake "lld-$llvm" "llvm-$llvm" nasm ninja-build pkg-config \
        wine wine64
    # rcedit (rb_patch_exe) writes the icon and version into CEF's bootstrap.exe under Wine.
    if ! command -v wine > /dev/null 2>&1 && command -v wine64 > /dev/null 2>&1; then
        sudo ln -s "$(command -v wine64)" /usr/local/bin/wine
    fi
    echo "/usr/lib/llvm-$llvm/bin" >> "${GITHUB_PATH:?set by the runner}"
}

# The libraries CEF's libcef.so needs (its NEEDED entries, the deb's dependencies in
# packaging/linux/nfpm.yaml.in under their ubuntu-24.04 names): the tests' binaries link it, the
# package smoke starts it. Xvfb for a start without a display, binutils' strip and librsvg's
# rsvg-convert for scripts/package-linux.sh and scripts/package-deb-rpm.sh.
linux() {
    local apt_opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
    timeout 300 sudo apt-get "${apt_opts[@]}" update -q
    timeout 600 sudo apt-get "${apt_opts[@]}" install -y -q --no-install-recommends \
        libnss3 libnspr4 libatk1.0-0t64 libatk-bridge2.0-0t64 libatspi2.0-0t64 libcups2t64 \
        libdbus-1-3 libx11-6 libxcb1 libxcomposite1 libxdamage1 libxext6 libxfixes3 libxrandr2 \
        libxkbcommon0 libgbm1 libexpat1 libcairo2 libpango-1.0-0 libudev1 libasound2t64 \
        libglib2.0-0t64 xvfb xauth binutils librsvg2-bin
}

case "${1:-}" in
    linters) shift ;;
    cross) [[ $# -eq 1 ]] || usage; cross; exit 0 ;;
    linux) [[ $# -eq 1 ]] || usage; linux; exit 0 ;;
    *) usage ;;
esac
dest=""
case "${1:-}" in
    --dest) dest="${2:?--dest needs a directory}" ;;
    "") ;;
    *) usage ;;
esac
if [[ -z "$dest" ]]; then
    dest="${RUNNER_TEMP:?not on a runner: name the directory with --dest}/ci-bin"
    linters "$dest"
    echo "$dest" >> "${GITHUB_PATH:?set by the runner}"
else
    linters "$dest"
fi
echo "    shellcheck $RB_SHELLCHECK_VERSION, gitleaks $RB_GITLEAKS_VERSION, actionlint $RB_ACTIONLINT_VERSION in $dest"
