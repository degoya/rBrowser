"""The settings data without compiling, shared by the settings lint and the settings reference.

- The categories and sections are crates/rb-settings/src/category.rs's (`Category::ALL`, the ids
  of `Category::id`, the `SECTIONS` list), read from the Rust source.
- The settings are the `x-*` annotations of the JSON schemas: every `schema/*.json` and
  `schema/parts/*.json` (core keys), and the `[settings] schema` of every bundled plugin
  (`plugins/<dir>/manifest.toml`; keys `<plugin id>.<property>`).
- The texts are locales/en.json with the wave parts locales/parts/*.json merged in, and a
  plugin's own `[locales] en` catalogue for its keys.

The walk is the one of crates/rb-settings/src/annotate.rs; keep the two in step. A property with
any of x-category, x-section or x-order is a setting and needs all three plus x-scope; one
without them that resolves (`$ref`, a single `allOf`, an `anyOf`/`oneOf` with one non-null
branch) to an object with `properties` is a group the walk enters; anything else is a key
without annotations. The root's `$schema` and `version` are file structure.

The inputs (RB-080-20): `x-input` ("file" or "folder") sits on a string and names the native
dialog; a string key whose last segment ends in `path` or `dir` (any case) has to carry it.
`x-options` is a list of strings the window suggests, `"dynamic"` on a path whose
choices the core finds at runtime (RB-080-32), or the name of a source the core fills at runtime
(`OPTION_SOURCES`, RB-080-72) on a list (its items) or a map (its keys). Every key has a help text,
`x-help-key` or `<base>.help`, beside its label and description.

A secret key (`x-secret`, RB-094-26, RB-094-27) is no setting of the window; its `x-panel`,
`{"panel": <id>, "tab": <id>}`, names the panel of its plugin that edits it (a panel the plugin's
manifest lists), and only a secret key of a plugin carries one.
"""

import json
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

CATEGORY_RS = "crates/rb-settings/src/category.rs"
CATALOGUE = "locales/en.json"
PARTS = "locales/parts"
STRUCTURAL_KEYS = ("$schema", "version")
SCOPES = ("user", "project", "local")
INPUTS = ("file", "folder")
DYNAMIC_OPTIONS = "dynamic"
# The sources of known values the core fills in (`rbrowser::settings_window::dynamic::SOURCES`).
OPTION_SOURCES = (
    "plugins", "plugin-panels", "plugin-page-scripts", "check-rules", "environments", "breakpoints",
    "layouts"
)
# The sources whose values one string takes (a choice of one), not a list or a map's keys.
STRING_SOURCES = ("layouts",)
PATH_LEAF = re.compile(r"(path|dir)\Z", re.I)
MAX_DEPTH = 32
KEY_SEGMENT = re.compile(r"[A-Za-z][A-Za-z0-9_-]*\Z")
I18N_AREA = re.compile(r"[a-z][a-z0-9]*\Z")
I18N_SEGMENT = re.compile(r"[a-z0-9][A-Za-z0-9_-]*\Z")
PANEL_ID = re.compile(r"[a-z][a-z0-9-]{0,39}\Z")


class DataError(Exception):
    """The data does not read: a source is missing or not in the shape expected."""


@dataclass
class Setting:
    key: str
    category: str
    section: str
    order: int
    scopes: list
    local_only: bool
    protected: bool
    label_key: str
    description_key: str
    help_key: str
    schema: dict
    source: str
    plugin: str | None = None
    input: str | None = None
    # The whole schema the key sits in, for the `$ref`s of its nested values (RB-090-16).
    root: dict = field(default_factory=dict)
    # `x-secret` (RB-094-26), and the panel its `x-panel` names (RB-094-27), well formed.
    secret: bool = False
    panel: dict | None = None


@dataclass
class Plugin:
    id: str
    schema: Path
    catalogue: dict = field(default_factory=dict)
    # The ids of its `[[panels]]` (RB-094-27: a secret key's `x-panel` names one).
    panels: list = field(default_factory=list)


