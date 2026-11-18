#!/usr/bin/env bash
#
# The portable self-update with real binaries (RB-090-14): the real `rbrowser-updater` switches a
# portable installation from one build of this tree to the next and back. The program is
# rb-update's stand-in `update_smoke_program` (crates/rb-update/examples/), which does what
# rBrowser does around an update - recover at the start, share the instance lock, prove the
# switched version, hand over - without CEF, so the run needs no display. Three updates, each in
# a fresh installation of <old>:
#
#   1. Install and restart: the updater switches, starts <new>, which proves itself; the next
#      start removes .previous/.
#   2. A <new> that does not prove itself (RB_UPDATE_TEST_FAIL_START, honoured by a debug build
#      only): the updater takes it back and starts <old> again.
#   3. Install at quit: the updater switches and starts nothing; the next start, a person's, is
#      <new> and proves it.
#
# The signed manifest, the replay floor and the verified download are rb-update's unit tests,
# with a test key: no build carries a trust root other than the release key, on purpose.
#
# Usage:
#   scripts/self-update-smoke.sh <old-dir> <old-version> <new-dir> <new-version>
# where each dir holds a debug build's `update_smoke_program[.exe]` and `rbrowser-updater[.exe]`
# (`cargo build -p rb-update --bin rbrowser-updater --example update_smoke_program`, then
# target/debug/ and target/debug/examples/). RB_SMOKE_LOGS=<dir> keeps the installations and
# their journals for a failed run. Ends with `==> all requested checks passed`; exit 0 when every
# check held, 1 when one did not, 2 on a usage error. Compiles nothing.
set -euo pipefail

usage() {
    echo "usage: scripts/self-update-smoke.sh <old-dir> <old-version> <new-dir> <new-version>" >&2
    exit 2
}

