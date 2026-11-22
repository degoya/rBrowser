# shellcheck shell=bash
#
# The native Linux app in the gate (RB-097-05), for scripts/check.sh --rust: the CEF cache the
# Linux build links against and the check of the app the tests' build left.
#
#   source "$ROOT/scripts/lib/linux-app.sh"     # sources scripts/lib/cef.sh
#   rb_linux_app_cef                             # exports CEF_PATH, the shared CEF cache
#   rb_linux_app_check "$ROOT" "$JOBS" "$LOGS"   # builds rbrowser-nosandbox and holds it to CEF

# shellcheck source=scripts/lib/cef.sh
source "$(dirname "${BASH_SOURCE[0]}")/cef.sh"

# The Linux app links CEF: its distribution is downloaded once into the shared cache
# (scripts/lib/cef.sh), which every worktree and the package scripts use.
rb_linux_app_cef() {
    CEF_PATH="$(rb_cef_path)"
    export CEF_PATH
}

# Checkout $1, $2 build jobs, logs in $3: rbrowser-nosandbox linked against CEF's libcef.so,
# RUNPATH $ORIGIN for the package (libcef.so beside it) and, in a dev build, the cef crate's copy
# in target/debug/build; the CEF archive the build downloaded held to its pin.
rb_linux_app_check() {
    local root="$1" jobs="$2" logs="$3" exe dynamic
    exe="${CARGO_TARGET_DIR:-$root/target}/debug/rbrowser-nosandbox"
    dynamic="$logs/linux-app.dynamic"
    CARGO_BUILD_JOBS="$jobs" cargo build --locked -p rbrowser --bin rbrowser-nosandbox -j "$jobs" || return 1
    rb_cef_verify_linux "$CEF_PATH" || return 1
    [[ -f "$(dirname "$exe")/build/libcef.so" ]] || { echo "!! no libcef.so in $(dirname "$exe")/build (the cef crate's copy)" >&2; return 1; }
    readelf -d "$exe" > "$dynamic" || return 1
    grep -q 'NEEDED.*\[libcef\.so\]' "$dynamic" || { echo "!! $exe does not link libcef.so" >&2; return 1; }
    # shellcheck disable=SC2016  # $ORIGIN is the dynamic loader's, not the shell's
    grep -qE '(RUNPATH|RPATH).*\[\$ORIGIN[]:]' "$dynamic" || { echo "!! $exe has no RUNPATH \$ORIGIN" >&2; return 1; }
}