def categories(root: Path) -> list[tuple[str, list[str]]]:
    """The categories in navigation order, each with its section ids in display order."""
    path = root / CATEGORY_RS
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise DataError(f"{CATEGORY_RS}: {error.strerror}") from error
    ids = dict(re.findall(r'Category::(\w+) => "([a-z0-9-]+)"', text))
    all_list = re.search(r"pub const ALL: \[Category; (\d+)\] = \[(.*?)\];", text, re.S)
    if not all_list:
        raise DataError(f"{CATEGORY_RS}: `Category::ALL` not found")
    order = re.findall(r"Category::(\w+)", all_list.group(2))
    if len(order) != int(all_list.group(1)) or not order:
        raise DataError(f"{CATEGORY_RS}: `Category::ALL` lists {len(order)} categories")
    sections = re.findall(r's\(Category::(\w+), "([a-z0-9-]+)"\)', text)
    result = []
    for variant in order:
        if variant not in ids:
            raise DataError(f"{CATEGORY_RS}: Category::{variant} has no id")
        own = [section for owner, section in sections if owner == variant]
        if not own:
            raise DataError(f"{CATEGORY_RS}: Category::{variant} has no section")
        result.append((ids[variant], own))
    return result


def flatten(tree: object, prefix: str = "") -> dict:
    """A nested catalogue as dotted keys."""
    if not isinstance(tree, dict):
        return {prefix: tree} if prefix else {}
    flat = {}
    for name, value in tree.items():
        flat.update(flatten(value, f"{prefix}.{name}" if prefix else name))
    return flat


