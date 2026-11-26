#!/usr/bin/env bash
#
# The reproducible, checked content of the Windows package (RB-099-01), without a build: against
# fake stage folders and a scratch git repository.
# - rb_package_epoch: a clean commit's committer time, SOURCE_DATE_EPOCH when set, the clock for a
#   dirty tree; rb_package_iso_time writes it as VERSION.txt and build.rs take it.
# - rb_package_zip: the same stage, its files written in another order and with other times,
#   gives the same bytes under SOURCE_DATE_EPOCH - members sorted, every time the epoch, modes
#   0755 or 0644; rb_package_sums writes SHA256SUMS that `sha256sum -c` accepts.
# - rb_trim_locales: only en-US, de, es and fr (and their gendered variants) stay in locales\.
# - rb_check_stage against packaging/windows/package-content.txt: a stage with every listed entry
#   passes; a missing entry, a stray top-level entry, a locale left untrimmed, a missing bundled
#   plugin and a sidecar\node_modules\.bin each fail and are named.
#
# Pure bash and python3, a second.
#
#   scripts/tests/windows-paket.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
MANIFEST="$ROOT/packaging/windows/package-content.txt"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

# Runs the snippet $1 with scripts/lib/package.sh and package-stage.sh sourced; the rest are its
# arguments ($1 inside the snippet is the first of them).
lib() {
    local snippet="$1"
    shift
    run_status bash -c "source '$ROOT/scripts/lib/package.sh' && source '$ROOT/scripts/lib/package-stage.sh' && $snippet" _ "$@"
}

# --- the build time ---------------------------------------------------------------------------
REPO="$SCRATCH/repo"
mkdir -p "$REPO"
git -C "$REPO" init -q
printf 'one\n' > "$REPO/file"
git -C "$REPO" add -A
GIT_COMMITTER_DATE="2026-01-02T03:04:05Z" GIT_AUTHOR_DATE="2026-01-02T03:04:05Z" \
    git -C "$REPO" -c user.name=test -c user.email=test@example.invalid commit -qm one
COMMIT="$(git -C "$REPO" rev-parse --short=8 HEAD)"
EPOCH=1767323045

run_status bash -c 'cd "$1" && unset SOURCE_DATE_EPOCH && source "$2/scripts/lib/package.sh" && rb_package_epoch "$3"' \
    _ "$REPO" "$ROOT" "$COMMIT"
expect_status "the epoch of a clean commit" 0
expect "is its committer time" "$EPOCH" "$output"
run_status bash -c 'cd "$1" && unset SOURCE_DATE_EPOCH && source "$2/scripts/lib/package.sh" && rb_package_epoch "$3-dirty"' \
    _ "$REPO" "$ROOT" "$COMMIT"
expect_true "a dirty tree takes the clock, not the commit" '[[ "$output" =~ ^[0-9]+$ && "$output" -gt $EPOCH ]]'
run_status bash -c 'cd "$1" && source "$2/scripts/lib/package.sh" && SOURCE_DATE_EPOCH=1700000000 rb_package_epoch "$3"' \
    _ "$REPO" "$ROOT" "$COMMIT"
expect "SOURCE_DATE_EPOCH, when set, wins" "1700000000" "$output"
run_status bash -c 'cd "$1" && source "$2/scripts/lib/package.sh" && SOURCE_DATE_EPOCH=soon rb_package_epoch "$3"' \
    _ "$REPO" "$ROOT" "$COMMIT"
expect_status "a SOURCE_DATE_EPOCH that is no number is refused" 1
lib 'rb_package_iso_time "$1"' "$EPOCH"
expect "the time as VERSION.txt names it" "2026-01-02T03:04:05Z" "$output"

# --- the reproducible ZIP ---------------------------------------------------------------------
STAGES="$SCRATCH/stages"
make_stage() {
    local stage="$STAGES/rbrowser-1.2.3-windows-x64"
    rm -rf "$stage"
    mkdir -p "$stage/locales" "$stage/ui/assets"
    if [[ "$1" == forward ]]; then
        printf 'exe\n' > "$stage/rbrowser.exe"
        printf 'pak\n' > "$stage/locales/en-US.pak"
        printf 'html\n' > "$stage/ui/index.html"
        printf 'js\n' > "$stage/ui/assets/app.js"
    else
        printf 'js\n' > "$stage/ui/assets/app.js"
        printf 'html\n' > "$stage/ui/index.html"
        printf 'pak\n' > "$stage/locales/en-US.pak"
        printf 'exe\n' > "$stage/rbrowser.exe"
        touch -d '2030-05-06 07:08:09' "$stage/ui/index.html" "$stage/rbrowser.exe"
    fi
    chmod 755 "$stage/rbrowser.exe"
    chmod 600 "$stage/ui/index.html"
}
zip_stage() {
    rm -f "$STAGES/$1.zip"
    run_status env SOURCE_DATE_EPOCH="$EPOCH" bash -c \
        'source "$1/scripts/lib/package.sh" && rb_package_zip "$2" rbrowser-1.2.3-windows-x64 "$3"' \
        _ "$ROOT" "$STAGES" "$STAGES/$1.zip"
}
make_stage forward
zip_stage first
expect_status "the stage is zipped" 0
make_stage backward
zip_stage second
expect_status "the same stage, written the other way round, is zipped" 0
expect "two ZIPs of the same content are byte-identical" \
    "$(sha256sum < "$STAGES/first.zip")" "$(sha256sum < "$STAGES/second.zip")"
