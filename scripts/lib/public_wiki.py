"""Converts a staged copy of the private wiki to GitHub-wiki form, in place (RB-090-12).

    python3 -I -B scripts/lib/public_wiki.py <staged wiki tree>

Run by scripts/export-public-wiki.sh, whose header states the conversion and the private
markers. Copied from the converter inside rCommander's scripts/export-wiki.sh (RC-100-30),
itself from rDownloader's. Prints the counts and exits 0, or names every problem and exits 1.
"""

import os
import posixpath
import re
import sys

if len(sys.argv) != 2 or not os.path.isdir(sys.argv[1]):
    print("usage: public_wiki.py <staged wiki tree>", file=sys.stderr)
    sys.exit(2)
root = sys.argv[1]
RENAMES = {"home.md": "Home.md", "_sidebar.md": "_Sidebar.md", "_footer.md": "_Footer.md"}

for old, new in RENAMES.items():
    if os.path.exists(os.path.join(root, old)):
        os.rename(os.path.join(root, old), os.path.join(root, new))

problems = []
LINK = re.compile(r'(!?\[[^\]]*\])\(([^)\s]+)((?:\s+"[^"]*")?)\)')
# The other two ways Markdown links: a reference definition `[label]: target "title"` on a line
# of its own, and raw HTML `href="..."` / `src="..."`. Each is rewritten and checked like an
# inline link, so neither can carry a private page or a folder path to GitHub unseen.
REFERENCE = re.compile(r'^(\s{0,3}\[[^\]]+\]:\s*)<?([^\s>]+)>?', re.MULTILINE)
HTML = re.compile(r'(\b(?:href|src)\s*=\s*)(["\'])([^"\']*)\2', re.IGNORECASE)
SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
PAGE_MARK, OPEN, CLOSE = "<!-- private page -->", "<!-- private -->", "<!-- /private -->"
MARKER_TEXT = re.compile(r"<!--\s*/?\s*private\b")
HEADING = re.compile(r"^#{1,6}\s+(.*?)\s*#*\s*$")
LIST_ITEM = re.compile(r"^\s*[-*+]\s")


def markdown_files():
    for directory, _, files in os.walk(root):
        for name in sorted(files):
            if name.endswith(".md"):
                full = os.path.join(directory, name)
                yield posixpath.relpath(full, root).replace(os.sep, "/"), full


def stem_of(path):
    return posixpath.splitext(posixpath.basename(path))[0].lower()


def anchor(heading):
    # GitHub's heading id: lower case, punctuation dropped, spaces become hyphens.
    return re.sub(r"[^\w\- ]", "", heading.strip().lower()).replace(" ", "-")


# Private pages go first, so nothing below can see them as pages.
private_pages = {}  # lower-case base name -> path relative to the root
for relative, full in list(markdown_files()):
    with open(full, encoding="utf-8") as handle:
        if handle.readline().strip() == PAGE_MARK:
            private_pages[stem_of(relative)] = relative
            os.remove(full)


def link_targets(text):
    """Every link target of `text`: inline, reference definitions and HTML attributes."""
    return ([m.group(2) for m in LINK.finditer(text)]
            + [m.group(2) for m in REFERENCE.finditer(text)]
            + [m.group(3) for m in HTML.finditer(text)])


def private_page(target):
    if target.startswith("#") or SCHEME.match(target):
        return None
    path = target.partition("#")[0]
    if posixpath.splitext(path)[1] not in ("", ".md"):
        return None
    return private_pages.get(stem_of(path))


# Then the private sections, remembering the heading ids that went with them.
removed_anchors = {}  # lower-case base name -> ids no longer on the page
sections = 0
for relative, full in markdown_files():
    with open(full, encoding="utf-8") as handle:
        lines = handle.read().split("\n")
    key = stem_of(relative)
    out, opened, joined, fenced, kept, gone = [], 0, False, False, set(), set()
    for number, line in enumerate(lines, 1):
        text = line.strip()
        if text == PAGE_MARK:
            problems.append(f"{relative}:{number}: {PAGE_MARK} counts only as the first line")
            continue
        if text == OPEN:
            if opened:
                problems.append(f"{relative}:{number}: private section opened inside the one from line {opened}")
            else:
                opened = number
            continue
        if text == CLOSE:
            if not opened:
                problems.append(f"{relative}:{number}: {CLOSE} without an open private section")
            else:
                opened, joined, sections = 0, True, sections + 1
            continue
        if text.startswith(("```", "~~~")):
            fenced = not fenced
        heading = None if fenced else HEADING.match(line)
        if opened:
            if heading:
                gone.add(anchor(heading.group(1)))
            continue
        if heading:
            kept.add(anchor(heading.group(1)))
        if MARKER_TEXT.search(line):
            # The markers never reach the public wiki, not even quoted in a sentence.
            problems.append(f"{relative}:{number}: marker text outside a marker line would be published")
        if key in ("_sidebar", "_footer") and LIST_ITEM.match(line) \
                and any(private_page(target) for target in link_targets(line)):
            continue
        # One blank line where a section stood, not two.
        if joined and not text and out and not out[-1].strip():
            continue
        joined = False
        out.append(line)
    if opened:
        problems.append(f"{relative}:{opened}: private section never closed")
    if joined or len(out) < len(lines):
        # A section at the end would otherwise leave the page without its final newline.
        while out and not out[-1].strip():
            out.pop()
        out.append("")
    removed_anchors[key] = gone - kept
    with open(full, "w", encoding="utf-8") as handle:
        handle.write("\n".join(out))

