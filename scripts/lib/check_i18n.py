#!/usr/bin/env python3
"""The i18n lint (RB-010-14), run by scripts/check-i18n.sh and check.sh --preflight:

    python3 scripts/lib/check_i18n.py <repository root>

It fails (exit 1, one line per problem) when
* locales/en.json, a wave part (locales/parts/*.json), another language's catalogue or a plugin's
  locales/<lang>.json breaks the catalogue rules (i18n_catalogue.py), or en.json is not in the
  canonical form merge_locales.py writes;
* a source names a key the catalogue lacks: Rust in crates/*/src and crates/*/tests (rb-i18n
  itself aside, whose tests name made-up keys), TypeScript and Vue in ui/src, the
  `x-label-key` / `x-description-key` of schema/ and of Rust attributes;
* a key of the catalogue is used nowhere: neither named (tests do not count, they show nothing)
  nor written as a literal in a source or data file, nor below a family looked up by a computed
  key - DYNAMIC_PREFIXES, plus every `t(`prefix.${x}`)` / `t(&format!("prefix.{x}"))` found;
* a `.vue` template in ui/src writes visible text raw (`<button>Save</button>`, `title="Save"`)
  instead of through a key;
* a text of any catalogue, a plugin's included, sets an error code in (`{code}`) or writes a
  count without its plural form (`{n} views`) - i18n_texts.py (RB-080-90).
* a plugin command has no title text in its plugin's catalogue (i18n_plugin_commands.py,
  RB-080-82);
* a text breaks the typography (three dots, a `>` path, a pseudo-plural `(s)`) or a command
  title the pattern of command titles (i18n_style.py, RB-099-22).

The catalogue it checks against is en.json together with the parts not merged yet, so a job's
branch passes with its keys in its part. Another language may only hold keys en.json has.
"""

import os
import re
import sys

sys.dont_write_bytecode = True
from i18n_catalogue import dump, flatten, nest  # noqa: E402
from i18n_sources import raw_texts, scan  # noqa: E402
from i18n_texts import problems as text_problems  # noqa: E402
import i18n_plugin_commands  # noqa: E402
import i18n_style  # noqa: E402

# Families looked up by a key computed from an id at run time and held complete by their own
# lints: settings labels (RB-010-02, from the schema), command titles and groups (RB-010-17,
# from the registry), plugin keys (each plugin's own catalogue), the plugin capabilities
# (RB-010-03, `rb_plugin_types::Capability::title_key`, one per capability id).
DYNAMIC_PREFIXES = ("settings.", "commands.", "plugin.", "plugins.capability.")
SKIPPED_DIRS = {"node_modules", "target", "dist"}
RUST_ROOTS = ("crates",)
RUST_SKIPPED = (os.path.join("crates", "rb-i18n") + os.sep,)
FRONTEND_ROOTS = (os.path.join("ui", "src"),)
DATA_ROOTS = ("schema",)
TEST_FILE = re.compile(r"(^|/)(tests\.rs|[^/]*_tests\.rs|[^/]*\.(test|spec)\.ts)$|(^|/)tests/")


def files_under(root, relative, extensions):
    """Every file below root/relative ending in one of `extensions`, as sorted relative paths."""
    found = []
    for directory, subdirectories, names in os.walk(os.path.join(root, relative)):
        subdirectories[:] = sorted(d for d in subdirectories
                                   if d not in SKIPPED_DIRS and not d.startswith("."))
        for name in names:
            if name.endswith(extensions):
                found.append(os.path.relpath(os.path.join(directory, name), root))
    return sorted(found)


def read(root, relative):
    with open(os.path.join(root, relative), encoding="utf-8") as handle:
        return handle.read()


def check_catalogues(root, problems):
    """The English catalogue with the pending parts, by key."""
    english_path = os.path.join("locales", "en.json")
    present = os.path.exists(os.path.join(root, english_path))
    english_text = read(root, english_path) if present else "{}"
    english, found = flatten(english_text, english_path)
    problems += found
    if not found:
        try:
            if dump(nest(english)) != english_text:
                problems.append(f"{english_path}: not canonical (nested, sorted, 2-space indent); "
                                "run python3 scripts/lib/merge_locales.py")
        except ValueError as error:
            problems.append(f"{english_path}: {error}")
    catalogue = dict(english)
    for part in files_under(root, os.path.join("locales", "parts"), (".json",)):
        flat, found = flatten(read(root, part), part)
        problems += found
        for key, text in flat.items():
            if catalogue.get(key, text) != text:
                problems.append(f"{part}: `{key}` conflicts with the text already in the catalogue")
            catalogue.setdefault(key, text)
    for other in files_under(root, "locales", (".json",)):
        if os.path.dirname(other) != "locales" or other == english_path:
            continue
        flat, found = flatten(read(root, other), other)
        problems += found
        problems += [f"{other}: `{key}` is not in {english_path}"
                     for key in flat if key not in catalogue]
    problems += text_problems(catalogue, "locales/en.json (with the parts)")
    problems += i18n_style.text_problems(catalogue, "locales/en.json (with the parts)")
    for locale in files_under(root, "plugins", (".json",)):
        if os.path.basename(os.path.dirname(locale)) == "locales":
            flat, found = flatten(read(root, locale), locale)
            problems += found + text_problems(flat, locale) + i18n_style.text_problems(flat, locale)
    return catalogue


def main(argv):
    if len(argv) != 1:
        print("usage: check_i18n.py <repository root>", file=sys.stderr)
        return 2
    root = argv[0]
    problems = []
    catalogue = check_catalogues(root, problems)
    sources = [(path, "rs") for base in RUST_ROOTS for path in files_under(root, base, (".rs",))
               if not path.startswith(RUST_SKIPPED)]
    sources += [(path, "ts") for base in FRONTEND_ROOTS
                for path in files_under(root, base, (".ts", ".vue"))]
    sources += [(path, "json") for base in DATA_ROOTS
                for path in files_under(root, base, (".json",))]
    sources += [(path, "json") for path in files_under(root, "crates", (".json",))
                if f"{os.sep}data{os.sep}" in path]
    used, dynamic = set(), set(DYNAMIC_PREFIXES)
    for path, kind in sources:
        text = read(root, path)
        named, prefixes, literals = scan(text, kind)
        test = bool(TEST_FILE.search(path.replace(os.sep, "/")))
        for line, key in named:
            if key not in catalogue:
                problems.append(f"{path}:{line}: `{key}` is not in locales/en.json")
            if not test:
                used.add(key)
        if not test:
            dynamic |= prefixes
            used |= literals
        if path.endswith(".vue") and not test:
            problems += [f"{path}:{line}: raw text `{raw}` - use a key (t('...'))"
                         for line, raw in raw_texts(text)]
    unused = sorted(key for key in catalogue
                    if key not in used and not key.startswith(tuple(dynamic)))
    problems += [f"locales/en.json: `{key}` is used nowhere" for key in unused]
    i18n_plugin_commands.check(root, problems)
    i18n_style.check_titles(root, catalogue, problems)
    for problem in problems:
        print(f"!! {problem}")
    if problems:
        print(f"i18n: {len(problems)} problem(s)")
        return 1
    print(f"i18n: {len(catalogue)} key(s), {len(sources)} source(s), ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
