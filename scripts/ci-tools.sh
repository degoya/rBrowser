#!/usr/bin/env bash
#
# The tools the GitHub workflows install beyond the runner image (RB-090-04, adapted from
# rCommander's and rDownloader's script of the same name). Linux x86_64 only.
#
#   scripts/ci-tools.sh linters [--dest DIR]   # shellcheck, gitleaks, actionlint at their pins
#   scripts/ci-tools.sh cross                  # the packages of docs/development.md for the
#                                              # cargo-xwin and musl builds (runner only)
#
# `linters` checks each download against its SHA-256 from scripts/lib/tool-versions.sh before it
# is unpacked, so `scripts/check.sh --preflight` runs those stages instead of skipping them. On a
# runner the directory is $RUNNER_TEMP/ci-bin and goes on GITHUB_PATH; elsewhere `--dest` names
# it (nothing is put on PATH). `cross` installs with apt and puts LLVM's own bin/ (clang-cl,
# lld-link, llvm-lib, which Ubuntu does not link into /usr/bin) on GITHUB_PATH.
set -euo pipefail

# shellcheck source=lib/tool-versions.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/tool-versions.sh"

usage() { echo "usage: scripts/ci-tools.sh linters [--dest DIR] | cross" >&2; exit 2; }

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
    timeout 600 sudo apt-get "${apt_opts[@]}" install -y -q --no-install-recommends \
        build-essential clang cmake lld llvm nasm ninja-build pkg-config
    dirname "$(readlink -f "$(command -v clang)")" >> "${GITHUB_PATH:?set by the runner}"
}

case "${1:-}" in
    linters) shift ;;
    cross) [[ $# -eq 1 ]] || usage; cross; exit 0 ;;
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
