"""The catalogue rules of crates/rb-i18n in Python, for merge_locales.py and check_i18n.py.

A catalogue (`locales/<lang>.json`) is a JSON object nested by key segment
(`{"toolbar": {"reload": "Reload"}}` holds `toolbar.reload`). Every key is `<area>.<thing>`
(rb_i18n::is_well_formed_key), no key is given twice, and every text is a non-empty message:
`{name}` placeholders and ICU plurals `{n, plural, =0 {...} one {# x} other {# xs}}`, without
quoting (docs/i18n.md). The written form is canonical: nested, keys sorted, two-space indent,
UTF-8, one newline at the end - what `dump(nest(...))` gives.
"""

import json
import re

KEY = re.compile(r"[a-z][a-z0-9]*(?:\.[a-z0-9][A-Za-z0-9_-]*)+\Z")
# Segments that would walk the shell's nested catalogue into a prototype (rb_i18n UNSAFE_SEGMENTS).
UNSAFE_SEGMENTS = {"__proto__", "constructor", "prototype"}
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
DIGITS = re.compile(r"[0-9]+")
CATEGORIES = {"zero", "one", "two", "few", "many", "other"}
SPACE = " \t\n\r\x0c"


class _Object(list):
    """A JSON object as its (name, value) pairs in file order, duplicates kept."""


def flatten(text, source):
    """The texts of catalogue `text` by dotted key, and the problems found (`source: ...`)."""
    problems = []
    try:
        data = json.loads(text, object_pairs_hook=_Object)
    except ValueError as error:
        return {}, [f"{source}: not valid JSON: {error}"]
    if not isinstance(data, _Object):
        return {}, [f"{source}: the catalogue is not a JSON object"]
    flat = {}

    def walk(pairs, prefix):
        for segment, value in pairs:
            key = f"{prefix}.{segment}" if prefix else segment
            if isinstance(value, _Object):
                walk(value, key)
            elif not isinstance(value, str):
                problems.append(f"{source}: `{key}` is neither a text nor an object")
            elif not KEY.match(key) or UNSAFE_SEGMENTS & set(key.split(".")):
                problems.append(f"{source}: `{key}` is no well-formed key (<area>.<thing>)")
            elif key in flat:
                problems.append(f"{source}: `{key}` is given twice")
            else:
                reason = message_error(value) if value.strip() else "the text is empty"
                if reason:
                    problems.append(f"{source}: `{key}` is no valid message: {reason}")
                flat[key] = value

    walk(data, "")
    return flat, problems


def nest(flat):
    """`flat` nested by segment; raises ValueError where a key is both a text and a prefix."""
    tree = {}
    for key in sorted(flat):
        *parents, leaf = key.split(".")
        node = tree
        for depth, segment in enumerate(parents):
            node = node.setdefault(segment, {})
            if not isinstance(node, dict):
                prefix = ".".join(parents[: depth + 1])
                raise ValueError(f"`{prefix}` is a text and also the prefix of `{key}`")
        if leaf in node:
            raise ValueError(f"`{key}` is a text and also the prefix of other keys")
        node[leaf] = flat[key]
    return tree


def dump(tree):
    """The canonical text of a nested catalogue."""
    return json.dumps(tree, indent=2, ensure_ascii=False, sort_keys=True) + "\n"


def message_error(text):
    """Why `text` is no valid message, or None."""
    try:
        _Parser(text).message(False)
    except ValueError as error:
        return str(error)
    return None


class _Parser:
    def __init__(self, text):
        self.s = text
        self.i = 0

    def peek(self):
        return self.s[self.i] if self.i < len(self.s) else None

    def space(self):
        while self.peek() is not None and self.peek() in SPACE:
            self.i += 1

    def expect(self, wanted):
        if self.peek() != wanted:
            raise ValueError(f"expected `{wanted}` at {self.i}")
        self.i += 1

    def word(self, pattern, what):
        found = pattern.match(self.s, self.i)
        if not found:
            raise ValueError(f"expected {what} at {self.i}")
        self.i = found.end()
        return found.group()

    def message(self, in_branch):
        while True:
            c = self.peek()
            if c is None:
                if in_branch:
                    raise ValueError("a plural branch is not closed")
                return
            if c == "}":
                if in_branch:
                    return
                raise ValueError(f"unmatched `}}` at {self.i}")
            if c == "{":
                self.argument()
            else:
                self.i += 1

    def argument(self):
        self.i += 1
        self.space()
        self.word(IDENT, "an argument name")
        self.space()
        if self.peek() == "}":
            self.i += 1
            return
        self.expect(",")
        self.space()
        kind = self.word(IDENT, "`plural`")
        if kind != "plural":
            raise ValueError(f"`{kind}` is not supported, only `plural`")
        self.space()
        self.expect(",")
        seen = set()
        while True:
            self.space()
            c = self.peek()
            if c is None:
                raise ValueError("a plural is not closed")
            if c == "}":
                self.i += 1
                break
            if c == "=":
                self.i += 1
                selector = "=" + str(int(self.word(DIGITS, "a number after `=`")))
            else:
                selector = self.word(IDENT, "a plural category")
                if selector not in CATEGORIES:
                    raise ValueError(f"`{selector}` is no plural category")
            if selector in seen:
                raise ValueError(f"a plural names `{selector}` twice")
            seen.add(selector)
            self.space()
            self.expect("{")
            self.message(True)
            self.expect("}")
        if "other" not in seen:
            raise ValueError("a plural needs an `other` branch")