pages = {}  # lower-case base name -> path relative to the root
for relative, _ in markdown_files():
    key = stem_of(relative)
    if key in pages:
        problems.append(f"two pages share the base name {posixpath.basename(relative)[:-3]!r}: {pages[key]} and {relative}")
    pages[key] = relative


def refuse_private_anchor(page, key, fragment, target):
    if fragment and fragment.lower() in removed_anchors.get(key, ()):
        problems.append(f"{page}: link into a private section: {target}")


def convert(page, target):
    if SCHEME.match(target):
        return target
    path, _, fragment = target.partition("#")
    if not path:
        refuse_private_anchor(page, stem_of(page), fragment, target)
        return target
    hidden = private_page(target)
    if hidden:
        problems.append(f"{page}: link to the private page {hidden}: {target}")
        return target
    base = posixpath.basename(path)
    stem, extension = posixpath.splitext(base)
    if extension in ("", ".md"):
        found = pages.get(stem.lower())
        if found is None:
            problems.append(f"{page}: link to a page that does not exist: {target}")
            return target
        refuse_private_anchor(page, stem.lower(), fragment, target)
        return posixpath.basename(found)[:-3] + ("#" + fragment if fragment else "")
    resolved = posixpath.normpath(posixpath.join(posixpath.dirname(page), path))
    if resolved.startswith("../") or not os.path.isfile(os.path.join(root, resolved)):
        problems.append(f"{page}: link to a file that does not exist: {target}")
        return target
    return resolved + ("#" + fragment if fragment else "")


# The public reader has no use for the planning's job ids or the internal test's status (RB-090-12;
# rCommander's and rDownloader's handbooks carry neither): a parenthesis of nothing but job ids
# goes, a job id inside another parenthesis goes with its separator, and so does a parenthesis
# that marks something as in the internal test. Prose only; code stays as written.
JOB = r"RB-\d{3}-\d{2}"
PUBLIC_CLEANUP = [
    (re.compile(rf"\s*\*?\((?:{JOB})(?:(?:,\s*|\s+and\s+|/){JOB})*\)\*?"), ""),
    (re.compile(rf"[,;]\s*{JOB}(?=[^()]*\))"), ""),
    (re.compile(rf"(?<=\()\s*{JOB}[,;]\s*"), ""),
    (re.compile(r"\s*\*?\((?:[^()]*?,\s*)?internal test(?:[;,][^()]*)?\)\*?"), ""),
]


def public_text(text):
    for pattern, replacement in PUBLIC_CLEANUP:
        text = pattern.sub(replacement, text)
    return text


for page in sorted(pages.values()):
    full = os.path.join(root, page)
    with open(full, encoding="utf-8") as handle:
        lines = handle.read().split("\n")
    # Links are rewritten per run of prose between code fences, not per line: a link whose
    # text wraps (`[the diagnostics\nlog](../operating/...)`) was missed line by line and reached
    # GitHub as a dead folder link (first real export, 2026-09-25).
    out, prose, fenced = [], [], False

    def flush():
        if prose:
            text = LINK.sub(
                lambda m: f"{m.group(1)}({convert(page, m.group(2))}{m.group(3)})",
                public_text("\n".join(prose)),
            )
            text = REFERENCE.sub(lambda m: m.group(1) + convert(page, m.group(2)), text)
            text = HTML.sub(
                lambda m: f"{m.group(1)}{m.group(2)}{convert(page, m.group(3))}{m.group(2)}", text
            )
            out.append(text)
            prose.clear()

    for line in lines:
        fence = line.lstrip().startswith(("```", "~~~"))
        if fenced or fence:
            flush()
            out.append(line)
            if fence:
                fenced = not fenced
        else:
            prose.append(line)
    flush()
    with open(full, "w", encoding="utf-8") as handle:
        handle.write("\n".join(out))

if problems:
    print("!! the handbook cannot be exported as it is:", file=sys.stderr)
    for problem in problems:
        print("   " + problem, file=sys.stderr)
    sys.exit(1)
print(f"    {len(pages)} pages converted; left out: {len(private_pages)} private pages, "
      f"{sections} private sections")
