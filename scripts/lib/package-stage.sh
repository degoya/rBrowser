# shellcheck shell=bash
#
# What the Windows package carries beyond the app folder of scripts/lib/cef.sh (RB-010-15): the
# front ends built, the pinned Node runtime, the sidecar, the console twin rbrowser.com,
# rbrowser.exe's icon and version resource, crash_reporter.cfg, and the size report. Every download is held to its SHA-256 pin in scripts/lib/tool-versions.sh and cached in
# the build cache, so a second package downloads nothing.
#
#   source "$ROOT/scripts/lib/package-stage.sh"
#   cache="$(rb_package_cache)"                           # ~/.cache/rbrowser/package
#   rb_fetch_pinned <url> <file> <sha256>                 # download once, keep only the pin
#   rb_build_frontend "$ROOT/agent"                       # npm ci --ignore-scripts, npm run build
#   rb_stage_node "$ROOT" "$cache" "$STAGE"               # node\node.exe and node\LICENSE
#   rb_stage_sidecar "$ROOT/sidecar" "$STAGE/sidecar"     # dist\{main,flow-worker}.js, node_modules
#   rb_sidecar_packages "$ROOT/sidecar/package-lock.json" # what it stages (a dry listing)
#   rb_prune_sidecar_extras "$STAGE/sidecar/node_modules" # no *.map, *.d.ts, *.md, *.txt
#   rb_stage_console "$build_dir" "$STAGE"                # rbrowser-console.exe as rbrowser.com
#   rb_patch_exe "$STAGE/rbrowser.exe" <ico> <version> "$cache"   # icon + version (rcedit)
#   rb_write_crash_reporter_cfg <template> "$STAGE" <version>
#   rb_package_sizes "$STAGE" "$ZIP"                      # MB per part
#
# Tests: scripts/tests/package-stage.sh (fakes for curl and rcedit), scripts/tests/package.sh
# (a fake npm).

# shellcheck source=scripts/lib/tool-versions.sh
source "$(dirname "${BASH_SOURCE[0]}")/tool-versions.sh"

# The package's download cache: $RB_PACKAGE_CACHE, else ~/.cache/rbrowser/package (shared by every
# worktree, beside the CEF cache).
rb_package_cache() {
    printf '%s\n' "${RB_PACKAGE_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/rbrowser/package}"
}

# Puts url $1 at file $2 when its SHA-256 is $3: a cached file is checked, not downloaded again;
# a download goes to $2.part first and is kept only with the pin. A cached file that no longer
# matches stops the run - it is never replaced silently.
rb_fetch_pinned() {
    local url="$1" file="$2" pin="$3" actual
    if [[ -f "$file" ]]; then
        actual="$(sha256sum "$file" | cut -d ' ' -f 1)"
        [[ "$actual" == "$pin" ]] && return 0
        echo "!! $file has SHA-256 $actual; the pin (scripts/lib/tool-versions.sh) is $pin." >&2
        echo "   remove $file and run again; if the source changed the file, check it first." >&2
        return 1
    fi
    mkdir -p "$(dirname "$file")"
    echo "==> downloading $url" >&2
    if ! curl -fsSL --proto '=https' --tlsv1.2 --retry 3 -o "$file.part" "$url"; then
        rm -f "$file.part"
        echo "!! could not download $url" >&2
        return 1
    fi
    actual="$(sha256sum "$file.part" | cut -d ' ' -f 1)"
    if [[ "$actual" != "$pin" ]]; then
        rm -f "$file.part"
        echo "!! $url has SHA-256 $actual; the pin (scripts/lib/tool-versions.sh) is $pin. Nothing was kept." >&2
        return 1
    fi
    mv "$file.part" "$file"
}

