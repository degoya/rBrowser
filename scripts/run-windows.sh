#!/usr/bin/env bash
#
# Builds rBrowser for Windows from WSL (cargo-xwin), assembles the app folder in the sandboxed
# layout of ADR 0001 (CEF's bootstrap.exe as rbrowser.exe, rbrowser.dll, the CEF runtime, ui/dist
# as ui\ - scripts/lib/cef.sh) in target/<triple>/<profile>/app, copies it to
# %LOCALAPPDATA%\rbrowser-dev\ and starts it there through WSL interop, detached. Windows never
# runs rBrowser from the WSL file system: the CEF sandbox cannot start its processes from there
# (and SmartScreen and file locks behave differently), hence the copy. The CEF archive is held to
# its SHA-256 pin before and after the build.
#
# With --package it builds nothing: it unpacks the package of scripts/package-windows.sh (the
# version's ZIP under artifacts/windows/, or the one named) into %LOCALAPPDATA%\rbrowser-package\
# - the folder of that package emptied first - and starts the rbrowser.exe in it, as a tester
# would (RB-010-15).
#
# Usage:
#   scripts/run-windows.sh                  # debug build
#   scripts/run-windows.sh --release        # release build
#   scripts/run-windows.sh [--release] -- <arguments for rbrowser.exe>
#   scripts/run-windows.sh [--release] --build-only   # build and assemble only (run-windows.ps1)
#   scripts/run-windows.sh --package [<zip>] [-- <arguments>]   # start the unpacked package
#
# When C: is mounted read-only in WSL the copy cannot happen here: run scripts/run-windows.ps1
# on the Windows host instead - it builds through this script and copies on the Windows side
# (`-Package` for the package).
#
# Takes the build lock (scripts/lib/lock.sh), except with --package - never wrap it in flock. A
# running rbrowser.exe locks the file: close it first, or the copy fails.
set -euo pipefail

TARGET="x86_64-pc-windows-msvc"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/jobs.sh
source "$ROOT/scripts/lib/jobs.sh"
# shellcheck source=lib/lock.sh
source "$ROOT/scripts/lib/lock.sh"
# shellcheck source=lib/cef.sh
source "$ROOT/scripts/lib/cef.sh"
# shellcheck source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"

usage() {
    echo "usage: scripts/run-windows.sh [--release] [--build-only] [-- <arguments>]" >&2
    echo "       scripts/run-windows.sh --package [<zip>] [-- <arguments>]" >&2
    exit 2
}

arguments=("$@")
profile=dev
directory=debug
build_only=0
package=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) profile=release; directory=release; shift ;;
        --build-only) build_only=1; shift ;;
        --package)
            package=default
            shift
            if [[ $# -gt 0 && "$1" != -* ]]; then package="$(realpath "$1")"; shift; fi
            ;;
        --) shift; break ;;
        *) usage ;;
    esac
done
[[ -z "$package" || ("$build_only" == 0 && "$profile" == dev) ]] || usage
# The package is built already: no build, so no lock.
[[ -n "$package" ]] || rb_take_lock "${arguments[@]}"
cd "$ROOT"
if [[ "$package" == default ]]; then
    package="$ROOT/artifacts/windows/rbrowser-$(rb_workspace_version < Cargo.toml)-windows-x64.zip"
fi
if [[ -n "$package" && ! -f "$package" ]]; then
    echo "!! no package $package: scripts/package-windows.sh makes it" >&2
    exit 1
fi

# WSL appends the Windows PATH only with `appendWindowsPath`; the file is there regardless.
cmd="$(command -v cmd.exe || echo /mnt/c/Windows/System32/cmd.exe)"
if ! command -v wslpath > /dev/null 2>&1 || [[ ! -e "$cmd" ]]; then
    echo "!! wslpath or cmd.exe not found: this needs WSL" >&2
    exit 1
fi
if ! compgen -G '/proc/sys/fs/binfmt_misc/WSLInterop*' > /dev/null; then
    echo "!! WSL interop is off (no WSLInterop binfmt entry): enable [interop] in /etc/wsl.conf" >&2
    exit 1
fi

if [[ -z "$package" ]]; then
    CEF_PATH="$(rb_cef_path)"
    export CEF_PATH
    rb_cef_verify "$CEF_PATH" --if-present
    echo "==> cross-building rBrowser ($profile, jobs $JOBS, CEF in $CEF_PATH)"
    CARGO_BUILD_JOBS="$JOBS" cargo xwin build --locked --profile "$profile" --target "$TARGET" \
        -j "$JOBS" -p rbrowser
    rb_cef_verify "$CEF_PATH"
    build="${CARGO_TARGET_DIR:-$ROOT/target}/$TARGET/$directory"
    app="$build/app"
    rb_stage_app "$build" "$app" "$ROOT/ui/dist"
    if [[ "$build_only" == 1 ]]; then
        echo "==> assembled $app"
        exit 0
    fi
fi

# cmd.exe started from a WSL directory warns about UNC paths; from /mnt/c it does not.
localappdata="$(cd /mnt/c && "$cmd" /d /c 'echo %LOCALAPPDATA%' 2> /dev/null | tr -d '\r')"
if [[ -z "$localappdata" || "$localappdata" == "%LOCALAPPDATA%" ]]; then
    echo "!! could not read %LOCALAPPDATA% through cmd.exe" >&2
    exit 1
fi
name=rbrowser-dev
[[ -z "$package" ]] || name=rbrowser-package
destination="$(wslpath -u "$localappdata")/$name"
if ! mkdir -p "$destination" 2> /dev/null || [[ ! -w "$destination" ]]; then
    echo "!! $destination is not writable from WSL (C: mounted read-only?)." >&2
    echo "   Run on the Windows host instead:" >&2
    echo "   powershell -ExecutionPolicy Bypass -File $(wslpath -w "$ROOT/scripts/run-windows.ps1")${package:+ -Package}" >&2
    exit 1
fi
if [[ -n "$package" ]]; then
    echo "==> unpacking $package into $destination"
    folder="$(rb_package_unzip "$package" "$destination")"
else
    echo "==> copying $app to $destination"
    cp -ru "$app/." "$destination/"
    [[ -f "$app/ui/index.html" ]] || rm -rf "$destination/ui"
    folder="$destination"
fi
windows_exe="$(wslpath -w "$folder/rbrowser.exe")"
echo "==> starting $windows_exe $*"
# `start ""` detaches on the Windows side; setsid and the redirections detach it from this shell.
(cd /mnt/c && setsid "$cmd" /d /c start "" "$windows_exe" "$@" > /dev/null 2>&1 < /dev/null &)
echo "==> started"
