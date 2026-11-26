# shellcheck shell=bash
#
# What the package scripts share: the version a package is named by, the commit it is
# built from, and the owner's rule that one version never names two builds (AGENTS.md, "A new
# package with a new feature is at least a new minor version").
#
#   source "$ROOT/scripts/lib/package.sh"
#   version="$(rb_package_version "$ROOT")"   # fails on an empty or inconsistent version
#   commit="$(rb_package_commit)"              # 1a2b3c4d, 1a2b3c4d-dirty or unknown
#   rb_package_clean "$commit" "$allow_dirty"  # a dirty tree stops the run unless allowed
#   rb_package_guard "$ZIP" "<stage>/VERSION.txt" "$commit"
#   rb_package_sign_check "$ROOT"             # --sign / RB_SIGN=1: the signer is configured
#   rb_package_hold_signing                   # RB_SIGN_*, RB_UPDATE_SIGNING_KEY leave the environment
#   rb_package_with_signing <command>...      # ...and come back for this one command only
#   list="$(rb_package_plugins "$ROOT" "$prebuilt")"   # the bundled .rbplugin files, one per line
#   epoch="$(rb_package_epoch "$commit")"      # the build time: the commit's, so two builds match
#   rb_package_iso_time "$epoch"               # 2026-01-02T03:04:05Z
#   rb_package_zip "$OUT" "<stage name>" "$ZIP"   # reproducible under SOURCE_DATE_EPOCH
#   rb_package_sums "$OUT" <file>...           # $OUT/SHA256SUMS of the named files of $OUT
#   rb_package_unportable "$STAGE"            # rb_package_zip runs it; the tarball calls it
#   rb_package_unzip "$ZIP" "$DEST"           # unpacks a package's one folder, prints its path
#
# rb_package_guard fails when the package already exists and was built from another commit, from
# a dirty tree, or names no commit at all - then the version is raised (scripts/set-version.sh).
# RB_REPACK=1 replaces it anyway.

# shellcheck source=scripts/lib/workspace-version.sh
source "$(dirname "${BASH_SOURCE[0]}")/workspace-version.sh"

# The workspace version of the checkout $1, after scripts/set-version.sh --check held it; nothing
# and a failure when there is none or it is inconsistent, so no package is named `rbrowser--...`.
rb_package_version() {
    local root="$1" version
    version="$(rb_workspace_version < "$root/Cargo.toml")"
    if [[ -z "$version" ]]; then
        echo "!! Cargo.toml has no [workspace.package] version; nothing was packaged" >&2
        return 1
    fi
    if ! "$root/scripts/set-version.sh" --check >&2; then
        echo "!! scripts/set-version.sh --check failed; nothing was packaged" >&2
        return 1
    fi
    printf '%s\n' "$version"
}

# The commit of the working tree, `-dirty` when anything differs from it - untracked files
# included, ignored ones not.
rb_package_commit() {
    local commit
    commit="$(git rev-parse --short=8 HEAD 2> /dev/null)" || { echo unknown; return 0; }
    [[ -z "$(git status --porcelain 2> /dev/null)" ]] || commit="$commit-dirty"
    printf '%s\n' "$commit"
}

# Fails, with what to do, when commit $1 (rb_package_commit) is a dirty tree and $2 is not 1
# (--allow-dirty): a package names the commit it was built from, and a dirty tree is none.
rb_package_clean() {
    local commit="$1" allow="${2:-0}"
    [[ "$commit" == *-dirty ]] || return 0
    if [[ "$allow" == 1 ]]; then
        echo "note: --allow-dirty: the package is built from a dirty tree ($commit) and says so in VERSION.txt" >&2
        return 0
    fi
    echo "!! the working tree is dirty ($commit): commit or stash the changes first" >&2
    echo "   (git status). --allow-dirty packages it anyway, for a local test only. Nothing was built." >&2
    return 1
}

# The `--sign` check, also asked for by RB_SIGN=1, before anything is built: the signer of
# checkout $1 (scripts/sign-windows.sh --check, RB-090-03) finds its tool, key and timestamp
# server, or the run ends here - so no package is changed and no unsigned package passes for a
# signed one. The signer prints neither the key's path nor its password.
rb_package_sign_check() {
    local root="$1"
    if ! "$root/scripts/sign-windows.sh" --check; then
        echo "!! --sign or RB_SIGN=1: signing not configured (docs/release.md, \"Code signing\")." >&2
        echo "   Nothing was built and no package was changed." >&2
        return 1
    fi
}

# The signing variables (RB_SIGN_*: the key, its password, the tool; RB_UPDATE_SIGNING_KEY) stay
# in this shell but leave the environment, so no build script, npm script or compiler the build
# runs inherits them; rb_package_with_signing gives them back to the one command that signs.
_rb_package_held=()
rb_package_hold_signing() {
    local name
    _rb_package_held=()
    while IFS= read -r name; do
        case "$name" in
            RB_SIGN_* | RB_UPDATE_SIGNING_KEY)
                _rb_package_held+=("$name")
                export -n "${name?}"
                ;;
        esac
    done < <(compgen -e)
}

