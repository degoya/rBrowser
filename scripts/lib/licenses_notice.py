"""The NOTICE files of the packages from one source (RB-099-12).

packaging/NOTICE.in is rendered once per platform into packaging/<platform>/NOTICE, which
scripts/package-windows.sh, scripts/package-linux.sh and scripts/package-macos.sh put into the
package. A line of the source is in every file; a line that starts with a tag, `[windows] `,
`[linux,macos] ` (the platforms, comma-separated, then one space), only in those platforms'
files, without the tag (a tag alone is an empty line there); a line that starts with `#` is a
comment for the maintainers and in no file. The placeholders `@CEF_VERSION@`, `@CEF_COMMIT@`
and `@CHROMIUM_VERSION@` (RB-099-13: the revisions the written source offer names) are filled
from the CEF pin in scripts/lib/tool-versions.sh, so a CEF update leaves the files stale until
they are rendered again; every `RB_CEF_ARCHIVE*` pin must name the same build. An unknown
placeholder, no pin or pins of two builds stop the run. scripts/tests/licenses-notice.sh holds
the three files to the source.

    python3 scripts/lib/licenses_notice.py <repo root> [--check]

Exit 0 when written (or, with --check, current); 1 when --check finds a file stale; 2 on an
unknown platform or placeholder, a CEF pin that does not read, or an input that does not read.
"""

import re
import sys
from pathlib import Path

SOURCE = "packaging/NOTICE.in"
PLATFORMS = ("windows", "linux", "macos")
TAG = re.compile(r"^\[([a-z,]+)\](?: |$)")
PINS = "scripts/lib/tool-versions.sh"
PLACEHOLDER = re.compile(r"@([A-Z][A-Z_]*)@")
PLACEHOLDERS = ("CEF_VERSION", "CEF_COMMIT", "CHROMIUM_VERSION")
# `RB_CEF_ARCHIVE...="cef_binary_<cef version>+g<commit>+chromium-<chromium version>_<platform>..."`.
ARCHIVE_PIN = re.compile(r'^(RB_CEF_ARCHIVE[A-Z0-9_]*)="([^"]*)"', re.MULTILINE)
ARCHIVE = re.compile(r"^cef_binary_([0-9.]+)\+g([0-9a-f]+)\+chromium-([0-9.]+)_")


def cef_build(pins: str) -> dict[str, str]:
    """The placeholders' values from the CEF pins; raises ValueError unless they name one build."""
    builds = {}
    for name, archive in ARCHIVE_PIN.findall(pins):
        if name.endswith("_SHA256"):
            continue
        parsed = ARCHIVE.match(archive)
        if parsed is None:
            raise ValueError(f"{PINS}: {name} does not read as a CEF archive: {archive}")
        builds[name] = parsed.groups()
    if not builds:
        raise ValueError(f"{PINS}: no RB_CEF_ARCHIVE pin")
    if len(set(builds.values())) > 1:
        raise ValueError(f"{PINS}: the RB_CEF_ARCHIVE pins name two CEF builds: {sorted(builds)}")
    cef, commit, chromium = next(iter(builds.values()))
    return {"CEF_VERSION": cef, "CEF_COMMIT": commit, "CHROMIUM_VERSION": chromium}


def fill(source: str, root: Path) -> str:
    """`source` with its placeholders filled; the pins are read only when it has one."""
    names = set(PLACEHOLDER.findall(source))
    for name in sorted(names - set(PLACEHOLDERS)):
        raise ValueError(f"{SOURCE}: unknown placeholder @{name}@")
    if not names:
        return source
    values = cef_build((root / PINS).read_text(encoding="utf-8"))
    return PLACEHOLDER.sub(lambda match: values[match.group(1)], source)


def render(source: str, platform: str) -> str:
    """The NOTICE of `platform`; raises ValueError on an unknown platform in a tag."""
    lines = []
    for number, line in enumerate(source.split("\n"), start=1):
        if line.startswith("#"):
            continue
        tag = TAG.match(line)
        if tag is None:
            lines.append(line)
            continue
        named = tag.group(1).split(",")
        for name in named:
            if name not in PLATFORMS:
                raise ValueError(f"{SOURCE}:{number}: unknown platform '{name}'")
        if platform in named:
            lines.append(line[tag.end():])
    return "\n".join(lines).rstrip("\n") + "\n"


def main(argv: list[str]) -> int:
    if len(argv) < 2 or any(arg != "--check" for arg in argv[2:]):
        print("usage: licenses_notice.py <repo root> [--check]", file=sys.stderr)
        return 2
    root = Path(argv[1])
    try:
        source = fill((root / SOURCE).read_text(encoding="utf-8"), root)
        outputs = {f"packaging/{platform}/NOTICE": render(source, platform) for platform in PLATFORMS}
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    check = "--check" in argv[2:]
    stale = []
    for name, text in outputs.items():
        path = root / name
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current == text:
            continue
        if check:
            stale.append(name)
            print(f"{name} is stale: run scripts/licenses.sh (the source is {SOURCE})", file=sys.stderr)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
            print(f"==> wrote {name}")
    if stale:
        return 1
    if check:
        print(f"==> the NOTICE files are current with {SOURCE}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
