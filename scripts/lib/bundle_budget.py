#!/usr/bin/env python3
"""The shell's bundle budget (RB-099-20), run by scripts/bundle-budget.sh after `npm run build`
in ui/ (check.sh --preflight):

    python3 scripts/lib/bundle_budget.py <dist dir> <budget file>

Every page of the build (`<dist>/*.html`) loads its entry script, the chunks it preloads and its
stylesheets at once; their raw sizes, summed per page as JS and CSS, must stay within the page's
budget in the budget file (`ui/bundle-budget.json`):

    {"pages": {"index.html": {"js": <bytes>, "css": <bytes>}, ...}}

A chunk loaded later (a route's `import()`) is not counted. A page without a budget, a budget for
a page the build lacks, and a file a page names but the build lacks fail too. Exit 1 with one line
per problem, naming the page and its largest files; 0 with one line per page.
"""

import json
import os
import re
import sys

ASSET = re.compile(r"""(?:src|href)\s*=\s*["']\./(assets/[^"'?#]+\.(js|css))["']""")
LARGEST = 3


def page_assets(dist, page):
    """{"js": [...], "css": [...]} of the asset paths `page` loads at once, each once."""
    with open(os.path.join(dist, page), encoding="utf-8") as handle:
        html = handle.read()
    found = {"js": [], "css": []}
    for match in ASSET.finditer(html):
        path, kind = match.group(1), match.group(2)
        if path not in found[kind]:
            found[kind].append(path)
    return found


def read_budget(path, problems):
    try:
        with open(path, encoding="utf-8") as handle:
            data = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: cannot read the budget: {error}")
        return {}
    pages = data.get("pages") if isinstance(data, dict) else None
    if not isinstance(pages, dict):
        problems.append(f"{path}: no \"pages\" object")
        return {}
    budget = {}
    for page, limits in pages.items():
        if (not isinstance(limits, dict) or set(limits) != {"js", "css"}
                or not all(isinstance(limits[k], int) and limits[k] > 0 for k in ("js", "css"))):
            problems.append(f"{path}: {page}: js and css must be byte counts")
            continue
        budget[page] = limits
    return budget


def check(dist, budget_path):
    problems, lines = [], []
    if not os.path.isdir(dist):
        return [f"no build at {dist}: run npm run build in ui/ first"], lines
    budget = read_budget(budget_path, problems)
    pages = sorted(name for name in os.listdir(dist) if name.endswith(".html"))
    if not pages:
        problems.append(f"no page (*.html) in {dist}: run npm run build in ui/ first")
    for page in pages:
        if page not in budget:
            problems.append(f"{page}: no budget in {budget_path}")
            continue
        sizes = {}
        for kind, paths in page_assets(dist, page).items():
            files = []
            for path in paths:
                full = os.path.join(dist, path)
                if not os.path.isfile(full):
                    problems.append(f"{page}: {path} is missing")
                    continue
                files.append((os.path.getsize(full), path))
            total = sum(size for size, _ in files)
            sizes[kind] = total
            limit = budget[page][kind]
            if total > limit:
                largest = ", ".join(f"{path} {size}" for size, path in sorted(files, reverse=True)[:LARGEST])
                problems.append(f"{page}: {kind.upper()} {total} bytes is over its budget of {limit} "
                                f"({largest})")
        lines.append(f"{page}: JS {sizes['js']} of {budget[page]['js']} bytes, "
                     f"CSS {sizes['css']} of {budget[page]['css']} bytes")
    problems += [f"{page}: a budget, but the build has no such page"
                 for page in sorted(budget) if page not in pages]
    return problems, lines


def main(argv):
    if len(argv) != 2:
        print("usage: bundle_budget.py <dist dir> <budget file>", file=sys.stderr)
        return 2
    problems, lines = check(argv[0], argv[1])
    for line in lines:
        print(line)
    for problem in problems:
        print(f"!! {problem}")
    if problems:
        print(f"bundle budget: {len(problems)} problem(s)")
        return 1
    print(f"bundle budget: {len(lines)} page(s) within budget")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
