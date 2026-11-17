#!/usr/bin/env python3
"""Merges the wave parts `locales/parts/*.json` into `locales/en.json`.

In a wave no job edits locales/en.json; each delivers its keys as locales/parts/<job-id>.json
(the same nested shape), and the coordinator runs this after the merges:

    python3 scripts/lib/merge_locales.py [--root DIR] [--check] [--prune]

The result is canonical (nested, keys sorted, two-space indent), so a second run changes nothing.
A key that two files give with different texts is a conflict: nothing is written, the run names
both files and exits 1 - resolve it in the part, never by overwriting. A key given twice in one
file, a malformed key or message, and a key that is a text and a prefix of another at once are
refused the same way. The same key with the same text in several files is fine.

--check writes nothing and exits 1 when en.json is not what a run would write (parts pending, or
the file not canonical). --prune deletes the merged part files after a successful write (the
`.gitkeep` stays). Exit 2 is a usage error.
"""

import argparse
import os
import sys

sys.dont_write_bytecode = True
from i18n_catalogue import dump, flatten, nest  # noqa: E402

CATALOGUE = os.path.join("locales", "en.json")
PARTS = os.path.join("locales", "parts")


def part_files(root):
    """The part files under `root`, sorted by name."""
    directory = os.path.join(root, PARTS)
    if not os.path.isdir(directory):
        return []
    return sorted(
        os.path.join(PARTS, name) for name in os.listdir(directory) if name.endswith(".json")
    )


def read(root, relative):
    with open(os.path.join(root, relative), encoding="utf-8") as handle:
        return handle.read()


def merged(root):
    """(current text of en.json, the merged canonical text or None, problems, parts)."""
    path = os.path.join(root, CATALOGUE)
    current = read(root, CATALOGUE) if os.path.exists(path) else None
    texts, problems = flatten(current if current is not None else "{}", CATALOGUE)
    origin = dict.fromkeys(texts, CATALOGUE)
    parts = part_files(root)
    for part in parts:
        flat, found = flatten(read(root, part), part)
        problems += found
        for key, text in flat.items():
            if key in texts and texts[key] != text:
                problems.append(
                    f"conflict: `{key}` is {texts[key]!r} in {origin[key]} "
                    f"and {text!r} in {part}"
                )
            elif key not in texts:
                texts[key] = text
                origin[key] = part
    if problems:
        return current, None, problems, parts
    try:
        return current, dump(nest(texts)), [], parts
    except ValueError as error:
        return current, None, [f"{CATALOGUE} with the parts: {error}"], parts


def main(argv):
    parser = argparse.ArgumentParser(description="Merge locales/parts/*.json into locales/en.json.")
    parser.add_argument("--root", default=".", help="the repository root (default: .)")
    parser.add_argument("--check", action="store_true", help="write nothing; exit 1 if stale")
    parser.add_argument("--prune", action="store_true", help="delete the merged part files")
    args = parser.parse_args(argv)
    if args.check and args.prune:
        parser.error("--check and --prune exclude each other")
    current, result, problems, parts = merged(args.root)
    for problem in problems:
        print(f"!! {problem}")
    if problems:
        print(f"!! nothing written; {len(problems)} problem(s)")
        return 1
    if args.check:
        if current != result:
            print(f"!! {CATALOGUE} is not merged or not canonical: "
                  "run python3 scripts/lib/merge_locales.py")
            return 1
        print(f"{CATALOGUE} is up to date")
        return 0
    if current != result:
        with open(os.path.join(args.root, CATALOGUE), "w", encoding="utf-8") as handle:
            handle.write(result)
        print(f"{CATALOGUE} written from {len(parts)} part(s)")
    else:
        print(f"{CATALOGUE} is up to date ({len(parts)} part(s))")
    if args.prune:
        for part in parts:
            os.remove(os.path.join(args.root, part))
        print(f"removed {len(parts)} part file(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
