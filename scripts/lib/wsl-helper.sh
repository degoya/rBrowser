# shellcheck shell=bash
#
# rbrowser-wsl, rBrowser's helper inside a WSL distribution (RB-030-04: `watch` for hot reload,
# `cdp-proxy` for the CDP tunnel). The app starts it as `wsl.exe -d <distro> -e
# /mnt/<drive>/<package folder>/rbrowser-wsl ...` from the folder of rbrowser.exe, so it has to
# run in any distribution: a static x86-64 Linux executable for x86_64-unknown-linux-musl,
# with musl and its start files from Rust's own target (self-contained) and linked by rust-lld
# (.cargo/config.toml) - no musl-gcc, no C toolchain, the same from WSL and from Windows itself.
# Sourced by scripts/package-windows.sh (the ZIP) and scripts/check.sh (--rust); tested by
# scripts/tests/wsl-helper.sh.
#
#   rb_wsl_helper_build <profile> <jobs>   # builds; prints the binary's path
#   rb_wsl_helper_check <file>             # a static x86-64 ELF executable, or exit 1
#   rb_stage_wsl_helper <file> <app dir>   # checks, then installs <app dir>/rbrowser-wsl (755)

RB_WSL_TARGET="x86_64-unknown-linux-musl"

# Builds the helper with cargo profile $1 and $2 jobs (cargo's output on stderr) and prints the
# path of the binary - under CARGO_TARGET_DIR when set, else target/ of the working directory.
rb_wsl_helper_build() {
    local profile="$1" jobs="$2" folder
    CARGO_BUILD_JOBS="$jobs" cargo build --locked --profile "$profile" --target "$RB_WSL_TARGET" \
        -j "$jobs" -p rb-wsl --bin rbrowser-wsl >&2 || return 1
    folder="$profile"
    [[ "$profile" == dev ]] && folder=debug
    echo "${CARGO_TARGET_DIR:-$PWD/target}/$RB_WSL_TARGET/$folder/rbrowser-wsl"
}

# Holds $1 to what a distribution can start without a matching libc: an ELF64 little-endian
# executable for x86-64 without a program interpreter (PT_INTERP), i.e. statically linked.
rb_wsl_helper_check() {
    python3 -I - "$1" << 'PY'
import struct
import sys

path = sys.argv[1]
try:
    with open(path, "rb") as binary:
        data = binary.read()
except OSError as error:
    sys.exit(f"!! the WSL helper {path} cannot be read: {error.strerror}")
if data[:4] != b"\x7fELF" or len(data) < 64:
    sys.exit(f"!! the WSL helper {path} is not an ELF executable")
if data[4] != 2 or data[5] != 1:
    sys.exit(f"!! the WSL helper {path} is not a 64-bit little-endian ELF")
kind, machine = struct.unpack_from("<HH", data, 16)
if machine != 62:
    sys.exit(f"!! the WSL helper {path} is not x86-64 (machine {machine})")
if kind not in (2, 3):
    sys.exit(f"!! the WSL helper {path} is not an executable (type {kind})")
phoff = struct.unpack_from("<Q", data, 32)[0]
phentsize, phnum = struct.unpack_from("<HH", data, 54)
for index in range(phnum):
    offset = phoff + index * phentsize
    if offset + 4 > len(data):
        sys.exit(f"!! the WSL helper {path} has a truncated program header table")
    if struct.unpack_from("<I", data, offset)[0] == 3:
        sys.exit(f"!! the WSL helper {path} is not static: it asks for a program interpreter")
PY
}

# Stages helper $1 as $2/rbrowser-wsl, executable, once it passed rb_wsl_helper_check.
rb_stage_wsl_helper() {
    local helper="$1" app="$2"
    rb_wsl_helper_check "$helper" || return 1
    install -m 755 "$helper" "$app/rbrowser-wsl"
}
