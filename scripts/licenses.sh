#!/usr/bin/env bash
#
# Writes ui/src/about/licenses/third-party.json, the component licences the About page of the
# settings lists (RB-010-18; adapted from rCommander's scripts/licenses.sh, RC-010-19).
#
# Generated, never kept by hand. `cargo tree` names every crate the rbrowser binary links on the
# platforms it is built for (Windows x64, Linux x64), every crate the WSL helper rbrowser-wsl
# links for x86_64-unknown-linux-musl (RB-030-04) and every crate a bundled plugin links for
# wasm32, features resolved as a build resolves them; proc macros, build scripts and tests run
# on the build machine and ship nowhere. `cargo metadata` has the declared licences. The npm
# packages come from the shell's package-lock.json (production dependencies, without the
# optional platform binaries of the build tools). Every other package of a lock file is listed
# by name as not shipped, so the tests can tell a package new to a lock file from one the list
# forgot. A package that declares no licence takes its entry from
# ui/src/about/licenses/overrides.json, which names where the licence was read; without one the
# list is not written. The bundled binaries (CEF, Node, ...) are ui/src/about/licenses/bundled.json,
# kept by hand. scripts/lib/licenses.py does the work.
#
# The tests in ui/src/about/licences.test.ts hold the list to Cargo.lock, the plugins' lock files
# and ui/package-lock.json. Neither `cargo metadata` nor `cargo tree` compiles anything, so no
# build lock is taken; they may download the manifests of crates for other platforms the first
# time, unless --offline.
#
# Usage:
#   scripts/licenses.sh                      # rewrite the list
#   scripts/licenses.sh --check              # exit 1 if it would change, 2 if a licence is missing
#   scripts/licenses.sh --offline [--check]  # cargo without the network
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The platforms the app is built for (phase 1: Windows x64; Linux x64 since RB-097-05) and the
# plugins' target (scripts/build-plugins.sh).
APP_TARGETS=(x86_64-pc-windows-msvc x86_64-unknown-linux-gnu)
# The WSL helper beside rbrowser.exe (scripts/lib/wsl-helper.sh).
WSL_HELPER_TARGET=x86_64-unknown-linux-musl
PLUGIN_TARGET=wasm32-unknown-unknown

cargo_flags=(--locked)
script_flags=()
for argument in "$@"; do
    case "$argument" in
        --offline) cargo_flags+=(--offline) ;;
        --check) script_flags+=(--check) ;;
        *)
            echo "unknown argument: $argument" >&2
            exit 2
            ;;
    esac
done

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# Metadata and tree of one Rust root: $1 the scratch name, $2 the manifest, $3 the package, $4...
# the targets.
rust_root() {
    local name="$1" manifest="$2" package="$3" target
    shift 3
    cargo metadata --format-version 1 --manifest-path "$manifest" "${cargo_flags[@]}" \
        > "$scratch/$name.metadata.json"
    : > "$scratch/$name.tree.txt"
    for target in "$@"; do
        cargo tree --manifest-path "$manifest" "${cargo_flags[@]}" --package "$package" \
            --target "$target" --edges normal,no-proc-macro --prefix none --format '{p}' \
            >> "$scratch/$name.tree.txt"
    done
}

cd "$ROOT"
rust_root app "$ROOT/Cargo.toml" rbrowser "${APP_TARGETS[@]}"
rust_root wsl-helper "$ROOT/Cargo.toml" rb-wsl "$WSL_HELPER_TARGET"
for manifest in "$ROOT"/plugins/*/Cargo.toml; do
    [[ -e "$manifest" ]] || continue
    plugin="$(basename "$(dirname "$manifest")")"
    # The package name may differ from the folder (`a11y-axe-plugin` in plugins/a11y-axe/).
    package="$(awk -F'"' '/^name = "/ { print $2; exit }' "$manifest")"
    rust_root "plugin-$plugin" "$manifest" "$package" "$PLUGIN_TARGET"
done
# Not exec: the trap removes the scratch directory afterwards.
python3 -B "$ROOT/scripts/lib/licenses.py" "$ROOT" "$scratch" "${script_flags[@]}"
