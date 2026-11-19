"""Generates the wiki pages that come from the repository's data (scripts/export-wiki.sh).

So far one page: reference/configuration.md, the file format - where the files live, how they
merge, and every property of the project config (schema/config.v1.json) and of the user
settings (schema/settings.v1.json) in file order, nested properties, list entries and map values
included, with the settings window's place of each settings key (x-category, x-section). The
window's own reference, ordered like the window, is reference/settings.md
(scripts/settings-reference.sh, RB-010-02). The example is the plan's example config, the test
fixture crates/rb-config/tests/fixtures/project/.rbrowser/config.json, which the rb-config tests
load.

Compiles nothing: it reads the checked-in schemas, which a rb-config test holds to the types.

    export_wiki.py ROOT WIKI            write the pages
    export_wiki.py ROOT WIKI --check    exit 1 when a page is stale or missing
    export_wiki.py ROOT WIKI --dry-run  name what would change, write nothing

Exit 2 when the data does not read (a schema or the example is missing or not JSON). The
annotations themselves are the settings lint's (scripts/check-settings.sh).
"""

import json
import re
import sys
from pathlib import Path

PROJECT_SCHEMA = "schema/config.v1.json"
USER_SCHEMA = "schema/settings.v1.json"
CATALOGUE = "locales/en.json"
PARTS = "locales/parts"
EXAMPLE = "crates/rb-config/tests/fixtures/project/.rbrowser/config.json"
PAGE = "reference/configuration.md"

# The titles of docs/settings.md; locales/en.json (settings.category.<id>) wins once it has them.
CATEGORY_TITLES = {
    "general": "General",
    "project": "Project",
    "breakpoints": "Breakpoints & Devices",
    "layout": "Layout",
    "sync": "Sync",
    "emulation": "Emulation",
    "engines": "Engines",
    "devtools": "DevTools & Debugging",
    "reload": "Reload & Files",
    "network": "Network",
    "capture": "Capture",
    "checks": "Checks",
    "automation": "Automation",
    "security": "Security & Privacy",
    "plugins": "Plugins",
    "shortcuts": "Keyboard Shortcuts",
    "advanced": "Advanced",
    "about": "About",
}

# Section ids whose plain capitalisation reads wrong.
SECTION_TITLES = {
    "start-url": "Start URL",
    "mcp": "MCP",
    "cdp-proxy": "CDP proxy",
    "seo": "SEO",
    "pwa": "PWA",
    "wsl-watcher": "WSL watcher",
    "devtools": "DevTools",
    "geo-time-cpu": "Geolocation, time zone and CPU",
}

class DataError(Exception):
    """The repository's data does not read; exit 2."""


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise DataError(f"cannot read {path}: {error}") from error


def catalogue_text(catalogue, key):
    node = catalogue
    for segment in key.split("."):
        if not isinstance(node, dict) or segment not in node:
            return None
        node = node[segment]
    return node if isinstance(node, str) else None


def walk(schema, path, out):
    """Every property schema with its display key (`breakpoints[].width`, `presets.<name>`)."""
    if not isinstance(schema, dict):
        return
    required = schema.get("required") or []
    for key, child in (schema.get("properties") or {}).items():
        child_path = f"{path}.{key}" if path else key
        out.append((child_path, child, key in required))
        walk(child, child_path, out)
    if isinstance(schema.get("items"), dict):
        walk(schema["items"], f"{path}[]", out)
    if isinstance(schema.get("additionalProperties"), dict):
        walk(schema["additionalProperties"], f"{path}.<name>", out)
    for branches in ("anyOf", "oneOf", "allOf"):
        for branch in schema.get(branches) or []:
            walk(branch, path, out)


