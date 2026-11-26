"""The house style of the catalogues (RB-099-22), two rules of the i18n lint (check_i18n.py):

* **Typography**, every text of every catalogue (a plugin's included):
  - the ellipsis is one character, `…`, never three dots; three dots in code stay (`"..."`,
    `forEach(...)`, `own:...`, `https://...`: a quote, bracket, colon or slash before them or a
    quote after them);
  - a settings or menu path is separated by ` › ` (`Settings › Capture › Video`), as the wiki
    and the settings window write it, never by ` > ` (a `>` before a lowercase word, the command
    palette's prefix, stays);
  - no pseudo-plural `(s)` or `(es)`: a count is an ICU plural (`{count, plural, one {...} other {...}}`), a
    unit is written out (`Duration (seconds)`).
* **Command titles** (docs/design.md, Glossary; owner, 2026-11-25), the core's
  (schema/commands.v1.json) and every plugin's (plugins/*/manifest.toml): a toggle is
  "Show or hide ..." when it shows something, "Switch ... on or off" when it switches a state
  without a display - never "Toggle ..." or "... on/off" - and a title is in sentence case: after
  its first word only a name keeps its capital (an acronym, a word with an inner capital such as
  DevTools, one of NAMES or one of the PHRASES). Group titles are names of their own and not
  checked.
"""

import json
import os
import re
import tomllib

from i18n_plugin_commands import _flat

DOTS = re.compile(r"\.\.\.")
CODE_BEFORE = "\"'([{:/=`"
CODE_AFTER = "\"'`"
PATH = re.compile(r"(?<=\S) > (?=[A-Z]|iOS\b)")
PSEUDO_PLURAL = re.compile(r"\((?:s|es)\)")

TOGGLE = re.compile(r"\btoggle\b", re.IGNORECASE)
ON_OFF = re.compile(r"\b(on/off|off/on)\b", re.IGNORECASE)
WORD = re.compile(r"[A-Za-z][A-Za-z0-9'+-]*")

# Names that keep their capital inside a sentence-case title: products of one word, and the
# names of several words as whole phrases - their words alone (Web, Store, Tools) are no names.
NAMES = frozenset("""
    Android Chrome Claude Cloudflare Firefox Git Lighthouse Node Playwright React Safari Vue Xcode
""".split())
PHRASES = (
    "A4 or Letter", "Android Platform Tools", "Basic auth", "Chrome Web Store", "Claude Code",
    "Cloudflare Quick Tunnel", "Live CSS", "Web Vitals",
)


def _code_dots(text, match):
    before = text[match.start() - 1] if match.start() > 0 else ""
    after = text[match.end()] if match.end() < len(text) else ""
    return (before and before in CODE_BEFORE) or (after and after in CODE_AFTER)


def text_problems(flat, source):
    """The texts of catalogue `flat` (key -> text) that break the typography, as lines."""
    found = []
    for key, text in sorted(flat.items()):
        if not isinstance(text, str):
            continue
        if any(not _code_dots(text, match) for match in DOTS.finditer(text)):
            found.append(f"{source}: `{key}` writes ... - write the ellipsis …")
        if PATH.search(text):
            found.append(f"{source}: `{key}` separates a path with > - write ›")
        if PSEUDO_PLURAL.search(text):
            found.append(f"{source}: `{key}` writes (s) - write the plural "
                         "({count, plural, one {...} other {...}}) or the unit in words")
    return found


def _is_name(word):
    letters = [c for c in word if c.isalpha()]
    return (not any(c.islower() for c in letters)          # an acronym: CSS, MCP, A4
            or any(c.isupper() for c in word[1:])          # an inner capital: DevTools, WebKit
            or word in NAMES)


def title_problem(key, title):
    """Why `title` breaks the pattern of command titles, or None."""
    if TOGGLE.search(title):
        return f"`{key}`: \"{title}\" says toggle - write \"Show or hide ...\" or \"Switch ... on or off\""
    if ON_OFF.search(title):
        return f"`{key}`: \"{title}\" says on/off - write \"Switch ... on or off\""
    masked = title
    for phrase in PHRASES:
        masked = masked.replace(phrase, "name")
    capitals = [w for w in WORD.findall(masked)[1:] if w[0].isupper() and not _is_name(w)]
    if capitals:
        return f"`{key}`: \"{title}\" is not in sentence case ({', '.join(capitals)})"
    return None


def _titles(root, catalogue):
    """(source, key, title) of every command, the core's and the plugins'."""
    commands = os.path.join(root, "schema", "commands.v1.json")
    if os.path.isfile(commands):
        with open(commands, encoding="utf-8") as handle:
            data = json.load(handle)
        for command in data.get("commands") or []:
            key = command.get("titleKey") if isinstance(command, dict) else None
            if isinstance(catalogue.get(key), str):
                yield "schema/commands.v1.json", key, catalogue[key]
    plugins = os.path.join(root, "plugins")
    for name in sorted(os.listdir(plugins)) if os.path.isdir(plugins) else []:
        manifest = os.path.join(plugins, name, "manifest.toml")
        if not os.path.isfile(manifest):
            continue
        try:
            with open(manifest, "rb") as handle:
                data = tomllib.load(handle)
            locale = (data.get("locales") or {}).get("en")
            with open(os.path.join(plugins, name, str(locale)), encoding="utf-8") as handle:
                texts = _flat(json.load(handle))
        except (OSError, ValueError, tomllib.TOMLDecodeError):
            continue  # i18n_plugin_commands.check names it
        for command in data.get("commands") or []:
            key = command.get("title") if isinstance(command, dict) else None
            if isinstance(texts.get(key), str):
                yield f"plugins/{name}/manifest.toml", key, texts[key]


def check_titles(root, catalogue, problems):
    """Appends one line per command title that breaks the pattern."""
    for source, key, title in _titles(root, catalogue):
        problem = title_problem(key, title)
        if problem:
            problems.append(f"{source}: {problem}")
