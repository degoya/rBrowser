"""The read rule of the settings (RB-094-30): every core settings key is read by some code.

A key in the schema that nothing reads is a switch without effect: the settings window shows it,
a change does nothing (`plugins.compileCache` and `ui.language` were found that way in RB-094-23).
This finds such keys from the sources, compiling nothing. A core key (schema/*.json,
schema/parts/*.json; plugin keys are their plugins' business) counts as read when one of these
appears outside the tests (`*_tests.rs`, `tests/`, `*.test.ts`, `test/`) and outside the key
tables of the annotations (crates/rb-config/src/annotate/):

- in Rust (crates/, plugins/): its last segment in snake_case as a field access, `.compile_cache`;
- in the front ends (ui/src, agent/src, sidecar/src): its last segment as written, `.compileCache`;
- anywhere in those sources: the dotted key as a string, `"plugins.compileCache"`.

It is a search, not a compiler: a key whose last segment another struct shares (`language`,
`enabled`) passes as read even when this key is not. What it catches is a key whose name nothing
uses at all. `PLACEHOLDERS` names the keys kept unread on purpose, each with its reason.

    python3 -I scripts/lib/settings_unread.py <repo root>

Exit 0 when every key is read, 1 with the unread keys named, 2 when the data does not read.
"""

import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import settings_schema  # noqa: E402
from settings_schema import DataError  # noqa: E402

# Keys that stay unread by the owner's decision, with the reason.
PLACEHOLDERS = {
    # Owner, 2026-11-22 (RB-094-30): a pick list with the one choice "English" until the
    # translations come in phase 2; nothing reads it before.
    "ui.language": "the UI language, English only until phase 2",
}

RUST_DIRS = ("crates", "plugins")
FRONT_DIRS = ("ui/src", "agent/src", "sidecar/src")
FRONT_SUFFIXES = (".ts", ".vue")
SKIPPED_PARTS = {"node_modules", "target", "dist", "tests", "test", "fixtures"}
KEY_TABLES = "crates/rb-config/src/annotate"


def snake(name: str) -> str:
    """`compileCache` -> `compile_cache`."""
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name).lower()


def counted(path: Path, root: Path) -> bool:
    """Whether a read in `path` counts: no test, no key table, nothing built or installed."""
    relative = path.relative_to(root)
    if SKIPPED_PARTS.intersection(relative.parts[:-1]):
        return False
    name = relative.name
    if name.endswith("_tests.rs") or ".test." in name or ".spec." in name:
        return False
    return not relative.as_posix().startswith(KEY_TABLES + "/")


def sources(root: Path, dirs: tuple, suffixes: tuple) -> str:
    """The text of every counted source file under `dirs`, joined."""
    texts = []
    for directory in dirs:
        base = root / directory
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if path.suffix in suffixes and path.is_file() and counted(path, root):
                texts.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(texts)


def unread(root: Path) -> tuple[list[str], int]:
    """The core keys no code reads, and how many core keys there are."""
    settings, issues = settings_schema.read_all(root)
    if issues:
        raise DataError(f"the settings schemas do not read: {issues[0]}")
    keys = sorted({setting.key for setting in settings if setting.plugin is None})
    rust = sources(root, RUST_DIRS, (".rs",))
    front = sources(root, FRONT_DIRS, FRONT_SUFFIXES)
    found = []
    for key in keys:
        if key in PLACEHOLDERS:
            continue
        leaf = key.rsplit(".", 1)[-1]
        quoted = re.compile(r"['\"`]" + re.escape(key) + r"['\"`]")
        read = (
            re.search(r"\." + re.escape(snake(leaf)) + r"\b", rust)
            or re.search(r"\." + re.escape(leaf) + r"\b", front)
            or quoted.search(rust)
            or quoted.search(front)
        )
        if not read:
            found.append(key)
    return found, len(keys)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: settings_unread.py <repo root>", file=sys.stderr)
        return 2
    root = Path(argv[1]).resolve()
    try:
        found, count = unread(root)
    except DataError as error:
        print(f"!! the settings data does not read: {error}", file=sys.stderr)
        return 2
    for key in found:
        print(
            f"{key}: no code reads this key - make it work, remove it (with a migration), or "
            "name it in PLACEHOLDERS (scripts/lib/settings_unread.py) with the reason"
        )
    summary = f"settings read: {count} core keys, {len(PLACEHOLDERS)} placeholder(s)"
    if found:
        print(f"!! {summary}: {len(found)} unread")
        return 1
    print(f"==> {summary}: every key is read")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