def type_of(schema):
    if not isinstance(schema, dict):
        return "any"
    if "const" in schema:
        return f"`{json.dumps(schema['const'])}`"
    if "enum" in schema:
        values = [v for v in schema["enum"] if v is not None]
        return "one of " + ", ".join(f"`{json.dumps(v)}`" for v in values)
    for branches in ("anyOf", "oneOf"):
        if branches in schema:
            kinds = [type_of(b) for b in schema[branches]
                     if b.get("type") != "null" and b.get("deprecated") is not True]
            return " or ".join(dict.fromkeys(kinds))
    kinds = schema.get("type")
    if kinds is None:
        return "any"
    if isinstance(kinds, str):
        kinds = [kinds]
    kinds = [k for k in kinds if k != "null"]
    named = []
    for kind in kinds:
        if kind == "array":
            named.append(f"array of {type_of(schema.get('items'))}")
        elif kind == "object" and isinstance(schema.get("additionalProperties"), dict):
            if not schema.get("properties"):
                named.append(f"map of {type_of(schema['additionalProperties'])}")
            else:
                named.append("object")
        else:
            named.append(kind)
    return " or ".join(named) or "any"


def default_of(schema):
    if "default" not in schema or isinstance(schema["default"], dict):
        return ""
    text = json.dumps(schema["default"], ensure_ascii=False)
    return f"`{text}`" if len(text) <= 40 else "(see schema)"


def clean(text):
    """A description as one table cell: rustdoc links unwrapped, one line, pipes escaped."""
    text = re.sub(r"\[(`[^`]+`)\](\([^)]*\))?", r"\1", text or "")
    text = text.replace("`crate::", "`")
    text = " ".join(text.split())
    return text.replace("|", "\\|")


def catalogue(root):
    """locales/en.json with the wave parts (locales/parts/*.json) merged in, nested."""
    merged = {}

    def merge(into, tree):
        for name, value in tree.items():
            if isinstance(value, dict) and isinstance(into.get(name), dict):
                merge(into[name], value)
            else:
                into[name] = value

    for path in [root / CATALOGUE] + sorted((root / PARTS).glob("*.json")):
        if path.is_file():
            data = read_json(path)
            if isinstance(data, dict):
                merge(merged, data)
    return merged


def place(prop, texts):
    """Where the settings window shows a key: `Category › Section`, plus its rules."""
    category, section = prop.get("x-category"), prop.get("x-section")
    if not category or not section:
        return ""
    category_title = catalogue_text(texts, f"settings.category.{category}")
    section_title = catalogue_text(texts, f"settings.section.{category}.{section}")
    text = (
        f"{category_title or CATEGORY_TITLES.get(category, category)} › "
        f"{section_title or SECTION_TITLES.get(section, section.replace('-', ' ').capitalize())}"
    )
    if prop.get("x-local-only"):
        text += " (local only)"
    if prop.get("x-protected"):
        text += " (protected from MCP)"
    return text


def table(schema, texts):
    lines = ["| Key | Type | Default | Settings | Description |", "| --- | --- | --- | --- | --- |"]
    out = []
    walk(schema, "", out)
    for key, prop, required in out:
        kind = type_of(prop) + (", required" if required else "")
        lines.append(
            f"| `{key}` | {kind} | {default_of(prop)} | {place(prop, texts)} | "
            f"{clean(prop.get('description', ''))} |"
        )
    return "\n".join(lines) + "\n"


def compact(value, indent=0, width=96):
    """JSON with two-space indentation where a value fits on one line it stays on one line."""
    flat = json.dumps(value, ensure_ascii=False, separators=(", ", ": "))
    if len(flat) + indent <= width or not isinstance(value, (dict, list)) or not value:
        return flat
    pad = " " * (indent + 2)
    if isinstance(value, dict):
        items = [f"{pad}{json.dumps(k)}: {compact(v, indent + 2, width)}" for k, v in value.items()]
        return "{\n" + ",\n".join(items) + "\n" + " " * indent + "}"
    items = [f"{pad}{compact(v, indent + 2, width)}" for v in value]
    return "[\n" + ",\n".join(items) + "\n" + " " * indent + "]"


