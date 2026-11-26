#!/usr/bin/env bash
#
# scripts/package-msi.sh (RB-099-01) without WiX: --sources-only against a fake unpacked package.
# The generated files.wxs names every file of the package once, under its own name and from its
# own path (a `$` escaped for the WiX preprocessor), one component per folder with an HKCU key
# path and a RemoveFolder, ids unique and every GUID the same in a second run; install-kind says
# `msi`; license.rtf escapes RTF's special characters; the WiX arguments carry the MSI version
# (a pre-release counts as its three numbers), the culture and packaging/windows/msi/. A package
# without a version, with one MSI cannot hold, without rbrowser.exe, with the portable marker or
# with an install-kind of its own is refused; without --sources-only a missing `wix` is named.
# Every !(loc.…) of rbrowser.wxs is in en-US.wxl. scripts/package-windows.sh --msi without `wix`
# stops before anything is built.
#
# Pure bash and python3, a second.
#
#   scripts/tests/windows-paket-msi.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

STAGE="$SCRATCH/rbrowser-1.2.3-pre.4-windows-x64"
OUT="$SCRATCH/sources"
make_stage() {
    rm -rf "$STAGE"
    mkdir -p "$STAGE/locales" "$STAGE/sidecar/node_modules/a/b" "$STAGE/empty"
    printf 'exe\n' > "$STAGE/rbrowser.exe"
    printf '%s\n' "rBrowser ${1:-1.2.3-pre.4}" "commit 1a2b3c4d" > "$STAGE/VERSION.txt"
    printf '%s\n' 'GNU GENERAL PUBLIC LICENSE {3} \ end' > "$STAGE/LICENSE"
    printf 'pak\n' > "$STAGE/locales/en-US.pak"
    printf 'js\n' > "$STAGE/sidecar/node_modules/a/b/c.js"
    printf 'odd\n' > "$STAGE/sidecar/node_modules/a/we\$ird & <name>.txt"
}
sources() { run_status "$ROOT/scripts/package-msi.sh" --sources-only "$STAGE" "$OUT"; }

make_stage
sources
expect_status "the sources of a package are written" 0
for file in files.wxs license.rtf install-kind wix-arguments.txt; do
    expect_true "with $file" '[[ -s "$OUT/'"$file"'" ]]'
done
expect "install-kind says msi" "msi" "$(cat "$OUT/install-kind")"
expect_true "the licence page escapes RTF's braces and backslash" \
    'grep -qF "GNU GENERAL PUBLIC LICENSE \{3\} \\\\ end\par" "$OUT/license.rtf"'
expect_true "the MSI version is the release's three numbers" 'grep -qx "Version=1.2.3" "$OUT/wix-arguments.txt"'
expect_true "the culture is en-US" 'grep -qx "en-US" "$OUT/wix-arguments.txt"'
expect_true "the strings come from en-US.wxl" 'grep -q "packaging/windows/msi/en-US.wxl$" "$OUT/wix-arguments.txt"'
expect_true "the source is rbrowser.wxs" 'grep -q "packaging/windows/msi/rbrowser.wxs$" "$OUT/wix-arguments.txt"'

run_status python3 -I - "$OUT/files.wxs" "$STAGE" << 'PY'
import os
import sys
import xml.etree.ElementTree as ET

ns = {"w": "http://wixtoolset.org/schemas/v4/wxs"}
root = ET.parse(sys.argv[1]).getroot()
stage = sys.argv[2]
errors = []
expected = sorted(os.path.relpath(os.path.join(folder, name), stage).replace(os.sep, "\\")
                  for folder, _, names in os.walk(stage) for name in names)
sources = sorted(f.get("Source").replace("$$", "$").removeprefix("$(var.Stage)\\")
                 for f in root.iter("{%s}File" % ns["w"]))
if sources != expected:
    errors.append(f"files: {sources} != {expected}")
for f in root.iter("{%s}File" % ns["w"]):
    if f.get("Name") != f.get("Source").replace("$$", "$").rsplit("\\", 1)[-1]:
        errors.append(f"name {f.get('Name')} of {f.get('Source')}")
ids = [e.get("Id") for e in root.iter() if e.get("Id")]
if len(ids) != len(set(ids)):
    errors.append("an id is used twice")
folders = sum(1 for folder, _, _ in os.walk(stage))
components = list(root.iter("{%s}Component" % ns["w"]))
if len(components) != folders:
    errors.append(f"{len(components)} components for {folders} folders")
for component in components:
    keys = [r for r in component.iter("{%s}RegistryValue" % ns["w"]) if r.get("KeyPath") == "yes"]
    if len(keys) != 1 or keys[0].get("Root") != "HKCU":
        errors.append(f"component {component.get('Id')} has no HKCU key path")
    if not list(component.iter("{%s}RemoveFolder" % ns["w"])):
        errors.append(f"component {component.get('Id')} removes no folder")
    if not component.get("Guid"):
        errors.append(f"component {component.get('Id')} has no Guid")
