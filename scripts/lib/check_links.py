"""Fails on a relative Markdown link whose target file does not exist.

Checks README.md, ROADMAP.md, CHANGELOG.md and every Markdown file under docs/; external links
(scheme or mailto) and pure anchors are skipped, an anchor after a path is ignored.
"""
import pathlib
import re
import sys

LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")
root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
files = [root / n for n in ("README.md", "ROADMAP.md", "CHANGELOG.md") if (root / n).exists()]
files += sorted((root / "docs").rglob("*.md"))
broken = []
for md in files:
    text = re.sub(r"```.*?```", "", md.read_text(encoding="utf-8"), flags=re.S)
    text = re.sub(r"`[^`]*`", "", text)
    for target in LINK.findall(text):
        if re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith("#"):
            continue
        path = target.split("#", 1)[0]
        if path and not (md.parent / path).exists():
            broken.append(f"{md.relative_to(root)}: {target}")
print("\n".join(broken) if broken else f"links ok ({len(files)} files)")
sys.exit(1 if broken else 0)