# Runs $@ with the variables rb_package_hold_signing took out of the environment.
rb_package_with_signing() {
    (
        for name in "${_rb_package_held[@]}"; do export "${name?}"; done
        "$@"
    )
}

# The `commit ...` line of member $2 (VERSION.txt) in the zip or tarball $1; a failure when it
# cannot be read.
rb_package_built_commit() {
    python3 -I - "$1" "$2" << 'PY'
import sys
import tarfile
import zipfile

archive, member = sys.argv[1], sys.argv[2]
try:
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as package:
            text = package.read(member).decode("utf-8")
    else:
        with tarfile.open(archive) as package:
            text = package.extractfile(member).read().decode("utf-8")
except (OSError, KeyError, AttributeError, UnicodeDecodeError, tarfile.TarError, zipfile.BadZipFile):
    sys.exit(1)
for line in text.splitlines():
    if line.startswith("commit "):
        print(line[len("commit "):])
        sys.exit(0)
sys.exit(1)
PY
}

# Whether the package $1 may be written for commit $3: it does not exist yet, or its member $2
# names the same clean commit, or RB_REPACK=1.
rb_package_guard() {
    local archive="$1" member="$2" commit="$3" found
    [[ -e "$archive" ]] || return 0
    if [[ "${RB_REPACK:-0}" == 1 ]]; then
        echo "note: RB_REPACK=1, so $archive is replaced whatever build it holds" >&2
        return 0
    fi
    found="$(rb_package_built_commit "$archive" "$member")" || found=""
    if [[ -n "$found" && "$found" == "$commit" && "$commit" != *-dirty && "$commit" != unknown ]]; then
        return 0
    fi
    echo "!! $archive exists, built from commit ${found:-(not readable)}; this build is $commit." >&2
    echo "   One version never names two builds: raise it with scripts/set-version.sh, or" >&2
    echo "   RB_REPACK=1 replaces the package. Nothing was built." >&2
    return 1
}

# The bundled plugin packages of checkout $1, one path per line: built and signed by
# scripts/build-plugins.sh, or - with a directory as $2, a CI job's signed packages -
# taken from there unbuilt. A plugin without its package is a failure; a plugin folder with the
# marker `TEST-ONLY` (plugins/hello-panel, RB-060-12) is built and tested but never packaged, one
# with `CATALOG-ONLY` goes to the plugin catalogue instead (RB-091-01).
# Every command checks its own status: a command substitution runs without `set -e`.
rb_package_plugins() {
    local root="$1" from="${2:-}" names name version package
    if [[ -z "$from" ]]; then
        echo "==> building the bundled plugins" >&2
        "$root/scripts/build-plugins.sh" >&2 || return $?
        from="$root/artifacts/plugins"
    fi
    names="$("$root/scripts/build-plugins.sh" --list)" || return $?
    while IFS= read -r name; do
        [[ -n "$name" ]] || continue
        [[ -f "$root/plugins/$name/TEST-ONLY" ]] && continue
        # Published in the plugin catalogue instead (scripts/catalogue-plugins.sh, RB-091-01).
        [[ -f "$root/plugins/$name/CATALOG-ONLY" ]] && continue
        version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/plugins/$name/manifest.toml" | head -n 1)"
        package="$from/$name-$version.rbplugin"
        [[ -f "$package" ]] || { echo "!! no plugin package $package" >&2; return 1; }
        printf '%s\n' "$package"
    done <<< "$names"
}

