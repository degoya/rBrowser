"""Every settings mention is a link (RB-094-21): a UI text that names a settings path
("Settings > Engines > Android", also with `›`) is a key of the table
crates/rbrowser/data/settings-links.json -

* in `links`, mapped to the place it opens: a key, a category, a section or a key prefix, the
  core's resolution (settings_targets.resolve). The core links a notice of that text, or one
  whose arguments carry it, by the table (crates/rbrowser/src/status/settings_links.rs); a page
  or a panel shows the link beside the text (`<SettingLink setting-key="...">`,
  `panel.openSettings('...')`);
* or in `unlinked`, with why: a path in another program's settings (the phone's, Xcode's).

A secret key is no setting of the window (RB-094-27): a row linked to one opens the panel that
edits it, so its text names that panel's place ("API client › Variables"), never a settings path,
and a link to a secret key without a panel opens nothing.

The texts are the core catalogue (locales/en.json with the wave parts) and every plugin's
plugins/<id>/locales/en.json. The settings window's own texts (`settings.*`,
`plugin.<id>.settings.*`) and the MCP tool descriptions (`plugin.<id>.tools.*`), read by a
model, need no entry. A row whose text names no settings path any more is a problem too. Used
by scripts/lib/settings_lint.py.
"""

import json
import re
from pathlib import Path

import settings_schema
import settings_targets

TABLE = Path("crates/rbrowser/data/settings-links.json")
MENTION = re.compile(r"\bSettings ?(?:>|›|&gt;)")
EXEMPT = (re.compile(r"^settings\."), re.compile(r"^plugin\.[^.]+\.(?:settings|tools)\."))


def texts(root: Path) -> dict:
    """Every text of the catalogues, the core's and the plugins', as key -> (file, text)."""
    found = {}
    catalogues = [("locales/en.json", settings_schema.catalogue(root))]
    for path in sorted((root / "plugins").glob("*/locales/en.json")):
        catalogues.append((settings_schema.relative(path, root),
                           settings_schema.flatten(settings_schema.load_json(path, root))))
    for where, catalogue in catalogues:
        for key, text in catalogue.items():
            if isinstance(text, str):
                found[key] = (where, text)
    return found


def mentions(every: dict) -> dict:
    """Every text key of `every` that names a settings path, with the file it is in."""
    return {
        key: where for key, (where, text) in every.items()
        if MENTION.search(text) and not any(p.match(key) for p in EXEMPT)
    }


def table(root: Path) -> tuple[dict, dict]:
    """The table's `links` and `unlinked`; both empty without the file."""
    path = root / TABLE
    if not path.is_file():
        return {}, {}
    data = settings_schema.load_json(path, root)
    links = data.get("links", {}) if isinstance(data, dict) else {}
    unlinked = data.get("unlinked", {}) if isinstance(data, dict) else {}
    if not isinstance(links, dict) or not isinstance(unlinked, dict):
        raise settings_schema.DataError(f"{TABLE}: `links` and `unlinked` are objects")
    return links, unlinked


def problems(root: Path, settings: list, cats: list) -> tuple[list[str], int]:
    """The problems, one line each, and the number of settings mentions."""
    try:
        links, unlinked = table(root)
        every = texts(root)
        found = mentions(every)
    except settings_schema.DataError as error:
        return [str(error)], 0
    out = []
    for key, where in sorted(found.items()):
        if key not in links and key not in unlinked:
            out.append(f"{where}: `{key}` names a settings path but links nowhere: map it to its "
                       f"setting in {TABLE} (`links`), or say in `unlinked` why it cannot link")
    name = TABLE.name
    for key, target in sorted(links.items()):
        place = settings_targets.resolve(target, settings, cats) if isinstance(target, str) else None
        if place is not None and place.startswith("panel "):
            # A secret key's panel (RB-094-27): the text names the panel, no settings path.
            if key not in every:
                out.append(f"{name}: `{key}` is no text of the catalogues")
            elif key in found:
                out.append(f"{name}: `{key}` opens the {place} but its text names a settings path")
        elif isinstance(target, str) and settings_targets.why_not(target, settings) == settings_targets.SECRET:
            out.append(f"{name}: `{key}` links to `{target}`, {settings_targets.SECRET}")
        elif key not in found:
            out.append(f"{name}: `{key}` is no text that names a settings path")
        elif place is None:
            out.append(f"{name}: `{key}` links to `{target}`, which is no key, category, section "
                       "or key prefix")
    for key, why in sorted(unlinked.items()):
        if key not in found:
            out.append(f"{name}: `{key}` is no text that names a settings path")
        elif not isinstance(why, str) or not why.strip():
            out.append(f"{name}: unlinked `{key}` says not why")
    return out, len(found)
