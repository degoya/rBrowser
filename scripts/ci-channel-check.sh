#!/usr/bin/env bash
#
# One check of package-channels.yml (RB-090-13, RB-110-04; after rCommander's
# scripts/ci-channel-check.sh, RC-100-52), after a package manager installed or upgraded rBrowser:
# the `rbrowser` on PATH and the console twin rbrowser.com in the program folder both answer
# `--version` with the expected version, the folder holds what the ZIP ships beside the program
# (rbrowser.exe, rbrowser.com, plugins/), and the installation is the manager's as the update check
# sees it: rb-update reads the `install-kind` marker beside the program (crates/rb-update), which
# the Scoop manifest writes as `scoop`. rBrowser has no `install-kind` subcommand, so the marker
# file is read directly. winget cannot write a marker (a path rule in install_kind is still open),
# and Homebrew has no package until RB-110-01/02: for those the marker is reported, not required.
# CI only; Git Bash on Windows, where rbrowser.exe is a GUI program whose output a pipe carries
# and rbrowser.com its console twin.
#
#   scripts/ci-channel-check.sh <scoop|winget|homebrew> <tag or version>
#
# The program folder: `scoop prefix rbrowser`; the one degoya.rBrowser_* package folder under
# %LOCALAPPDATA%\Microsoft\WinGet\Packages and the ZIP's folder in it; the folder of the
# `rbrowser` on PATH for Homebrew. RB_PROGRAM_DIR names it instead (the script's test).
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: scripts/ci-channel-check.sh <scoop|winget|homebrew> <version>" >&2; exit 2; }
kind="$1"
version="${2#v}"
case "$kind" in
    scoop | winget | homebrew) ;;
    *) echo "unknown channel: $kind (scoop, winget, homebrew)" >&2; exit 2 ;;
esac
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] \
    || { echo "not a release version: $2" >&2; exit 2; }

# On stderr, so that a failure inside $(program_dir) is not taken for the folder.
fail() { echo "::error::$*" >&2; exit 1; }

program_dir() {
    local -a found
    if [[ -n "${RB_PROGRAM_DIR:-}" ]]; then
        printf '%s\n' "$RB_PROGRAM_DIR"
        return
    fi
    case "$kind" in
        scoop) cygpath -u "$(scoop prefix rbrowser | tr -d '\r')" ;;
        winget)
            found=("$(cygpath -u "$LOCALAPPDATA")"/Microsoft/WinGet/Packages/degoya.rBrowser_*)
            [[ ${#found[@]} -eq 1 && -d "${found[0]}" ]] \
                || fail "expected one degoya.rBrowser_* package folder, found ${found[*]}"
            printf '%s\n' "${found[0]}/rbrowser-${version}-windows-x64"
            ;;
        homebrew) dirname "$(command -v rbrowser)" ;;
    esac
}

path_rbrowser="$(command -v rbrowser)" || fail "no rbrowser on PATH after the ${kind} install"
echo "rbrowser on PATH: ${path_rbrowser}"
said="$(rbrowser --version | tr -d '\r')"
echo "rbrowser --version: ${said}"
[[ "$said" == *"$version"* ]] || fail "rbrowser --version is not ${version}"

dir="$(program_dir)"
echo "program folder: ${dir}"
[[ -d "$dir" ]] || fail "no program folder at ${dir}"
if [[ "$kind" != homebrew ]]; then
    for entry in rbrowser.exe rbrowser.com plugins; do
        [[ -e "$dir/$entry" ]] || fail "no ${entry} in ${dir}"
    done
    said="$("$dir/rbrowser.com" --version | tr -d '\r')"
    echo "rbrowser.com --version: ${said}"
    [[ "$said" == *"$version"* ]] || fail "rbrowser.com --version is not ${version}"
fi

marker="$dir/install-kind"
if [[ -f "$marker" ]]; then
    recorded="$(sed -n '/^[[:space:]]*[^#[:space:]]/{p;q}' "$marker" | tr -d '\r[:space:]')"
    echo "install-kind: ${recorded}"
    [[ "$recorded" == "$kind" ]] || fail "the update check takes this installation for '${recorded}', not ${kind}"
elif [[ "$kind" == scoop ]]; then
    fail "no install-kind marker in ${dir}: the update check would offer a self-update"
else
    echo "::notice::no install-kind marker for ${kind}; rb-update cannot tell this installation yet"
fi
echo "the ${kind} installation of ${version} checks out"
