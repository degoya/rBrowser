"""The names of the choices of the settings (RB-090-16), shared by the settings lint.

A key whose value is a closed choice - an `enum`, or a `oneOf` of constants - shows each option by
its name from the catalogue, never by its id. The walk is the one of ui/src/settings/kinds.ts
(`enumOptions`, `kindOf`) and the window's names are ui/src/settings/optionNames.ts's; keep the
three in step:

- a core key's option is `settings.option.<key>.<value>`;
- a plugin key's is `plugin.<id>.settings.option.<path>.<value>`, in the plugin's own catalogue
  (the window gets the plugin's `plugin.<id>.settings.` texts), so `option` is no plugin key;
- a nested field adds its name (`breakpoints.engine`), a list's items and a map's values add
  nothing;
- the value's first letter in lower case (`A4` -> `a4`), as an i18n key segment starts;
- a number names itself and needs no text.
"""

import re

from settings_schema import MAX_DEPTH, is_i18n_key, resolve

RESERVED = "option"


def options_of(node: dict):
    """The allowed values of an `enum` or a `oneOf` of constants; `None` when the node is none.
    A branch marked `deprecated` - an earlier form the schema still accepts - is no option."""
    values = node.get("enum")
    if isinstance(values, list):
        return values
    branches = node.get("oneOf")
    if isinstance(branches, list):
        branches = [b for b in branches if not (isinstance(b, dict) and b.get("deprecated") is True)]
    if (
        isinstance(branches, list) and branches
        and all(isinstance(branch, dict) and "const" in branch for branch in branches)
    ):
        return [branch["const"] for branch in branches]
    return None


def choices(root: dict, node: object, name: str, depth: int = 0):
    """`(name, value)` of every option of `node` and its nested values, `name` as the window
    names it (the key, a nested field's with its path)."""
    node = resolve(root, node)
    if depth >= MAX_DEPTH or not isinstance(node, dict):
        return
    values = options_of(node)
    if values is not None:
        yield from ((name, value) for value in values)
        return
    for nested in ("items", "additionalProperties"):
        if isinstance(node.get(nested), dict):
            yield from choices(root, node[nested], name, depth + 1)
    properties = node.get("properties")
    if isinstance(properties, dict):
        for field, child in properties.items():
            yield from choices(root, child, f"{name}.{field}", depth + 1)


def segment(value: str) -> str:
    """The catalogue segment of an option: the value, its first letter in lower case."""
    return value[:1].lower() + value[1:]


def option_key(setting, name: str, value: str) -> str:
    """The i18n key of `value` of the field `name` (the key, or a nested field's path)."""
    if setting.plugin is None:
        return f"settings.option.{name}.{segment(value)}"
    path = name[len(setting.plugin) + 1:]
    return f"plugin.{setting.plugin}.settings.{RESERVED}.{path}.{segment(value)}"


def problems(setting, root: dict, known: dict) -> list[str]:
    """The options of `setting` without a name in `known`, a plugin key named `option`, and a
    description or help text that names an option by its id (RB-090-29)."""
    found = []
    if setting.plugin is not None and setting.key.split(".")[1] == RESERVED:
        found.append(f"`{RESERVED}` is reserved for the names of the choices")
    named = {}
    for name, value in choices(root, setting.schema, setting.key):
        if not isinstance(value, str):
            continue
        key = option_key(setting, name, value)
        if not is_i18n_key(key):
            found.append(f"option {value!r} makes no i18n key ({key})")
        elif key not in known:
            found.append(f'missing i18n key {key} (option "{value}")')
        elif isinstance(known[key], str):
            named.setdefault(value, known[key])
    for text_key in (setting.description_key, setting.help_key):
        text = known.get(text_key)
        if not isinstance(text, str):
            continue
        for value, shown in named.items():
            if (setting.key, value) not in ID_EXCEPTIONS and names_id(text, value, shown):
                found.append(f'{text_key} names the option id "{value}" - name it "{shown}"')
    return found


# The texts that name an option's id on purpose, each with its reason.
ID_EXCEPTIONS = {
    ("emulation.dir", "ltr"): "the value the page reads in <html dir>, which the text names",
    ("emulation.dir", "rtl"): "the value the page reads in <html dir>, which the text names",
}

# An id no English word looks like: camelCase, a hyphen, an underscore or a digit.
CODED = re.compile(r"[A-Z0-9_-]")


def names_id(text: str, value: str, shown: str) -> bool:
    """Whether `text` names the option `value` by its id rather than by `shown`, the name the
    window shows (RB-090-29). An id that is its name's word (`hidden`, `Hidden`) is no bare id.
    Any other counts where it is used as the option's name: opening a sentence or a clause
    (after `.`, `:`, `;`), or before "(the default)"; an id no word looks like (`perReport`,
    `px-right`) anywhere. A match inside the name as the text spells it (`skip them`, `US
    Letter`) is the name."""
    head = re.sub(r"\s*\(.*\)\s*\Z", "", shown)
    if head.lower() == value.lower():
        return False
    spelled = [(m.start(), m.end()) for m in re.finditer(re.escape(head), text, re.IGNORECASE)]
    word = re.escape(value)
    after = r"(?![\w`/-])"
    patterns = [
        r"(?:\A|[.:;!?]\s+)(" + word + ")" + after,
        r"(?<![\w.`/-])(" + word + r") \(the default\)",
    ]
    if CODED.search(value):
        patterns.append(r"(?<![\w.`/@-])(" + word + ")" + after)
    for pattern in patterns:
        for match in re.finditer(pattern, text):
            if not any(start <= match.start(1) < end for start, end in spelled):
                return True
    return False
