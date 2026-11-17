#!/usr/bin/env bash
#
# scripts/lib/wsl-helper.sh without a build (RB-030-04): a fake `cargo` records its arguments
# and leaves a file where cargo would; ELF headers written here stand in for the real helper.
# The build asks for the musl target, the profile and the jobs and names the binary of that
# profile's folder (`dev` builds into debug/); the check takes only a static x86-64 Linux
# executable (no interpreter: it runs in any distribution without a matching libc) and names
# what is wrong otherwise; the stage puts it beside rbrowser.exe as `rbrowser-wsl`, executable.
#
# Pure bash and python3, a second.
#
#   scripts/tests/wsl-helper.sh
# shellcheck disable=SC2016  # the snippets are single-quoted on purpose: a child bash runs them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

BIN="$SCRATCH/bin"
mkdir -p "$BIN"
# cargo build ... --profile <p> --target <t> ...: records the arguments and writes the helper
# where cargo would, under $CARGO_TARGET_DIR.
cat > "$BIN/cargo" << 'SH'
#!/usr/bin/env bash
echo "$*" >> "$RB_FAKE_LOG"
[[ "${RB_FAKE_CARGO_FAIL:-0}" == 1 ]] && exit 101
profile="" target=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile) profile="$2"; shift 2 ;;
        --target) target="$2"; shift 2 ;;
        *) shift ;;
    esac
done
dir="$profile"
[[ "$profile" == dev ]] && dir=debug
mkdir -p "$CARGO_TARGET_DIR/$target/$dir"
printf 'built\n' > "$CARGO_TARGET_DIR/$target/$dir/rbrowser-wsl"
SH
chmod +x "$BIN/cargo"

# Runs snippet $1 in a bash that sourced the library, with the fake cargo first on PATH; $2...
# are its positional parameters.
in_lib() {
    local snippet="$1"
    shift
    run_status env PATH="$BIN:$PATH" RB_FAKE_LOG="$SCRATCH/cargo.log" \
        CARGO_TARGET_DIR="$SCRATCH/target" bash -c \
        'set -euo pipefail; source "$RB_ROOT/scripts/lib/wsl-helper.sh"; '"$snippet" _ "$@"
}
export RB_ROOT="$ROOT"

# An ELF64 little-endian executable of machine $2 (62 x86-64, 183 aarch64), ET_DYN, with one
# program header of type $3 (1 PT_LOAD, 3 PT_INTERP), into $1.
elf() {
    python3 -I - "$1" "$2" "$3" << 'PY'
import struct
import sys

path, machine, ptype = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
header = b"\x7fELF" + bytes([2, 1, 1, 0]) + bytes(8)
header += struct.pack("<HHIQQQIHHHHHH", 3, machine, 1, 0x1000, 64, 0, 0, 64, 56, 2, 64, 0, 0)
phdrs = struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, 120, 120, 0x1000)
phdrs += struct.pack("<IIQQQQQQ", ptype, 4, 0, 0, 0, 0, 0, 1)
with open(path, "wb") as out:
    out.write(header + phdrs)
PY
}

# --- rb_wsl_helper_build: the musl target, the profile, the jobs; the binary's path.
in_lib 'rb_wsl_helper_build release 3'
expect_status "the helper is built" 0
expect "and its path is named" "$SCRATCH/target/x86_64-unknown-linux-musl/release/rbrowser-wsl" "$output"
expect_true "for the musl target, locked, with the jobs" \
    'grep -q -- "build --locked --profile release --target x86_64-unknown-linux-musl -j 3 -p rb-wsl --bin rbrowser-wsl" "$SCRATCH/cargo.log"'
in_lib 'rb_wsl_helper_build dev 2'
expect "the dev profile builds into debug/" "$SCRATCH/target/x86_64-unknown-linux-musl/debug/rbrowser-wsl" "$output"
RB_FAKE_CARGO_FAIL=1 in_lib 'rb_wsl_helper_build release 2'
expect_status "a failed build stops the run" 1

# --- rb_wsl_helper_check: a static x86-64 Linux executable, nothing else.
elf "$SCRATCH/static" 62 1
in_lib 'rb_wsl_helper_check "$1"' "$SCRATCH/static"
expect_status "a static x86-64 executable passes" 0
elf "$SCRATCH/dynamic" 62 3
in_lib 'rb_wsl_helper_check "$1"' "$SCRATCH/dynamic"
expect_status "a dynamically linked one is refused" 1
expect_output "and named as such" "not static"
elf "$SCRATCH/arm" 183 1
in_lib 'rb_wsl_helper_check "$1"' "$SCRATCH/arm"
expect_status "another machine is refused" 1
expect_output "and named" "not x86-64"
printf 'MZ not an ELF\n' > "$SCRATCH/exe"
in_lib 'rb_wsl_helper_check "$1"' "$SCRATCH/exe"
expect_status "a Windows executable is refused" 1
expect_output "and named" "not an ELF"
in_lib 'rb_wsl_helper_check "$1"' "$SCRATCH/missing"
expect_status "a missing file is refused" 1

# --- rb_stage_wsl_helper: beside rbrowser.exe, executable, only after the check.
STAGE="$SCRATCH/stage"
mkdir -p "$STAGE"
in_lib 'rb_stage_wsl_helper "$1" "$2"' "$SCRATCH/dynamic" "$STAGE"
expect_status "a helper that fails the check is not staged" 1
expect_true "and nothing lands in the stage" '[[ ! -e "$STAGE/rbrowser-wsl" ]]'
in_lib 'rb_stage_wsl_helper "$1" "$2"' "$SCRATCH/static" "$STAGE"
expect_status "the helper is staged" 0
expect_true "as rbrowser-wsl" 'cmp -s "$SCRATCH/static" "$STAGE/rbrowser-wsl"'
expect_true "executable" '[[ -x "$STAGE/rbrowser-wsl" ]]'

finish_tests "wsl-helper"