print("\n".join(errors))
sys.exit(1 if errors else 0)
PY
expect_status "files.wxs names every file once, one component per folder" 0
cp "$OUT/files.wxs" "$SCRATCH/first.wxs"
sources
expect "a second run writes the same files.wxs, GUIDs included" \
    "$(sha256sum < "$SCRATCH/first.wxs")" "$(sha256sum < "$OUT/files.wxs")"

make_stage 1.2.3
sources
expect_true "a release keeps its version" 'grep -qx "Version=1.2.3" "$OUT/wix-arguments.txt"'
make_stage "next"
sources
expect_status "a VERSION.txt without a version is refused" 1
expect_output "and named" "names no version"
make_stage 256.0.0
sources
expect_status "a version an MSI cannot hold is refused" 1
make_stage
rm "$STAGE/rbrowser.exe"
sources
expect_status "a folder without rbrowser.exe is refused" 1
expect_output "and named" "rbrowser.exe"
make_stage
: > "$STAGE/portable"
sources
expect_status "the portable marker is refused" 1
make_stage
printf 'scoop\n' > "$STAGE/install-kind"
sources
expect_status "a package with an install-kind of its own is refused" 1

make_stage
run_status env PATH="/usr/bin:/bin" "$ROOT/scripts/package-msi.sh" "$STAGE" "$SCRATCH/out.msi"
expect_status "without --sources-only a missing wix stops the run" 1
expect_output "and is named" "wix"

run_status python3 -I - "$ROOT/packaging/windows/msi/rbrowser.wxs" "$ROOT/packaging/windows/msi/en-US.wxl" << 'PY'
import re
import sys
import xml.etree.ElementTree as ET

ET.parse(sys.argv[1])
wxl = ET.parse(sys.argv[2]).getroot()
defined = {s.get("Id") for s in wxl if s.tag.endswith("String")}
used = set(re.findall(r"!\(loc\.([A-Za-z0-9_]+)\)", open(sys.argv[1], encoding="utf-8").read()))
missing = sorted(used - defined)
print(" ".join(missing))
sys.exit(1 if missing or wxl.get("Culture") != "en-US" else 0)
PY
expect_status "every string of rbrowser.wxs is in en-US.wxl" 0

# scripts/package-windows.sh --msi holds `wix` before the build: a scratch repository whose
# scripts/build-plugins.sh notes that it was reached.
TREE="$SCRATCH/repo"
mkdir -p "$TREE/scripts/lib"
cp "$ROOT/scripts/package-windows.sh" "$ROOT/scripts/package-msi.sh" "$TREE/scripts/"
cp "$ROOT/scripts/lib/jobs.sh" "$ROOT/scripts/lib/lock.sh" "$ROOT/scripts/lib/package.sh" \
    "$ROOT/scripts/lib/workspace-version.sh" "$ROOT/scripts/lib/cef.sh" \
    "$ROOT/scripts/lib/tool-versions.sh" "$ROOT/scripts/lib/package-stage.sh" \
    "$ROOT/scripts/lib/wsl-helper.sh" "$TREE/scripts/lib/"
printf '%s\n' '#!/usr/bin/env bash' 'echo reached > "$RB_FAKE_REACHED"' 'exit 3' > "$TREE/scripts/build-plugins.sh"
printf '%s\n' '#!/usr/bin/env bash' 'exit 0' > "$TREE/scripts/set-version.sh"
printf '%s\n' '[workspace]' '' '[workspace.package]' 'version = "1.2.3"' > "$TREE/Cargo.toml"
printf '%s\n' '/artifacts/' > "$TREE/.gitignore"
chmod +x "$TREE"/scripts/*.sh
git -C "$TREE" init -q
git -C "$TREE" add -A
git -C "$TREE" -c user.name=test -c user.email=test@example.invalid commit -qm tree
run_status env PATH="/usr/bin:/bin" RB_NO_LOCK=1 RB_MIN_FREE_MB=0 RB_NO_SCCACHE=1 \
    RB_FAKE_REACHED="$SCRATCH/reached" "$TREE/scripts/package-windows.sh" --msi
expect_status "package-windows.sh --msi without wix stops" 1
expect_output "naming WiX" "wix"
expect_true "before anything is built" '[[ ! -e "$SCRATCH/reached" ]]'
run_status env PATH="/usr/bin:/bin" RB_NO_LOCK=1 RB_MIN_FREE_MB=0 RB_NO_SCCACHE=1 \
    RB_FAKE_REACHED="$SCRATCH/reached" "$TREE/scripts/package-windows.sh"
expect_status "without --msi the run goes on to the build" 3

finish_tests "windows-paket-msi"
