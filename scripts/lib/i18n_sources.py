"""What check_i18n.py reads of a source: the keys it names, the key-shaped literals it holds, the
key families it looks up by a computed key, and - in a `.vue` file - visible text written raw.

Rust: `t("k")`, `t_with("k", ...)`, `t!("k", ...)` (also on the next line, after rustfmt), and
`t(&format!("prefix.{x}"))` as the dynamic family `prefix.`. TypeScript and Vue: `t('k')`,
`$t("k")`, `i18n.global.t(`...`)`, `keypath="k"`; a template literal `t(`prefix.${x}`)` is the
dynamic family `prefix.`. Schemas and Rust attributes: `"x-label-key"` / `"x-description-key"`.
Comment lines (`//` first) are no code.
"""

import re

KEY_SHAPE = r"[a-z][a-z0-9]*(?:\.[a-z0-9][A-Za-z0-9_-]*)+"

RUST_CALL = re.compile(r"(?<![\w])(?:t|t_with)!?\(\s*\"([^\"\\]*)\"")
RUST_DYNAMIC = re.compile(r"(?<![\w])(?:t|t_with)!?\(\s*&format!\(\s*\"([^\"{]*)\{")
TS_CALL = re.compile(r"(?<![\w$])\$?t\(\s*(['\"`])((?:(?!\1)[^\\\n])*)\1")
VUE_KEYPATH = re.compile(r"\bkeypath=\"([^\"]+)\"")
SCHEMA_KEY = re.compile(r"\"x-(?:label|description)-key\"\s*[:=]\s*\"([^\"]+)\"")
LITERAL = re.compile(r"[\"']({})[\"']".format(KEY_SHAPE))

# Attributes whose static value a user reads; a bound one (`:title`, `v-bind:title`) is code.
VISIBLE_ATTRIBUTES = ("title", "placeholder", "alt", "label", "aria-label", "aria-placeholder",
                      "aria-description")
STATIC_ATTRIBUTE = re.compile(
    r"(?<![:@#\w.-])({})\s*=\s*\"([^\"]*)\"".format("|".join(VISIBLE_ATTRIBUTES)))
TAG = re.compile(r"<(?:\"[^\"]*\"|'[^']*'|[^'\">])*>")
LETTER = re.compile(r"[^\W\d_]")


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def code_only(text):
    """`text` with every line that starts with `//` blanked (line numbers stay)."""
    return "\n".join("" if line.lstrip().startswith("//") else line for line in text.split("\n"))


def scan(text, kind):
    """(named keys as (line, key), dynamic prefixes, key-shaped literals) of one source.

    `kind` is "rs", "ts" (also .vue) or "json".
    """
    named, dynamic = [], set()
    code = code_only(text) if kind in ("rs", "ts") else text
    if kind == "rs":
        named += [(line_of(code, m.start()), m.group(1)) for m in RUST_CALL.finditer(code)]
        dynamic |= {m.group(1) for m in RUST_DYNAMIC.finditer(code) if m.group(1).endswith(".")}
    if kind == "ts":
        for m in TS_CALL.finditer(code):
            key = m.group(2)
            if m.group(1) == "`" and "${" in key:
                prefix = key.split("${", 1)[0]
                if prefix.endswith("."):
                    dynamic.add(prefix)
                continue
            named.append((line_of(code, m.start()), key))
        named += [(line_of(code, m.start()), m.group(1)) for m in VUE_KEYPATH.finditer(code)]
    named += [(line_of(code, m.start()), m.group(1)) for m in SCHEMA_KEY.finditer(code)]
    literals = {m.group(1) for m in LITERAL.finditer(code)}
    return named, dynamic, literals


def _blank(text, start, end):
    """`text` with [start, end) replaced by spaces, newlines kept."""
    return text[:start] + re.sub(r"[^\n]", " ", text[start:end]) + text[end:]


def raw_texts(text):
    """(line, text) of every visible text a `.vue` template writes raw instead of through a key:
    text between tags and the static value of a visible attribute, holding a letter. Comments,
    `{{ ... }}` and character references (`&times;`) are no raw text."""
    template = re.search(r"<template\b[^>]*>(.*)</template>", text, re.S)
    if not template:
        return []
    masked = text
    for pattern in (r"<!--.*?-->", r"\{\{.*?\}\}", r"&(?:[A-Za-z]+|#[0-9]+|#x[0-9A-Fa-f]+);"):
        for m in list(re.finditer(pattern, masked, re.S)):
            masked = _blank(masked, m.start(), m.end())
    start, end = template.span(1)
    found = []
    cursor = start
    for tag in TAG.finditer(masked, start, end):
        between = masked[cursor:tag.start()]
        if LETTER.search(between):
            found.append((line_of(text, cursor + len(between) - len(between.lstrip())),
                          " ".join(between.split())))
        for attribute in STATIC_ATTRIBUTE.finditer(tag.group()):
            if LETTER.search(attribute.group(2)):
                found.append((line_of(text, tag.start()),
                              f'{attribute.group(1)}="{attribute.group(2)}"'))
        cursor = tag.end()
    between = masked[cursor:end]
    if LETTER.search(between):
        found.append((line_of(text, cursor), " ".join(between.split())))
    return found
