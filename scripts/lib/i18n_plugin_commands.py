"""Every plugin command has its title (RB-080-82), a rule of the i18n lint (check_i18n.py).

A plugin names its command's title by key in its manifest (`[[commands]] title =
"plugin.<id>.commands.<command>"`); the text is in the plugin's own catalogue (`[locales] en`).
A missing text shows the key itself - in the shortcut editor, the palette, the which-key bar and
the status bar - so every command of every plugin folder (plugins/<dir>/manifest.toml, the
test-only ones too) must name a key below its own `plugin.<id>.` that its English catalogue
holds with a text.
"""

import json
import os
import tomllib


def _flat(value, prefix=""):
    """`{"a": {"b": "x"}}` as `{"a.b": "x"}`."""
    out = {}
    if isinstance(value, dict):
        for key, child in value.items():
            out.update(_flat(child, f"{prefix}{key}."))
    elif isinstance(value, str):
        out[prefix[:-1]] = value
    return out


def check(root, problems):
    """Appends one line per command of a plugin manifest without its title."""
    plugins = os.path.join(root, "plugins")
    if not os.path.isdir(plugins):
        return
    for name in sorted(os.listdir(plugins)):
        manifest = os.path.join(plugins, name, "manifest.toml")
        if not os.path.isfile(manifest):
            continue
        where = os.path.relpath(manifest, root)
        try:
            with open(manifest, "rb") as handle:
                data = tomllib.load(handle)
        except (OSError, tomllib.TOMLDecodeError) as error:
            problems.append(f"{where}: {error}")
            continue
        commands = data.get("commands") or []
        if not commands:
            continue
        plugin_id = data.get("id")
        locale = (data.get("locales") or {}).get("en")
        texts = {}
        if isinstance(locale, str):
            try:
                with open(os.path.join(plugins, name, locale), encoding="utf-8") as handle:
                    texts = _flat(json.load(handle))
            except (OSError, ValueError) as error:
                problems.append(f"{where}: its English catalogue {locale} cannot be read: {error}")
        for command in commands:
            command_id = command.get("id") if isinstance(command, dict) else None
            title = command.get("title") if isinstance(command, dict) else None
            label = f"{where}: the command `{command_id}`"
            if not isinstance(title, str) or title == "":
                problems.append(f"{label} has no title key")
            elif not title.startswith(f"plugin.{plugin_id}."):
                problems.append(f"{label}: its title `{title}` is not below plugin.{plugin_id}.")
            elif not texts.get(title, "").strip():
                problems.append(f"{label}: its title `{title}` has no text in the plugin's "
                                "English catalogue")
