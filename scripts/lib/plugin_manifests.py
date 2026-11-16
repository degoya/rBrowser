"""The commands and MCP tools of the bundled first-party plugins, for the generated wiki pages.

The data: every plugins/<id>/manifest.toml (its `[[commands]]`, `[[tools]]` and `mcp_prefix`)
with the English texts of the plugin's own locales. A plugin folder with the marker `TEST-ONLY`
(plugins/hello-panel) is never shipped, so it is left out, as the package leaves it out
(scripts/lib/package.sh). The names follow the plugin host (crates/rb-plugin-host,
manifest/mod.rs): a command is `plugin.<id>.<command>` with a `-` of the id written `_`, its
group `plugins` when the manifest names none; a tool is `<mcp_prefix>_<tool>`.
"""

import tomllib
from dataclasses import dataclass, field
from pathlib import Path

import settings_schema
from settings_schema import DataError

TEST_ONLY = "TEST-ONLY"
DEFAULT_GROUP = "plugins"
# What a tool flagged `evaluates_input` needs over MCP (docs/plugins.md, "MCP").
EVALUATE_SETTING = "mcp.allowEvaluate"


@dataclass
class Command:
    id: str
    title: str
    group: str
    bindings: list
    mcp: dict  # {"covered": <tool>} or {"omitted": <reason>}, as in schema/commands.v1.json
    plugin: str  # the plugin's name


@dataclass
class Tool:
    name: str
    description: str
    input_schema: dict
    evaluates_input: bool
    plugin: str


@dataclass
class Plugin:
    id: str
    name: str
    commands: list = field(default_factory=list)
    tools: list = field(default_factory=list)


def _text(texts: dict, key: object, fallback: str) -> str:
    value = texts.get(key) if isinstance(key, str) else None
    return value if isinstance(value, str) and value else fallback


def _read(manifest: Path, root: Path) -> Plugin:
    where = settings_schema.relative(manifest, root)
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise DataError(f"{where}: {error}") from error
    plugin_id, name = data.get("id"), data.get("name")
    if not isinstance(plugin_id, str) or not isinstance(name, str):
        raise DataError(f"{where}: id or name missing")
    locale = (data.get("locales") or {}).get("en")
    texts = settings_schema.flatten(settings_schema.load_json(manifest.parent / locale, root)) \
        if isinstance(locale, str) else {}
    prefix = data.get("mcp_prefix")
    plugin = Plugin(plugin_id, name)
    for entry in data.get("tools") or []:
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            raise DataError(f"{where}: a tool without a name")
        if not isinstance(prefix, str):
            raise DataError(f"{where}: tools without an mcp_prefix")
        tool_name = f"{prefix}_{entry['name']}"
        schema = entry.get("input")
        plugin.tools.append(Tool(
            name=tool_name,
            description=_text(texts, entry.get("description"), tool_name),
            input_schema=schema if isinstance(schema, dict) else {"type": "object"},
            evaluates_input=entry.get("evaluates_input") is True,
            plugin=name,
        ))
    command_prefix = "plugin." + plugin_id.replace("-", "_") + "."
    for entry in data.get("commands") or []:
        if not isinstance(entry, dict) or not isinstance(entry.get("id"), str):
            raise DataError(f"{where}: a command without an id")
        command_id = command_prefix + entry["id"]
        tool, omitted = entry.get("tool"), entry.get("mcp_omitted")
        if isinstance(tool, str) and isinstance(prefix, str):
            mcp = {"covered": f"{prefix}_{tool}"}
        elif isinstance(omitted, str):
            mcp = {"omitted": omitted}
        else:
            raise DataError(f"{where}: {command_id} names neither a tool nor mcp_omitted")
        shortcut = entry.get("shortcut")
        group = entry.get("group", DEFAULT_GROUP)
        if not isinstance(group, str):
            raise DataError(f"{where}: {command_id} has a malformed group")
        plugin.commands.append(Command(
            id=command_id,
            title=_text(texts, entry.get("title"), command_id),
            group=group,
            bindings=[shortcut] if isinstance(shortcut, str) else [],
            mcp=mcp,
            plugin=name,
        ))
    return plugin


def bundled(root: Path) -> list[Plugin]:
    """The plugins the package ships, by folder name; a repository without plugins/ has none."""
    found = []
    for manifest in sorted((root / "plugins").glob("*/manifest.toml")):
        if (manifest.parent / TEST_ONLY).exists():
            continue
        found.append(_read(manifest, root))
    return found


def commands(root: Path) -> list[Command]:
    return [command for plugin in bundled(root) for command in plugin.commands]


def tools(root: Path) -> list[Tool]:
    return [tool for plugin in bundled(root) for tool in plugin.tools]


def marker(plugin: str) -> str:
    """How a page marks a plugin's entry."""
    return f"*(plugin: {plugin})*"