HEADER = """\
<!-- Generated by scripts/export-wiki.sh from {project} and {user}; do not edit by hand. -->

# Configuration

The file format of rBrowser's configuration: where the files live, how they combine, and every
key they may hold, generated from the JSON schemas.

## Where configuration lives

| File | Scope | Checked in | Holds |
| --- | --- | --- | --- |
| `.rbrowser/config.json` | Project | yes | Everything the team shares |
| `.rbrowser/config.local.json` | Local | no (git-ignored) | Overrides for this machine; local-only keys |
| `%APPDATA%\\rBrowser\\settings.json` | User | — | App settings and your shortcuts |

- `config.local.json` overrides `config.json` key by key. Lists whose entries have an `id`
  (breakpoints, environments) merge entry by entry, so a local file can change one field of one
  breakpoint; other lists are replaced.
- The files are plain JSON (no comments), written with two-space indentation in their own key
  order. `$schema` gives your editor completion and checks.
- `version` is the format version. A config of an older version is migrated in memory when it
  is opened; the file stays as it is until rBrowser saves it, and then the old file is kept as
  `config.json.bak-v<old version>`.
- The first time rBrowser writes a config it adds `.rbrowser/.gitignore` with
  `config.local.json`, `*.bak-v*` and `out/`.

## Local-only keys

Keys marked **Local only** take effect only in `config.local.json`. In `config.json` they are
ignored, with a warning when they are set, so a shared config can never switch off TLS checks or
route the whole team through a proxy.

## Keys that wait for workspace trust

`network.ignoreCertificateErrors`, `network.proxy` and `network.hostResolverRules` are not
applied from any project file yet - neither `config.json` nor `config.local.json`, which a
cloned repository can commit as well. Each one that is set gives a warning naming the key and
the file. They take effect once workspace trust exists (planned, 0.1).

## Workspace trust

*(planned, 0.1)* A project you have not trusted will load only breakpoints, layout, environments
and emulation. Flows, snippets, project plugins, mocks, header overrides, host resolver rules
and page scripts wait until you choose **Trust**.

## Example

```json
{example}
```

## Reading the tables

The keys are listed in file order. `[]` stands for every entry of a list, `<name>` for every
entry of a map. **Settings** names the place in the settings window of each key the window
edits (a list such as `breakpoints` is one field there); the window's own reference is
[Settings](settings.md). Keys marked *local only* belong in `config.local.json`, keys marked
*protected from MCP* cannot be changed by an AI agent.
"""


def render(root):
    texts = catalogue(root)
    project = read_json(root / PROJECT_SCHEMA)
    user = read_json(root / USER_SCHEMA)
    example = read_json(root / EXAMPLE)
    if not isinstance(example, dict):
        raise DataError(f"{EXAMPLE}: not a JSON object")
    example.pop("$schema", None)
    return "".join([
        HEADER.format(project=PROJECT_SCHEMA, user=USER_SCHEMA, example=compact(example)),
        "\n## Project config: `config.json` and `config.local.json`\n\n",
        table(project, texts),
        "\n## User settings: `settings.json`\n\n",
        table(user, texts),
    ])


def main(argv):
    if len(argv) < 3 or len(argv) > 4 or (len(argv) == 4 and argv[3] not in ("--check", "--dry-run")):
        print("usage: export_wiki.py ROOT WIKI [--check|--dry-run]", file=sys.stderr)
        return 2
    root, wiki = Path(argv[1]), Path(argv[2])
    mode = argv[3] if len(argv) == 4 else "--write"
    try:
        pages = {PAGE: render(root)}
    except DataError as error:
        print(f"export-wiki: {error}", file=sys.stderr)
        return 2
    if not wiki.is_dir():
        if mode == "--write":
            print(f"export-wiki: no wiki at {wiki} (set RB_WIKI)", file=sys.stderr)
            return 2
        print(f"export-wiki: no wiki at {wiki}; the data reads, the pages are not compared")
        return 0
    stale = []
    for name, text in pages.items():
        path = wiki / name
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current == text:
            print(f"    {name}: up to date")
            continue
        stale.append(name)
        if mode == "--write":
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
            print(f"    {name}: written")
        else:
            print(f"    {name}: {'missing' if current is None else 'stale'}"
                  f"{' (would write)' if mode == '--dry-run' else ''}")
    if mode == "--check" and stale:
        print("export-wiki: stale pages; run scripts/export-wiki.sh", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