[[ $# -eq 4 ]] || usage
OLD_DIR="$1" OLD="$2" NEW_DIR="$3" NEW="$4"
EXT=""
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) EXT=".exe" ;; esac
for dir in "$OLD_DIR" "$NEW_DIR"; do
    for binary in update_smoke_program rbrowser-updater; do
        if [[ ! -f "$dir/$binary$EXT" ]]; then
            echo "!! $dir/$binary$EXT is missing" >&2
            exit 2
        fi
    done
done
[[ "$OLD" != "$NEW" ]] || { echo "!! the versions are the same" >&2; exit 2; }
PYTHON="$(command -v python3 || command -v python || true)"
[[ -n "$PYTHON" ]] || { echo "!! python3 is needed" >&2; exit 2; }

WORK="${RB_SMOKE_LOGS:-}"
if [[ -z "$WORK" ]]; then
    WORK="$(mktemp -d)"
    trap 'rm -rf "$WORK"' EXIT
fi
mkdir -p "$WORK"
WORK="$(cd "$WORK" && pwd)"
failures=0

check() {
    local what="$1"
    shift
    if "$@"; then
        echo "ok   $what"
    else
        echo "FAIL $what"
        failures=$((failures + 1))
    fi
}

# A portable installation of <old> in $1: the program as rbrowser, the updater beside it,
# VERSION.txt and a file of the person's own.
install_old() {
    local folder="$1"
    mkdir -p "$folder"
    cp "$OLD_DIR/update_smoke_program$EXT" "$folder/rbrowser$EXT"
    cp "$OLD_DIR/rbrowser-updater$EXT" "$folder/rbrowser-updater$EXT"
    printf 'rBrowser %s\n' "$OLD" > "$folder/VERSION.txt"
    printf 'mine\n' > "$folder/notes.txt"
}

# The ZIP of <new> at $1, laid out as the package: one version folder, executables 0755.
zip_new() {
    "$PYTHON" -I - "$1" "$NEW" "$NEW_DIR" "$EXT" << 'PY'
import sys, zipfile
out, version, folder, ext = sys.argv[1:5]
root = f"rbrowser-{version}-smoke/"
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as archive:
    for source, name in ((f"update_smoke_program{ext}", f"rbrowser{ext}"),
                         (f"rbrowser-updater{ext}", f"rbrowser-updater{ext}")):
        info = zipfile.ZipInfo(root + name)
        info.external_attr = 0o100755 << 16
        info.compress_type = zipfile.ZIP_DEFLATED
        with open(f"{folder}/{source}", "rb") as handle:
            archive.writestr(info, handle.read())
    archive.writestr(root + "VERSION.txt", f"rBrowser {version}\n")
PY
}

# The journal's field $2 of data folder $1, or "none".
journal() {
    "$PYTHON" -I - "$1/update/journal.json" "$2" << 'PY'
import json, sys
try:
    with open(sys.argv[1], encoding="utf-8") as handle:
        value = json.load(handle).get(sys.argv[2])
except OSError:
    value = None
print("none" if value is None else str(value).lower())
PY
}

# Waits up to $3 seconds until the journal of $1 is in one of the phases $2 (a regex).
wait_phase() {
    local data="$1" phases="$2" seconds="$3" phase
    for ((i = 0; i < seconds * 2; i++)); do
        phase="$(journal "$data" phase)"
        if [[ "$phase" =~ ^($phases)$ ]]; then
            return 0
        fi
        sleep 0.5
    done
    echo "     the journal of $data stayed at $(journal "$data" phase)" >&2
    return 1
}

# Waits up to $3 seconds until the start log of $1 has the line $2.
wait_start() {
    local data="$1" line="$2" seconds="$3"
    for ((i = 0; i < seconds * 2; i++)); do
        if grep -qxF "$line" "$data/smoke-starts.log" 2> /dev/null; then
            return 0
        fi
        sleep 0.5
    done
    echo "     $data/smoke-starts.log has no '$line'" >&2
    return 1
}

version_is() { grep -qx "rBrowser $2" "$1/VERSION.txt"; }
is_dir() { [[ -d "$1" ]]; }
no_dir() { [[ ! -e "$1" ]]; }
same() { [[ "$1" == "$2" ]]; }

zip_new "$WORK/rbrowser-$NEW-smoke.zip"

echo "==> 1. install and restart: $OLD -> $NEW"
one="$WORK/one/rbrowser-$OLD" data1="$WORK/one/data"
install_old "$one"
"$one/rbrowser$EXT" --data "$data1" hand-over --zip "$WORK/rbrowser-$NEW-smoke.zip" --version "$NEW"
check "the updater ends with the new version proven" wait_phase "$data1" verified 120
check "the new version proved itself at its start" wait_start "$data1" "$NEW proven=true" 30
check "the program folder holds $NEW" version_is "$one" "$NEW"
check "the person's own file stays" grep -qx mine "$one/notes.txt"
check "the old version waits in .previous/" is_dir "$one/.previous"
sleep 3
"$one/rbrowser$EXT" --data "$data1"
check "the next start removes .previous/" no_dir "$one/.previous"
check "and marks the update cleaned" same "$(journal "$data1" cleaned)" true

echo "==> 2. a new version that does not prove itself is taken back"
two="$WORK/two/rbrowser-$OLD" data2="$WORK/two/data"
install_old "$two"
RB_UPDATE_TEST_FAIL_START=1 "$two/rbrowser$EXT" --data "$data2" hand-over \
    --zip "$WORK/rbrowser-$NEW-smoke.zip" --version "$NEW" --confirm-timeout 10
check "the updater takes the update back" wait_phase "$data2" rolled_back 120
check "because the new version ended unproven" same "$(journal "$data2" reason)" update.new_version_exited
check "the program folder holds $OLD again" version_is "$two" "$OLD"
check "and $OLD runs again" wait_start "$data2" "$OLD proven=false" 30
check "the person's own file stays" grep -qx mine "$two/notes.txt"

echo "==> 3. install at quit: switched now, proven by the next start"
three="$WORK/three/rbrowser-$OLD" data3="$WORK/three/data"
install_old "$three"
"$three/rbrowser$EXT" --data "$data3" hand-over --zip "$WORK/rbrowser-$NEW-smoke.zip" \
    --version "$NEW" --on-quit
check "the updater switches and starts nothing" wait_phase "$data3" switched 120
sleep 2
check "nothing started the new version" same "$(grep -c "^$NEW " "$data3/smoke-starts.log" || true)" 0
"$three/rbrowser$EXT" --data "$data3"
check "the person's next start is $NEW and proves it" wait_start "$data3" "$NEW proven=true" 10
check "the journal says verified" same "$(journal "$data3" phase)" verified
check "the program folder holds $NEW" version_is "$three" "$NEW"

if ((failures)); then
    echo "!! $failures checks failed; the installations are in ${RB_SMOKE_LOGS:-a removed scratch folder}" >&2
    exit 1
fi
echo "==> all requested checks passed"