# Stages the Node runtime of checkout $1 into $3/node (node.exe and its LICENSE, which names the
# licences of everything Node bundles): the official win-x64 ZIP of .nvmrc's version, held to
# RB_NODE_WIN_X64_SHA256 and cached in $2. A pin for another version than .nvmrc's stops the run.
rb_stage_node() {
    local root="$1" cache="$2" stage="$3" version name zip
    version="$(rb_node_version "$root")"
    if [[ -z "$version" || "$RB_NODE_WIN_X64_VERSION" != "$version" ]]; then
        echo "!! the pin is for node $RB_NODE_WIN_X64_VERSION, .nvmrc says ${version:-nothing}:" >&2
        echo "   pin RB_NODE_WIN_X64_VERSION and _SHA256 in scripts/lib/tool-versions.sh" >&2
        echo "   (https://nodejs.org/dist/v$version/SHASUMS256.txt)" >&2
        return 1
    fi
    name="node-v$version-win-x64"
    zip="$cache/$name.zip"
    rb_fetch_pinned "https://nodejs.org/dist/v$version/$name.zip" "$zip" "$RB_NODE_WIN_X64_SHA256" \
        || return 1
    mkdir -p "$stage/node"
    python3 -I - "$zip" "$name" "$stage/node" << 'PY' || return 1
import os
import sys
import zipfile

archive, top, target = sys.argv[1], sys.argv[2], sys.argv[3]
with zipfile.ZipFile(archive) as package:
    for member in ("node.exe", "LICENSE"):
        with package.open(f"{top}/{member}") as source, open(os.path.join(target, member), "wb") as out:
            out.write(source.read())
PY
    echo "==> node $version staged (ZIP SHA-256 $RB_NODE_WIN_X64_SHA256)" >&2
}

# Builds the front end in folder $1 from its lock file: `npm ci` without the packages' install
# scripts (none of the three front ends needs one), then `npm run build`.
rb_build_frontend() {
    local dir="$1"
    echo "==> building $(basename "$dir")/" >&2
    (cd "$dir" && npm ci --ignore-scripts --no-audit --no-fund && npm run build)
}

# The sidecar's production dependencies that must be there, each staged with its LICENSE and
# NOTICE: playwright-core for the sidecar itself, @playwright/test and its playwright for the flow
# worker (RB-040-07), lighthouse and puppeteer-core for the Lighthouse module (RB-060-07). Their
# whole production tree is staged, but what only the packages of RB_SIDECAR_EXCLUDED need.
RB_SIDECAR_PACKAGES=(playwright-core playwright @playwright/test lighthouse puppeteer-core)
# Left out with what only they need: Lighthouse's error reporting (@sentry/node and its tree, the
# `sentry` package under the FSL), loaded only with `enableErrorReporting`, which the sidecar never
# sets (sidecar/src/modules/lighthouse/real.ts). A name ending in `/` is a scope.
RB_SIDECAR_EXCLUDED=(@sentry/ sentry)

# Prints the package paths (`node_modules/...`) of the production tree that lock file $1 names and
# rb_stage_sidecar stages: everything the root's dependencies reach, by Node's lookup, but the
# RB_SIDECAR_EXCLUDED packages and what only they reach. Reads the lock file alone - a dry listing.
rb_sidecar_packages() {
    python3 -I - "$1" "${RB_SIDECAR_EXCLUDED[@]}" << 'PY'
import json
import sys

lock, excluded = sys.argv[1], sys.argv[2:]
with open(lock, encoding="utf-8") as handle:
    packages = json.load(handle)["packages"]


def name_of(path):
    return path.rsplit("node_modules/", 1)[-1]


def left_out(name):
    return any(name.startswith(x) if x.endswith("/") else name == x for x in excluded)


def resolve(start, name):
    base = start
    while True:
        candidate = f"{base}/node_modules/{name}" if base else f"node_modules/{name}"
        if candidate in packages:
            return candidate
        if not base:
            return None
        cut = base.rfind("/node_modules/")
        base = base[:cut] if cut >= 0 else ""


def needs(path):
    entry = packages[path]
    return [*entry.get("dependencies", {}), *entry.get("optionalDependencies", {})]


staged, todo = set(), [resolve("", name) for name in needs("")]
while todo:
    path = todo.pop()
    if path is None or path in staged or left_out(name_of(path)):
        continue
    staged.add(path)
    todo.extend(resolve(path, name) for name in needs(path))
print("\n".join(sorted(staged)))
PY
}