def load_json(path: Path, root: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise DataError(f"{relative(path, root)}: {error.strerror}") from error
    except json.JSONDecodeError as error:
        raise DataError(f"{relative(path, root)}: not JSON ({error})") from error


def catalogue(root: Path) -> dict:
    """The English catalogue with the wave's parts merged in, as dotted keys."""
    flat = {}
    files = [root / CATALOGUE] + sorted((root / PARTS).glob("*.json"))
    for path in files:
        if path.is_file():
            flat.update(flatten(load_json(path, root)))
    return flat


# Generated files in schema/ that are no settings schema: the command list the keyboard
# reference is written from (RB-010-17), the format of the check findings (RB-060-01), the MCP
# tool list of the tool reference (RB-030-08) and the recorder's file format (RB-050-01).
NOT_SETTINGS = {"commands.v1.json", "findings.v1.json", "mcp-tools.v1.json", "recording.v1.json"}


def schema_files(root: Path) -> list[Path]:
    """The core settings schemas: schema/*.json, then schema/parts/*.json."""
    core = [p for p in sorted((root / "schema").glob("*.json")) if p.name not in NOT_SETTINGS]
    return core + sorted((root / "schema" / "parts").glob("*.json"))


def plugins(root: Path) -> list[Plugin]:
    """The bundled plugins with a settings schema, by directory name."""
    found = []
    for manifest in sorted((root / "plugins").glob("*/manifest.toml")):
        try:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as error:
            raise DataError(f"{relative(manifest, root)}: {error}") from error
        schema = (data.get("settings") or {}).get("schema")
        if not schema:
            continue
        plugin_id = data.get("id")
        if not isinstance(plugin_id, str) or not KEY_SEGMENT.match(plugin_id):
            raise DataError(f"{relative(manifest, root)}: malformed plugin id {plugin_id!r}")
        locale = (data.get("locales") or {}).get("en")
        texts = flatten(load_json(manifest.parent / locale, root)) if locale else {}
        panels = [panel.get("id") for panel in data.get("panels") or [] if isinstance(panel, dict)]
        found.append(Plugin(plugin_id, manifest.parent / schema, texts, panels))
    return found


def relative(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def is_i18n_key(key: object) -> bool:
    if not isinstance(key, str):
        return False
    parts = key.split(".")
    return (
        len(parts) >= 2
        and bool(I18N_AREA.match(parts[0]))
        and all(I18N_SEGMENT.match(part) for part in parts[1:])
    )


def resolve(root: dict, node: object, depth: int = 0) -> object:
    """`node` with its `$ref`, single `allOf` or one non-null `anyOf`/`oneOf` branch followed."""
    if depth >= MAX_DEPTH or not isinstance(node, dict):
        return node
    ref = node.get("$ref")
    if isinstance(ref, str) and ref.startswith("#/"):
        target = root
        for part in ref[2:].split("/"):
            part = part.replace("~1", "/").replace("~0", "~")
            target = target.get(part) if isinstance(target, dict) else None
        if target is not None:
            return resolve(root, target, depth + 1)
    all_of = node.get("allOf")
    if isinstance(all_of, list) and len(all_of) == 1:
        return resolve(root, all_of[0], depth + 1)
    for combinator in ("anyOf", "oneOf"):
        branches = node.get(combinator)
        if isinstance(branches, list):
            real = [b for b in branches if not (isinstance(b, dict) and b.get("type") == "null")]
            if len(real) == 1:
                return resolve(root, real[0], depth + 1)
    return node


def own_schema(root: dict, prop: dict) -> dict:
    """The key's schema with a `$ref` replaced by its target's fields, its own over them."""
    if "$ref" not in prop:
        return prop
    target = resolve(root, prop)
    merged = dict(target) if isinstance(target, dict) else {}
    merged.update({name: value for name, value in prop.items() if name != "$ref"})
    return merged


def walk(schema: object, source: str, sections: dict, plugin: str | None = None):
    """The settings of one schema and its issues, `(key, message)`, in schema order."""
    settings, issues = [], []
    top = resolve(schema, schema) if isinstance(schema, dict) else None
    props = top.get("properties") if isinstance(top, dict) else None
    if not isinstance(props, dict):
        return settings, [("", "the schema has no properties")]
    for name, prop in props.items():
        if name not in STRUCTURAL_KEYS:
            _walk(schema, name, prop, source, sections, plugin, 0, settings, issues)
    return settings, issues


def _walk(root, path, prop, source, sections, plugin, depth, settings, issues):
    key = f"{plugin}.{path}" if plugin else path
    prop = prop if isinstance(prop, dict) else {}
    if any(name in prop for name in ("x-category", "x-section", "x-order")):
        setting, problems = read_setting(root, key, path, prop, source, sections, plugin)
        if problems:
            issues.extend((key, problem) for problem in problems)
        else:
            settings.append(setting)
        return
    group = resolve(root, prop) if depth < MAX_DEPTH else None
    children = group.get("properties") if isinstance(group, dict) else None
    if not isinstance(children, dict):
        issues.append((key, "missing x-category"))
        return
    for name, child in children.items():
        _walk(root, f"{path}.{name}", child, source, sections, plugin, depth + 1, settings, issues)


def read_setting(root, key, path, prop, source, sections, plugin):
    """One annotated key: the setting, or every problem with its annotations."""
    problems = []
    if not all(KEY_SEGMENT.match(part) for part in key.split(".")):
        problems.append("malformed key")
    category = prop.get("x-category")
    section = prop.get("x-section")
    if category is None:
        problems.append("missing x-category")
    elif category not in sections:
        tail = " (a plugin cannot add a top-level category)" if plugin else ""
        problems.append(f"unknown category `{category}`{tail}")
    if section is None:
        problems.append("missing x-section")
    elif category in sections and section not in sections[category]:
        problems.append(f"unknown section `{section}` in category `{category}`")
    order = prop.get("x-order")
    if order is None:
        problems.append("missing x-order")
    elif isinstance(order, bool) or not isinstance(order, int) or not 0 <= order < 2**32:
        problems.append("x-order is not a whole number >= 0")
    scopes = prop.get("x-scope")
    if scopes is None:
        problems.append("missing x-scope")
    elif not isinstance(scopes, list) or not scopes or any(s not in SCOPES for s in scopes):
        problems.append('x-scope is not a list of "user", "project", "local"')
    elif len(set(scopes)) != len(scopes):
        problems.append("x-scope repeats a scope")
    flags = {}
    for name in ("x-local-only", "x-protected", "x-secret"):
        flags[name] = prop.get(name, False)
        if not isinstance(flags[name], bool):
            problems.append(f"{name} is not true or false")
    if flags["x-local-only"] is True and scopes != ["local"]:
        problems.append('a local-only key has x-scope ["local"]')
    # A secret's values never show (RB-094-26), and MCP never writes one.
    if flags["x-secret"] is True and flags["x-protected"] is not True:
        problems.append("a secret key is x-protected")
    panel = panel_of(prop, flags["x-secret"] is True, plugin, problems)
    base = f"plugin.{plugin}.settings.{path}" if plugin else f"settings.key.{path}"
    label = prop.get("x-label-key", f"{base}.label")
    description = prop.get("x-description-key", f"{base}.description")
    help_key = prop.get("x-help-key", f"{base}.help")
    for name, value in (
        ("x-label-key", label), ("x-description-key", description), ("x-help-key", help_key),
    ):
        if not is_i18n_key(value):
            problems.append(f"{name} `{value}` is malformed")
    schema = own_schema(root, prop)
    problems += input_problems(path, schema)
    setting = Setting(
        key, category, section, order, scopes, flags["x-local-only"] is True,
        flags["x-protected"] is True, label, description, help_key, schema, source, plugin,
        schema.get("x-input"), root, flags["x-secret"] is True, panel,
    )
    return setting, problems


def panel_of(prop: dict, secret: bool, plugin: str | None, problems: list) -> dict | None:
    """The key's `x-panel` when it is well formed and allowed (RB-094-27), else `None` and the
    problem; whether the plugin has that panel is read_all's."""
    panel = prop.get("x-panel")
    if panel is None:
        return None
    if not secret:
        problems.append("x-panel is only on an x-secret key")
        return None
    if plugin is None:
        problems.append("x-panel names a plugin's panel: only on a plugin's key")
        return None
    tab = panel.get("tab") if isinstance(panel, dict) else None
    if (
        not isinstance(panel, dict) or set(panel) - {"panel", "tab"}
        or not isinstance(panel.get("panel"), str) or not PANEL_ID.match(panel["panel"])
        or (tab is not None and (not isinstance(tab, str) or not PANEL_ID.match(tab)))
    ):
        problems.append('x-panel is not {"panel": <id>, "tab": <id>}')
        return None
    return panel


def is_string_schema(schema) -> bool:
    """Whether the schema types a string (`"string"` or `["string", "null"]`)."""
    kind = schema.get("type") if isinstance(schema, dict) else None
    return kind == "string" or (isinstance(kind, list) and "string" in kind)


def is_list_or_map(schema) -> bool:
    """Whether the schema types a list or a map (an object with `additionalProperties`)."""
    kind = schema.get("type")
    kinds = kind if isinstance(kind, list) else [kind]
    return "array" in kinds or ("object" in kinds and isinstance(schema.get("additionalProperties"), dict))


def input_problems(path: str, schema) -> list[str]:
    """The problems of a key's `x-input` and `x-options` (RB-080-20)."""
    problems = []
    if "x-input" in schema:
        if schema["x-input"] not in INPUTS:
            problems.append('x-input is not "file" or "folder"')
        elif not is_string_schema(schema):
            problems.append("x-input sits on a key that is no string")
    elif is_string_schema(schema) and PATH_LEAF.search(path.rsplit(".", 1)[-1]):
        problems.append('a path key needs x-input ("file" or "folder")')
    options = schema.get("x-options", schema.get("items", {}).get("x-options") if isinstance(
        schema.get("items"), dict) else None)
    if options == DYNAMIC_OPTIONS:
        if "x-input" not in schema:
            problems.append('x-options "dynamic" sits on a key without x-input')
    elif options in STRING_SOURCES:
        if not is_string_schema(schema):
            problems.append(f'x-options "{options}" sits on a key that is no string')
    elif options in OPTION_SOURCES:
        if not is_list_or_map(schema):
            problems.append(f'x-options "{options}" sits on a key that is no list or map')
    elif options is not None and (
        not isinstance(options, list) or not options or not all(isinstance(o, str) for o in options)
    ):
        problems.append(
            'x-options is not a list of strings, a source ('
            + ", ".join(OPTION_SOURCES)
            + ') or "dynamic" on a path'
        )
    return problems


def read_all(root: Path):
    """Every setting of the repository and every issue, `(file, key, message)`.

    Reads the core schemas and the bundled plugins' fragments, then holds them together: a key
    once, an `x-order` once per section and source (the core or one plugin). Raises DataError
    when the categories do not read.
    """
    sections = dict(categories(root))
    settings, issues = [], []
    sources = [(path, None) for path in schema_files(root)]
    panels = {}
    try:
        bundled = plugins(root)
        sources += [(plugin.schema, plugin.id) for plugin in bundled]
        panels = {plugin.id: plugin.panels for plugin in bundled}
    except DataError as error:
        issues.append(("plugins", "", str(error)))
    for path, plugin in sources:
        source = relative(path, root)
        try:
            schema = load_json(path, root)
        except DataError as error:
            issues.append((source, "", str(error).split(": ", 1)[-1]))
            continue
        found, problems = walk(schema, source, sections, plugin)
        settings += found
        issues += [(source, key, message) for key, message in problems]
    seen, orders = {}, {}
    for setting in settings:
        if setting.key in seen:
            issues.append((setting.source, setting.key, f"defined twice (also {seen[setting.key]})"))
            continue
        seen[setting.key] = setting.source
        if setting.panel is not None and setting.panel["panel"] not in panels.get(setting.plugin, []):
            issues.append((
                setting.source, setting.key,
                f"x-panel names `{setting.panel['panel']}`, no panel of plugin {setting.plugin}",
            ))
        slot = (setting.category, setting.section, setting.order, setting.plugin)
        if slot in orders:
            issues.append((
                setting.source, setting.key,
                f"x-order {setting.order} repeats `{orders[slot]}` in section "
                f"{setting.category}/{setting.section}",
            ))
        else:
            orders[slot] = setting.key
    return settings, issues