python3 -I - "$STAGES/second.zip" "$EPOCH" > "$SCRATCH/members" << 'PY'
import sys
import time
import zipfile

# A ZIP time counts in steps of two seconds.
epoch = time.gmtime(int(sys.argv[2]) // 2 * 2)[:6]
with zipfile.ZipFile(sys.argv[1]) as package:
    infos = package.infolist()
    names = [info.filename for info in infos]
    print("sorted" if names == sorted(names) else "unsorted")
    print("times" if all(info.date_time == epoch for info in infos) else "other times")
    for info in infos:
        print(f"{info.filename} {info.external_attr >> 16:o}")
PY
expect "the members are sorted" "sorted" "$(sed -n 1p "$SCRATCH/members")"
expect "every member carries the epoch" "times" "$(sed -n 2p "$SCRATCH/members")"
expect_true "the top folder comes first" 'grep -qx "rbrowser-1.2.3-windows-x64/ 40755" "$SCRATCH/members"'
expect_true "a program keeps 0755" 'grep -qx "rbrowser-1.2.3-windows-x64/rbrowser.exe 100755" "$SCRATCH/members"'
expect_true "any other file is 0644" 'grep -qx "rbrowser-1.2.3-windows-x64/ui/index.html 100644" "$SCRATCH/members"'

run_status bash -c 'source "$1/scripts/lib/package.sh" && rb_package_sums "$2" first.zip second.zip' \
    _ "$ROOT" "$STAGES"
expect_status "SHA256SUMS is written" 0
run_status bash -c 'cd "$1" && sha256sum -c SHA256SUMS' _ "$STAGES"
expect_status "and sha256sum -c accepts it" 0
expect "it names each file by its name alone" "first.zip second.zip" "$(cut -d ' ' -f 3 "$STAGES/SHA256SUMS" | xargs)"
run_status bash -c 'source "$1/scripts/lib/package.sh" && rb_package_sums "$2" missing.zip' _ "$ROOT" "$STAGES"
expect_status "a missing file is refused" 1

# --- the locales ------------------------------------------------------------------------------
LOCALES="$SCRATCH/locales"
mkdir -p "$LOCALES"
for name in en-US en-US_FEMININE en-GB de de_NEUTER es es-419 fr fr_MASCULINE it zh-CN; do
    : > "$LOCALES/$name.pak"
done
lib 'rb_trim_locales "$1"' "$LOCALES"
expect_status "the locales are trimmed" 0
expect "to en-US, de, es and fr with their variants" \
    "de.pak de_NEUTER.pak en-US.pak en-US_FEMININE.pak es.pak fr.pak fr_MASCULINE.pak" \
    "$(export LC_ALL=C; cd "$LOCALES" && printf '%s ' * | sed 's/ $//')"
rm -f "$LOCALES/en-US.pak"
lib 'rb_trim_locales "$1"' "$LOCALES"
expect_status "a CEF without en-US.pak is refused" 1
expect_output "naming it" "en-US.pak"

# --- the content ------------------------------------------------------------------------------
STAGE="$SCRATCH/content/rbrowser-1.2.3-windows-x64"
# A stage with every entry the manifest lists, required or optional.
fill_stage() {
    rm -rf "$STAGE"
    mkdir -p "$STAGE"
    local line path
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#\? }"
        path="$(xargs <<< "$line")"
        [[ -n "$path" ]] || continue
        if [[ "$path" == */ ]]; then
            mkdir -p "$STAGE/$path"
        else
            mkdir -p "$(dirname "$STAGE/$path")"
            : > "$STAGE/$path"
        fi
    done < "$MANIFEST"
    : > "$STAGE/plugins/alpha-0.1.0.rbplugin"
}
check() { lib 'rb_check_stage "$@"' "$STAGE" "$MANIFEST" plugins/alpha-0.1.0.rbplugin; }

expect_true "the manifest lists the program" 'grep -qx "rbrowser.exe" "$MANIFEST"'
fill_stage
check
expect_status "a stage with every listed entry passes" 0
rm -f "$STAGE/node/node.exe"
check
expect_status "a missing entry fails" 1
expect_output "and is named" "node/node.exe"
fill_stage
: > "$STAGE/libcef.lib"
check
expect_status "a stray top-level file fails" 1
expect_output "and is named" "libcef.lib"
fill_stage
: > "$STAGE/locales/it.pak"
check
expect_status "a locale left untrimmed fails" 1
expect_output "and is named" "locales/it.pak"
fill_stage
rm -f "$STAGE/plugins/alpha-0.1.0.rbplugin"
check
expect_status "a bundled plugin missing from the stage fails" 1
expect_output "and is named" "plugins/alpha-0.1.0.rbplugin"
fill_stage
mkdir -p "$STAGE/sidecar/node_modules/.bin"
check
expect_status "the sidecar's .bin links fail" 1
expect_output "and are named" "sidecar/node_modules/.bin"
fill_stage
rm -f "$STAGE/rbrowser.pdb"
rm -rf "$STAGE/plugins"
lib 'rb_check_stage "$@"' "$STAGE" "$MANIFEST"
expect_status "the optional entries may be absent" 0

finish_tests "windows-paket"