# Fails, naming each, when the folder $1 carries what switches a copy to the portable mode
# (rb_config::dirs, RB-010-15): the marker `portable` or a `data` beside a program (`rbrowser`,
# `rbrowser.exe`). A package never makes a copy portable by itself, and the update (RB-090-02)
# leaves both where they are, so one shipped would never go again.
rb_package_unportable() {
    local stage="$1" entry folder found=()
    while IFS= read -r -d '' entry; do
        folder="$(dirname "$entry")"
        if [[ -e "$folder/rbrowser" || -e "$folder/rbrowser.exe" ]]; then
            found+=("$entry")
        fi
    done < <(find "$stage" \( -name portable -o -name data \) -print0)
    if ((${#found[@]})); then
        printf '!! the package carries what makes a copy portable: %s\n' "${found[@]}" >&2
        echo "   No package carries portable or data beside the program; nothing was packaged." >&2
        return 1
    fi
}

# Unpacks the package $1 into folder $2 and prints the path of its one top folder, which is
# emptied first, so no file of an older package stays. A ZIP with more than one top folder, or a
# member that leaves it, is refused.
rb_package_unzip() {
    local python
    python="$(command -v python3 || command -v python)" || { echo "!! neither python3 nor python" >&2; return 1; }
    "$python" -I - "$1" "$2" << 'PY'
import os
import shutil
import sys
import zipfile

archive, target = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(archive) as package:
    names = package.namelist()
    tops = {name.split("/", 1)[0] for name in names}
    if len(tops) != 1 or any(name.startswith("/") or ".." in name.split("/") for name in names):
        sys.exit(f"!! {archive} is no package: one top folder, no member outside it")
    folder = os.path.join(target, tops.pop())
    shutil.rmtree(folder, ignore_errors=True)
    package.extractall(target)
print(folder)
PY
}

# The build time of commit $1 (rb_package_commit) in seconds: SOURCE_DATE_EPOCH when the caller
# set it, else the committer time of HEAD for a clean commit - so two builds of one commit name
# the same time and pack the same bytes (RB-099-01) - and the clock for a dirty tree or none.
rb_package_epoch() {
    local commit="$1" epoch
    if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
        if [[ ! "$SOURCE_DATE_EPOCH" =~ ^[0-9]+$ ]]; then
            echo "!! SOURCE_DATE_EPOCH '$SOURCE_DATE_EPOCH' is no number of seconds" >&2
            return 1
        fi
        printf '%s\n' "$SOURCE_DATE_EPOCH"
        return 0
    fi
    if [[ "$commit" != *-dirty && "$commit" != unknown ]] \
        && epoch="$(git log -1 --format=%ct HEAD 2> /dev/null)" && [[ "$epoch" =~ ^[0-9]+$ ]]; then
        printf '%s\n' "$epoch"
        return 0
    fi
    date +%s
}

# The epoch $1 in UTC, in the form VERSION.txt and build.rs name the build time.
rb_package_iso_time() {
    python3 -I -c 'import sys, time; print(time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(int(sys.argv[1]))))' "$1"
}

# Writes $1/SHA256SUMS - `<sha256>  <name>`, as `sha256sum -c` reads it inside $1 - for the files
# $2... of directory $1; a file that is not there is a failure and leaves no SHA256SUMS.
rb_package_sums() {
    local dir="$1"
    shift
    if ! (cd "$dir" && sha256sum -- "$@" > SHA256SUMS.part && mv SHA256SUMS.part SHA256SUMS); then
        rm -f "$dir/SHA256SUMS.part"
        echo "!! no SHA256SUMS written in $dir" >&2
        return 1
    fi
}

# Zips the folder $2 of directory $1 into $3 (an absolute path), the folder at the top, with
# Python's zipfile - the same on WSL and on the Windows runner's Git Bash, which has no `zip`.
# Reproducible (RB-099-01): the members sorted by name, the folders among them; the modes only
# 0755 (a folder, or a file with any execute bit) or 0644; and under SOURCE_DATE_EPOCH every
# member dated to it (UTC), so the same stage gives the same bytes whatever order and times its
# files were written with. Without SOURCE_DATE_EPOCH a member keeps its file's time. A folder
# that carries the portable mode's marker or data is refused first (rb_package_unportable).
rb_package_zip() {
    local dir="$1" name="$2" zip="$3" python
    rb_package_unportable "$dir/$name" || return 1
    python="$(command -v python3 || command -v python)" || { echo "!! neither python3 nor python" >&2; return 1; }
    "$python" -I - "$dir" "$name" "$zip" "${SOURCE_DATE_EPOCH:-}" << 'PY'
import os
import shutil
import stat
import sys
import time
import zipfile

base, name, target, epoch = sys.argv[1:5]
# ZIP dates start in 1980.
fixed = time.gmtime(max(int(epoch), 315532800))[:6] if epoch else None
paths = []
for folder, dirs, files in os.walk(os.path.join(base, name)):
    # A link would be dropped (a folder) or stored as a copy (a file): refuse it instead.
    for entry in dirs + files:
        if os.path.islink(os.path.join(folder, entry)):
            sys.exit(f"!! {os.path.join(folder, entry)} is a symbolic link; the package takes none")
    paths.append(folder)
    paths.extend(os.path.join(folder, entry) for entry in files)
members = sorted((os.path.relpath(path, base).replace(os.sep, "/"), path) for path in paths)
with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as package:
    for member, path in members:
        info = os.stat(path)
        folder = stat.S_ISDIR(info.st_mode)
        entry = zipfile.ZipInfo(member + "/" if folder else member,
                                fixed or time.localtime(info.st_mtime)[:6])
        entry.create_system = 3
        if folder:
            entry.external_attr = (stat.S_IFDIR | 0o755) << 16 | 0x10
            package.writestr(entry, b"")
            continue
        mode = 0o755 if info.st_mode & 0o111 else 0o644
        entry.external_attr = (stat.S_IFREG | mode) << 16
        entry.compress_type = zipfile.ZIP_DEFLATED
        entry.file_size = info.st_size
        with open(path, "rb") as source, package.open(entry, "w") as sink:
            shutil.copyfileobj(source, sink, 1024 * 1024)
PY
}