# Removes from the staged node_modules $1 what no package needs at run time (RB-080-25): source
# maps, TypeScript declarations and the packages' prose - `*.map`, `*.d.ts`, `*.md`, `*.txt` -
# but never a file that names a licence or a notice, whatever its suffix (`LICENSE.md`,
# `NOTICE.txt`, `ThirdPartyNotices.txt`).
rb_prune_sidecar_extras() {
    find "$1" -type f \( -name '*.map' -o -name '*.d.ts' -o -name '*.md' -o -name '*.txt' \) \
        ! -iname '*license*' ! -iname '*licence*' ! -iname '*notice*' -delete
}

# Stages the sidecar of folder $1 (built: dist/main.js and dist/flow-worker.js) into $2: both
# bundles, its package.json (an ES module) and its production dependencies, installed with
# `npm ci --omit=dev` in a scratch folder (so the source tree keeps its development install) and
# held to rb_sidecar_packages: every other installed package and the `.bin` links are removed,
# then rb_prune_sidecar_extras takes the source maps, declarations and prose.
rb_stage_sidecar() {
    local src="$1" dest="$2" prod bundle pkg path keep
    for bundle in main.js flow-worker.js; do
        if [[ ! -s "$src/dist/$bundle" ]]; then
            echo "!! $src/dist/$bundle is missing: npm ci && npm run build in sidecar/" >&2
            return 1
        fi
    done
    prod="$(mktemp -d)"
    cp "$src/package.json" "$src/package-lock.json" "$prod/"
    if ! (cd "$prod" && npm ci --omit=dev --ignore-scripts --no-audit --no-fund) >&2; then
        rm -rf "$prod"
        echo "!! the sidecar's production dependencies did not install" >&2
        return 1
    fi
    for pkg in "${RB_SIDECAR_PACKAGES[@]}"; do
        if [[ ! -f "$prod/node_modules/$pkg/package.json" ]]; then
            rm -rf "$prod"
            echo "!! the sidecar's production install has no $pkg" >&2
            return 1
        fi
    done
    if ! keep="$(rb_sidecar_packages "$prod/package-lock.json")"; then
        rm -rf "$prod"
        echo "!! the sidecar's lock file does not read" >&2
        return 1
    fi
    rm -rf "$prod/node_modules/.bin"
    while IFS= read -r path; do
        [[ -n "$path" && -e "$prod/$path" ]] || continue
        grep -qxF "$path" <<< "$keep" || rm -rf "${prod:?}/$path"
    done < <(python3 -I -c 'import json, sys; print("\n".join(p for p, e in json.load(open(sys.argv[1], encoding="utf-8"))["packages"].items() if p))' "$prod/package-lock.json")
    rb_prune_sidecar_extras "$prod/node_modules"
    mkdir -p "$dest/dist"
    for bundle in main.js flow-worker.js; do
        install -m 644 "$src/dist/$bundle" "$dest/dist/$bundle"
    done
    install -m 644 "$src/package.json" "$dest/package.json"
    rm -rf "$dest/node_modules"
    cp -R "$prod/node_modules" "$dest/node_modules"
    rm -rf "$prod"
}

# The Windows form of path $1 for a Windows program: cygpath on Git Bash, wslpath in WSL.
rb_windows_path() {
    if command -v cygpath > /dev/null 2>&1; then
        cygpath -w "$1"
    elif command -v wslpath > /dev/null 2>&1; then
        wslpath -w "$1"
    else
        echo "!! neither cygpath nor wslpath: rcedit runs only on Windows or in WSL with interop" >&2
        return 1
    fi
}

# Stages the console twin of build output $1 (target/<triple>/<profile>, `-p rbrowser-cli`) into
# the app folder $2 as rbrowser.com (RB-010-11): rbrowser.exe is a GUI program a shell neither
# waits for nor shows the output of, and a bare `rbrowser` finds the .com first.
rb_stage_console() {
    local build="$1" app="$2"
    if [[ ! -f "$build/rbrowser-console.exe" ]]; then
        echo "!! $build has no rbrowser-console.exe: build -p rbrowser-cli for Windows first" >&2
        return 1
    fi
    install -m 755 "$build/rbrowser-console.exe" "$app/rbrowser.com"
}

