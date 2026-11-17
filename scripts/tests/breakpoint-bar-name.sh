#!/usr/bin/env bash
#
# The strip under every view is the "breakpoint bar" wherever a user reads it (RB-090-11), and the
# toolbar's sun and moon say they set the page's colour scheme (RB-090-10). Reads the shell and
# core catalogue locales/en.json as it will be after the wave: the pending parts
# (locales/parts/*.json) added and the changed texts of a wave (locales/parts/*.changed, a JSON
# object of key -> text after `//` header lines) applied. Then:
#
#   - no text says "footer", except the one note "formerly called the footer";
#   - that note is there, and the strip's names (command, menus, toolbar, settings) say
#     "Breakpoint bar";
#   - the keys and ids stay (`ui.viewFooter.*`, `view_footer`, `footer.*`): nothing migrates;
#   - a changed text names a key en.json has (a new key belongs in a part, not here).
#
# Pure python3: it runs in a second. check.sh --preflight runs it.
#
#   scripts/tests/breakpoint-bar-name.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

python3 -I - "$ROOT" <<'PY'
import glob
import json
import os
import re
import sys

root = sys.argv[1]
failures = []


def flatten(node, prefix=""):
    if isinstance(node, dict):
        for key, value in node.items():
            yield from flatten(value, f"{prefix}.{key}" if prefix else key)
    else:
        yield prefix, node


with open(os.path.join(root, "locales", "en.json"), encoding="utf-8") as handle:
    texts = dict(flatten(json.load(handle)))
merged = set(texts)
for part in sorted(glob.glob(os.path.join(root, "locales", "parts", "*.json"))):
    with open(part, encoding="utf-8") as handle:
        for key, text in flatten(json.load(handle)):
            texts.setdefault(key, text)
for changed in sorted(glob.glob(os.path.join(root, "locales", "parts", "*.changed"))):
    with open(changed, encoding="utf-8") as handle:
        body = "".join(line for line in handle if not line.lstrip().startswith("//"))
    for key, text in json.loads(body).items():
        if key not in merged:
            failures.append(f"{os.path.basename(changed)}: `{key}` is no key of en.json")
        texts[key] = text

NOTE = "formerly called the footer"
for key, text in sorted(texts.items()):
    if re.search(r"footer", text.replace(NOTE, ""), re.IGNORECASE):
        failures.append(f"`{key}` still says footer: {text[:120]!r}")
if not any(NOTE in text for text in texts.values()):
    failures.append(f"no help text names the old name ({NOTE!r})")

NAMES = [
    "commands.ui.view_footer.toggle",
    "contextmenu.view_footer",
    "views.menu.footer",
    "shell.toolbar.footer",
    "settings.key.ui.viewFooter.visible.label",
    "settings.key.ui.viewFooter.fields.label",
]
for key in NAMES:
    if key not in texts:
        failures.append(f"`{key}` is gone: the keys stay")
    elif "breakpoint bar" not in texts[key].lower():
        failures.append(f"`{key}` does not say breakpoint bar: {texts[key]!r}")

for key in ("emulation.toolbar.light", "emulation.toolbar.dark"):
    if not texts.get(key, "").startswith("Page colour scheme"):
        failures.append(f"`{key}` does not say it is the page's colour scheme: {texts.get(key)!r}")

for failure in failures:
    print(f"FAIL: {failure}")
if failures:
    sys.exit(1)
print(f"ok: {len(texts)} texts, the strip is the breakpoint bar everywhere")
PY
