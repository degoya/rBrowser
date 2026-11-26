"""The planning's job ids out of the text a reader of the wiki sees (RB-090-12, RB-099-22).

    python3 -I -B scripts/lib/wiki_text.py <tree>   # every job id left in the prose of its pages

A job id (`RB-094-23`) is the planning's, not the reader's (owner, 2026-11-25): the generated
reference pages (scripts/export-wiki.sh, scripts/mcp-reference.sh) and the public handbook
(scripts/export-public-wiki.sh) carry none. The descriptions they are made of keep theirs for
the developer; `public_text` drops what reads as a reference - a parenthesis of nothing but job
ids, a job id inside another parenthesis with its separator - and `job_ids_left` names every one
it could not drop, which the caller refuses. In a handbook page, code - a fenced block or an
inline span - is kept as written and not checked (`outside_code`, `prose_job_ids`).
"""

import os
import re
import sys

JOB = r"RB-\d{3}-\d{2}"
JOB_ID = re.compile(rf"\b{JOB}\b")

# In order: `(RB-010-05)`, `*(RB-010-05, RB-020-01)*` and `(RB-030-04/RB-030-05)` go whole with
# the space before; `(a print view, RB-080-10)` loses `, RB-080-10`; `(RB-092-01, "Play from
# here")` loses `RB-092-01, `.
JOB_CLEANUP = [
    (re.compile(rf"\s*\*?\((?:{JOB})(?:(?:,\s*|\s+and\s+|/){JOB})*\)\*?"), ""),
    (re.compile(rf"[,;]\s*{JOB}(?=[^()]*\))"), ""),
    (re.compile(rf"(?<=\()\s*{JOB}[,;]\s*"), ""),
]


def public_text(text: str) -> str:
    """`text` without the job ids that read as a reference."""
    for pattern, replacement in JOB_CLEANUP:
        text = pattern.sub(replacement, text)
    return text


INLINE_CODE = re.compile(r"`[^`]*`")


def outside_code(text: str, clean) -> str:
    """`text` with `clean` applied to everything but its inline code spans."""
    out, at = [], 0
    for span in INLINE_CODE.finditer(text):
        out.append(clean(text[at:span.start()]))
        out.append(span.group(0))
        at = span.end()
    out.append(clean(text[at:]))
    return "".join(out)


def prose_job_ids(text: str, name: str) -> list[str]:
    """`job_ids_left` of a Markdown page's prose: fenced blocks and inline code spans aside."""
    found, fenced = [], False
    for number, line in enumerate(text.split("\n"), 1):
        if line.lstrip().startswith(("```", "~~~")):
            fenced = not fenced
            continue
        if fenced:
            continue
        for match in JOB_ID.finditer(INLINE_CODE.sub("", line)):
            found.append(f"{name}:{number}: names the job id {match.group(0)}")
    return found


def job_ids_left(text: str, name: str) -> list[str]:
    """`<name>:<line>: names the job id <id>` for every job id in `text`."""
    return [
        f"{name}:{number}: names the job id {match.group(0)}"
        for number, line in enumerate(text.split("\n"), 1)
        for match in JOB_ID.finditer(line)
    ]


def main(argv: list[str]) -> int:
    """Names every job id in the prose of the `.md` pages below `argv[1]`; exit 1 when any."""
    if len(argv) != 2 or not os.path.isdir(argv[1]):
        print("usage: wiki_text.py <tree>", file=sys.stderr)
        return 2
    found = []
    for directory, subdirectories, names in os.walk(argv[1]):
        subdirectories.sort()
        for page in sorted(n for n in names if n.endswith(".md")):
            path = os.path.join(directory, page)
            with open(path, encoding="utf-8") as handle:
                found += prose_job_ids(handle.read(), os.path.relpath(path, argv[1]))
    for line in found:
        print(f"{line}, which the public handbook does not carry", file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
