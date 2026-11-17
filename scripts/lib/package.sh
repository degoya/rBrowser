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
#   rb_package_zip "$OUT" "<stage name>" "$ZIP"
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

# Zips the folder $2 of directory $1 into $3 (an absolute path), the folder at the top: with
# `zip` where it is installed, else with Python's zipfile, which keeps the files' modes - the
# Windows runner's Git Bash has no `zip`. A folder that carries the portable mode's
# marker or data is refused first (rb_package_unportable).
rb_package_zip() {
    local dir="$1" name="$2" zip="$3" python
    rb_package_unportable "$dir/$name" || return 1
    if command -v zip > /dev/null 2>&1; then
        (cd "$dir" && zip -qry "$zip" "$name")
        return
    fi
    python="$(command -v python3 || command -v python)" || { echo "!! neither zip nor python" >&2; return 1; }
    "$python" -I - "$dir" "$name" "$zip" << 'PY'
import os
import sys
import zipfile

base, name, target = sys.argv[1], sys.argv[2], sys.argv[3]
with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as package:
    for folder, dirs, files in os.walk(os.path.join(base, name)):
        dirs.sort()
        for entry in sorted(dirs) + sorted(files):
            path = os.path.join(folder, entry)
            package.write(path, os.path.relpath(path, base).replace(os.sep, "/"))
PY
}
