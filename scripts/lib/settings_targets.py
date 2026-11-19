"""The settings targets of notices (RB-090-85): every `settings_key` a notice of the core or a
plugin carries, and every `openSettings { key }` a page sends, opens a place in the settings
window - a key, a category, a section or the area of a key prefix.

The resolution is the core's (`crates/rbrowser/src/settings_window/target.rs`), in this order:

1. a registered key opens its field;
2. a category id (`capture`) opens the category;
3. a section id (`sitemap`, `env-compare`) opens the section, the first category in
   navigation order that has a section of that id;
4. a key prefix (`pageApi` for `pageApi.enabled` and `pageApi.uaSuffix`) opens the one section
   all its keys sit in, else the one category they share; a prefix across categories opens
   nothing.

The targets are read from the sources, not run: in Rust (`crates/`, `plugins/*/src/`, tests
excluded) `settings_key: Some(<target>)`, `warning_notification(..., Some(<target>))`, the
`Some(<target>)` arms of a `fn settings_key` and `"settings.open"` with `"key": <target>`, where
`<target>` is a string literal or a constant (`SETTINGS_KEY`, `rb_config::bookmarks::BOOKMARKS_KEY`)
defined as `const NAME: &str = "..."` in the same file, else anywhere in the tree; a lower-case
name is a value only known at run time and is skipped. In TypeScript and Vue (`ui/src/`,
`plugins/*/web/`, `plugins/*/panel/`, tests excluded) `type: 'openSettings', key: '<target>'` and
`settingsKey: '<target>'`. Used by scripts/lib/settings_lint.py.
"""

import re
from pathlib import Path

RUST_DIRS = ("crates", "plugins")
TS_DIRS = ("ui/src", "plugins")
SKIP_PARTS = {"node_modules", "target", "dist", "tests", "test", "fixtures"}

TARGET = r'(?:"(?P<lit>[^"\\]*)"|(?P<const>(?:[a-z_][a-z0-9_]*::)*[A-Z][A-Z0-9_]*))'
RUST_PATTERNS = [
    re.compile(r"settings_key:\s*Some\(\s*" + TARGET),
    re.compile(r"warning_notification\(\s*[^,()]+,\s*Some\(\s*" + TARGET),
    re.compile(r'"settings\.open",\s*json!\(\{\s*"key":\s*' + TARGET),
]
SETTINGS_KEY_FN = re.compile(r"fn settings_key\(&self\)[^{]*\{(?P<body>.*?)\n    \}", re.S)
SOME_IN_FN = re.compile(r"Some\(\s*" + TARGET + r"\s*\)")
CONST = re.compile(r'const (?P<name>[A-Z][A-Z0-9_]*): &str = "(?P<value>[^"\\]*)";')

TS_PATTERNS = [
    re.compile(r"type:\s*'openSettings',\s*key:\s*'(?P<lit>[^']*)'"),
    re.compile(r"settingsKey:\s*'(?P<lit>[^']*)'"),
]


def _sources(root: Path, dirs: tuple, suffixes: tuple) -> list[Path]:
    found = []
    for top in dirs:
        base = root / top
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if path.suffix not in suffixes or not path.is_file():
                continue
            parts = set(path.relative_to(root).parts)
            name = path.name
            if parts & SKIP_PARTS or name.endswith("_tests.rs") or ".test." in name:
                continue
            found.append(path)
    return found


def _constants(files: list[Path]) -> dict:
    """The `&str` constants of each file."""
    return {
        path: {m["name"]: m["value"] for m in CONST.finditer(path.read_text(encoding="utf-8", errors="replace"))}
        for path in files
    }


def _crate_of(root: Path, path: Path) -> Path:
    """The crate or plugin directory `path` lies in (`crates/<name>`, `plugins/<name>`)."""
    parts = path.relative_to(root).parts
    return root.joinpath(*parts[:2])


def _const_values(root: Path, path: Path, const: str, per_file: dict) -> set:
    """The values the constant `const` used in `path` may have: the nearest definitions - the
    file, its directory, its crate, the crate a path names (`rb_config::...`), the tree."""
    *segments, name = const.split("::")
    crate = _crate_of(root, path)
    if segments and segments[0] not in ("crate", "super", "self"):
        crate = root / "crates" / segments[0].replace("_", "-")
    tiers = [
        lambda p: p == path,
        lambda p: p.parent == path.parent,
        lambda p: crate in p.parents,
        lambda p: True,
    ]
    if segments and segments[0] not in ("crate", "super", "self"):
        tiers = tiers[2:]
    for tier in tiers:
        values = {own[name] for p, own in per_file.items() if name in own and tier(p)}
        if values:
            return values
    return set()


def _line(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def targets(root: Path) -> list[tuple[str, str]]:
    """Every settings target of the tree as (where, target); `where` is `file:line`, and a
    constant that does not read is the target `!<name>`."""
    rust = _sources(root, RUST_DIRS, (".rs",))
    per_file = _constants(rust)
    found = []

    def add(path, text, offset, match):
        where = f"{path.relative_to(root)}:{_line(text, offset)}"
        if match["lit"] is not None:
            found.append((where, match["lit"]))
            return
        values = _const_values(root, path, match["const"], per_file)
        name = match["const"].rsplit("::", 1)[-1]
        if not values:
            found.append((where, f"!{name}"))
        found.extend((where, value) for value in sorted(values))

    for path in rust:
        text = path.read_text(encoding="utf-8", errors="replace")
        for pattern in RUST_PATTERNS:
            for match in pattern.finditer(text):
                add(path, text, match.start(), match)
        for body in SETTINGS_KEY_FN.finditer(text):
            for match in SOME_IN_FN.finditer(body["body"]):
                add(path, text, body.start("body") + match.start(), match)
    for path in _sources(root, TS_DIRS, (".ts", ".vue")):
        text = path.read_text(encoding="utf-8", errors="replace")
        for pattern in TS_PATTERNS:
            for match in pattern.finditer(text):
                found.append((f"{path.relative_to(root)}:{_line(text, match.start())}", match["lit"]))
    return found


def resolve(target: str, settings: list, cats: list) -> str | None:
    """Where `target` opens the settings: `key`, `category`, `section <category>.<id>`,
    `prefix ...`; `None` for nowhere."""
    if any(setting.key == target for setting in settings):
        return "key"
    if any(category == target for category, _ in cats):
        return "category"
    for category, sections in cats:
        if target in sections:
            return f"section {category}.{target}"
    under = [s for s in settings if s.key.startswith(f"{target}.")]
    places = {(s.category, s.section) for s in under}
    if len(places) == 1:
        category, section = next(iter(places))
        return f"prefix section {category}.{section}"
    if under and len({category for category, _ in places}) == 1:
        return f"prefix category {under[0].category}"
    return None


def problems(root: Path, settings: list, cats: list) -> tuple[list[str], int]:
    """The targets that open nothing, one line each, and how many targets were read."""
    found = targets(root)
    lines = []
    for where, target in found:
        if target.startswith("!"):
            lines.append(f"{where}: settings target: the constant {target[1:]} does not read")
        elif resolve(target, settings, cats) is None:
            lines.append(
                f"{where}: settings target `{target}` is no key, category, section or key prefix"
            )
    return lines, len(found)