# Writes the icon $2 and the version resource of version $3 into the executable $1 - CEF's
# bootstrap.exe, which has neither - with rcedit, downloaded into cache $4 and held to its pin.
# The strings are those build.rs gives rbrowser.dll.
rb_patch_exe() {
    local exe="$1" icon="$2" version="$3" cache="$4" rcedit
    rcedit="$cache/rcedit-x64.exe"
    rb_fetch_pinned "https://github.com/electron/rcedit/releases/download/v$RB_RCEDIT_VERSION/rcedit-x64.exe" \
        "$rcedit" "$RB_RCEDIT_SHA256" || return 1
    chmod +x "$rcedit"
    local win_exe win_icon
    win_exe="$(rb_windows_path "$exe")" || return 1
    win_icon="$(rb_windows_path "$icon")" || return 1
    if ! "$rcedit" "$win_exe" --set-icon "$win_icon" \
        --set-file-version "$version" --set-product-version "$version" \
        --set-version-string CompanyName "DEGOYA.digital" \
        --set-version-string FileDescription "rBrowser" \
        --set-version-string FileVersion "$version" \
        --set-version-string InternalName "rbrowser" \
        --set-version-string LegalCopyright "Copyright (c) DEGOYA.digital" \
        --set-version-string OriginalFilename "rbrowser.exe" \
        --set-version-string ProductName "rBrowser" \
        --set-version-string ProductVersion "$version"; then
        echo "!! rcedit failed on $exe (WSL: is interop on?)" >&2
        return 1
    fi
}

# Writes $2/crash_reporter.cfg from template $1 (packaging/windows/crash_reporter.cfg.in, which a
# test of rb-diag holds to rb_diag::crash_reporter_cfg) with version $3: Crashpad keeps its dumps
# local, there is no ServerURL.
rb_write_crash_reporter_cfg() {
    local template="$1" stage="$2" version="$3"
    sed "s/@VERSION@/$version/g" "$template" > "$stage/crash_reporter.cfg"
    if grep -q '^ServerURL' "$stage/crash_reporter.cfg"; then
        echo "!! $template names a ServerURL: crash dumps never leave the machine" >&2
        return 1
    fi
}

# The size report: MB per part of the stage $1 and, when given, of the ZIP $2.
rb_package_sizes() {
    python3 -I - "$1" "${2:-}" << 'PY'
import os
import sys

stage, archive = sys.argv[1], sys.argv[2]
folders = {"locales": "CEF locales", "ui": "shell UI", "node": "Node runtime",
           "plugins": "plugins", "sidecar": "sidecar", "licenses": "docs, notices, config"}
files = {"rbrowser.exe": "rbrowser.exe", "rbrowser.com": "rbrowser.com",
         "rbrowser.dll": "rbrowser.dll", "rbrowser.pdb": "rbrowser.dll",
         "rbrowser-wsl": "rbrowser-wsl"}
docs = {"LICENSE", "NOTICE", "README.txt", "CHANGELOG.md", "VERSION.txt", "crash_reporter.cfg"}
order = ["rbrowser.exe", "rbrowser.com", "rbrowser.dll", "rbrowser-wsl", "CEF runtime",
         "CEF locales", "shell UI", "Node runtime", "plugins", "sidecar", "docs, notices, config"]
sizes = {}
for folder, _, names in os.walk(stage):
    relative = os.path.relpath(folder, stage)
    for name in names:
        size = os.path.getsize(os.path.join(folder, name))
        if relative != ".":
            part = folders.get(relative.split(os.sep)[0], "CEF runtime")
        elif name in files:
            part = files[name]
        elif name in docs:
            part = "docs, notices, config"
        else:
            part = "CEF runtime"
        sizes[part] = sizes.get(part, 0) + size
mb = 1024 * 1024
print("==> size report")
for part in order:
    if part in sizes:
        print(f"{sizes[part] / mb:8.1f} MB  {part}")
print(f"{sum(sizes.values()) / mb:8.1f} MB  total, unpacked")
if archive:
    print(f"{os.path.getsize(archive) / mb:8.1f} MB  ZIP")
PY
}
