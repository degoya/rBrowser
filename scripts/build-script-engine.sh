#!/usr/bin/env bash
#
# Builds the script engine of MCP code mode (RB-098-01): plugins/script-engine for
# wasm32-unknown-unknown, `wasm-tools component new`, then holds the component to its one
# import - `rbrowser:script/host@0.1.0`, no WASI, nothing else - and writes it to
# crates/rb-script/engine/rb-script-engine.wasm, which rb-script embeds (`include_bytes!`) and
# which is checked in, so the app builds on every platform without a WebAssembly toolchain.
# Run it after a change below plugins/script-engine/ or crates/rb-script/wit/ and commit the
# component with the change.
#
# It builds in the plugins' target lane (target/lanes/plugins, as scripts/build-plugins.sh) and
# takes the build lock like every heavy script (scripts/lib/lock.sh) - never wrap it in flock.
#
#   scripts/build-script-engine.sh            # build, check, write the component
#   scripts/build-script-engine.sh --imports  # only check the checked-in component's imports
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENGINE="$ROOT/plugins/script-engine"
OUT="$ROOT/crates/rb-script/engine/rb-script-engine.wasm"
TARGET_DIR="${RB_PLUGIN_TARGET_DIR:-$ROOT/target/lanes/plugins}"
WASM_TOOLS="${WASM_TOOLS:-wasm-tools}"
# The one interface the component may import.
ALLOWED="rbrowser:script/host@0.1.0"

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'
}

case "${1:-}" in
    "" | --imports) ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
esac

command -v "$WASM_TOOLS" > /dev/null || {
    echo "!! wasm-tools is not installed (cargo install --locked wasm-tools)" >&2
    exit 2
}

# The imports of `component`, one per line.
imports_of() {
    "$WASM_TOOLS" component wit "$1" \
        | sed -n 's/^[[:space:]]*import[[:space:]]\{1,\}\([^;]*\);.*$/\1/p' \
        | sed 's/[[:space:]]*$//'
}

# Fails unless `component` imports exactly the host interface.
check_imports() {
    local component="$1" imports
    imports="$(imports_of "$component")" || { echo "!! $component is no component" >&2; return 1; }
    if [[ "$imports" != "$ALLOWED" ]]; then
        echo "!! $component must import $ALLOWED and nothing else; it imports:" >&2
        printf '   %s\n' "${imports:-<nothing>}" >&2
        return 1
    fi
    echo "==> $component imports only $ALLOWED"
}

if [[ "${1:-}" == --imports ]]; then
    [[ -f "$OUT" ]] || { echo "!! $OUT does not exist; run scripts/build-script-engine.sh" >&2; exit 1; }
    check_imports "$OUT"
    exit 0
fi

# shellcheck source=lib/jobs.sh
source "$ROOT/scripts/lib/jobs.sh"
# shellcheck source=lib/lock.sh
source "$ROOT/scripts/lib/lock.sh"
rb_take_lock "$@"

echo "==> plugins/script-engine for wasm32-unknown-unknown"
# In the engine's folder, so its .cargo/config.toml (getrandom's custom backend) applies.
(cd "$ENGINE" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$JOBS}" CARGO_TARGET_DIR="$TARGET_DIR" \
    cargo build --locked --quiet --release --target wasm32-unknown-unknown)
core="$TARGET_DIR/wasm32-unknown-unknown/release/rb_script_engine.wasm"
staged="$(mktemp "${TMPDIR:-/tmp}/rb-script-engine.XXXXXX.wasm")"
trap 'rm -f "$staged"' EXIT
"$WASM_TOOLS" component new "$core" -o "$staged"
check_imports "$staged"
mkdir -p "$(dirname "$OUT")"
cp "$staged" "$OUT"
size="$(wc -c < "$OUT")"
echo "==> $OUT ($size bytes); commit it with the change"
