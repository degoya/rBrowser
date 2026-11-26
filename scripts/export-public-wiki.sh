#!/usr/bin/env bash
#
# Exports the user handbook to the public repository's GitHub wiki (RB-090-12). Copied and
# adapted from rCommander's scripts/export-wiki.sh (RC-100-30), itself from rDownloader's; named
# apart because scripts/export-wiki.sh here writes the generated reference pages into the
# private wiki.
#
# The handbook is written in the private wiki (~/projects/rbrowser.wiki, GitLab form: page
# folders, `home.md`, `_sidebar.md`, links with folder paths). The public one is
# github.com/degoya/rBrowser/wiki, whose git repository is rBrowser.wiki.git. This script takes
# `git archive HEAD` of the private wiki - its committed state, never an unsaved edit - converts
# it to GitHub-wiki form (scripts/lib/public_wiki.py), refuses a private name
# (scripts/public-names.txt) outside the `wiki/` entries of scripts/public-names-allow.txt and a
# link into what scripts/public-exclude.txt leaves out and a job id left after the conversion,
# warns about every remaining mention of `docs/` and of the internal test, scans it with gitleaks
# and replaces the tree of a local clone of the public wiki with it, keeping that clone's `.git`.
# The commit is "Handbook for <version>", a snapshot: the private wiki's history does not come along.
#
# The conversion:
#   * home.md becomes Home.md and _sidebar.md _Sidebar.md (at the root; GitHub's names).
#   * A page link loses its folder path: `(getting-started/installation)` and
#     `(../using/x.md#a)` become `(installation)` and `(x#a)`. GitHub resolves a page by its
#     base name in any folder, so the folders stay - and two pages sharing a base name are
#     refused, because one of them could never be linked.
#   * An image or other file link becomes relative to the wiki root: `../images/a.png` and
#     `images/a.png` both become `images/a.png`, which GitHub serves for every page.
#   * A link to a page or file that does not exist is refused rather than exported dead, and so
#     is a link to the source repository at a path scripts/public-exclude.txt leaves out - all of
#     docs/ among them.
#   * The GitLab-only `.gitlab/` folder is left out.
#
# Private material stays in the one source, marked, and never leaves it:
#   * A page whose first line is `<!-- private page -->` is left out, and so is every list item
#     of `_sidebar.md` / `_footer.md` that links to it.
#   * The lines from a `<!-- private -->` line to the next `<!-- /private -->` line, both
#     included, are removed from the page. Each marker is a line of its own.
#   * Refused: a public page linking to a private page or to a heading inside a removed section,
#     an unclosed, nested or unopened marker, a page marker below the first line, and any marker
#     text left in the converted tree.
#
# Nothing leaves this machine without --push, and every push is the owner's word. A pre-release
# (`X.Y.Z-beta.N`) is refused: the public handbook describes the stable release and follows the
# next one.
#
# Usage:
#   scripts/export-public-wiki.sh 0.9.0              # convert, commit into the local clone
#   scripts/export-public-wiki.sh 0.9.0 --push       # ... and push it
#   scripts/export-public-wiki.sh 0.9.0 --out <dir>  # a dry run: the converted tree into the new
#                                                    # or empty <dir>, every check reported; no
#                                                    # clone, no commit, no push; exit 1 on a
#                                                    # finding
#
# Environment:
#   RB_WIKI                  the private wiki (default: rbrowser.wiki beside the main checkout,
#                            found from a worktree too, as scripts/export-wiki.sh does)
#   RB_PUBLIC_WIKI_DIR       the local clone (default: ~/projects/rBrowser-public.wiki)
#   RB_PUBLIC_WIKI_REMOTE    where to clone it from (default: git@github.com:degoya/rBrowser.wiki.git)
#   RB_PUBLIC_WIKI_BRANCH    the branch GitHub renders (default: master)
#   GITLEAKS                 the gitleaks binary, as for export-public.sh
#   RB_PUBLIC_LOCK_WAIT      seconds to wait for another export in the same clone (default: 600)
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/public.sh
source "$ROOT/scripts/lib/public.sh"
cd "$ROOT"

if [[ -z "${RB_WIKI:-}" ]]; then
    # The main checkout, also from a worktree: the parent of the common git directory.
    common="$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2> /dev/null || echo "$ROOT/.git")"
    RB_WIKI="$(dirname "$(dirname "$common")")/rbrowser.wiki"
fi
WIKI_SRC="$RB_WIKI"
WIKI_DIR="${RB_PUBLIC_WIKI_DIR:-$HOME/projects/rBrowser-public.wiki}"
WIKI_REMOTE="${RB_PUBLIC_WIKI_REMOTE:-git@github.com:degoya/rBrowser.wiki.git}"
WIKI_BRANCH="${RB_PUBLIC_WIKI_BRANCH:-master}"

VERSION=""
DO_PUSH=0
OUT=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --push) DO_PUSH=1; shift ;;
        --out) OUT="${2:?--out needs a directory}"; shift 2 ;;
        -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
        -*) echo "unknown argument: $1" >&2; exit 2 ;;
        *)
            [[ -z "$VERSION" ]] || { echo "unexpected argument: $1" >&2; exit 2; }
            VERSION="$1"; shift ;;
    esac
done
if ! rb_release_version "$VERSION"; then
    echo "usage: scripts/export-public-wiki.sh <version> [--push | --out <dir>]" >&2
    exit 2
fi
if rb_is_prerelease "$VERSION"; then
    echo "$VERSION is a pre-release: the public wiki stays on the last stable release until the next one" >&2
    exit 2
fi
if [[ -n "$OUT" ]]; then
    [[ "$DO_PUSH" -eq 0 ]] || { echo "--out is a dry run; drop --push" >&2; exit 2; }
    if [[ -e "$OUT" ]] && [[ ! -d "$OUT" || -n "$(ls -A "$OUT")" ]]; then
        echo "--out takes a new or empty directory: $OUT" >&2
        exit 2
    fi
fi

SOURCE_COMMIT="$(git -C "$WIKI_SRC" rev-parse --verify --quiet HEAD)" \
    || { echo "$WIKI_SRC is not a git checkout with a commit" >&2; exit 1; }
rb_public_find_gitleaks
[[ -n "$OUT" ]] || rb_public_find_author "$ROOT"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/rb-wiki-export.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

echo "==> exporting the handbook at ${SOURCE_COMMIT:0:12} to a staging tree"
git -C "$WIKI_SRC" archive --format=tar HEAD | tar -x -C "$STAGE"
rm -rf "${STAGE:?}/.gitlab"

# A dry run reports every check and goes on; each runs in a subshell, whose refusal is its exit.
FOUND=()
check() {
    if [[ -n "$OUT" ]]; then
        ("$@") || FOUND+=("$1")
    else
        "$@"
    fi
}

convert() {
    echo "==> converting to GitHub-wiki form"
    python3 -I -B "$ROOT/scripts/lib/public_wiki.py" "$STAGE"
}
check convert

# The handbook is public; the developer documentation under docs/ and whatever else
# scripts/public-exclude.txt names is not, so a link to it would be dead on GitHub.
check rb_public_check_links "$STAGE" "$ROOT/scripts/public-exclude.txt" --wiki
check rb_public_check_names "$STAGE" --wiki
check rb_public_scan "$STAGE"

# Named in prose, a path into docs/ or the internal test package is no link, but the public
# reader can follow neither of them: a warning. A job id is refused (job_ids below).
warn() {
    local what="$1" pattern="$2" warnings
    echo "==> $what that remain (a warning, not a refusal)"
    warnings="$(cd "$STAGE" && { grep -rnE "$pattern" --include='*.md' . || true; })"
    if [[ -n "$warnings" ]]; then
        echo "WARNING: $(wc -l <<< "$warnings") line(s) of the handbook name $what:"
        while IFS= read -r line; do echo "    ${line:0:200}"; done <<< "$warnings"
    else
        echo "    none"
    fi
}
warn "docs/" '(^|[^[:alnum:]_/.-])docs/'

# A job id is the planning's, not the reader's (owner, 2026-11-25): the conversion drops the
# parentheses that carry them, and one left in prose refuses the export, named by page and line.
# Code - a fenced block or an inline span - is kept as written and not checked, as the
# conversion leaves it (scripts/lib/wiki_text.py).
job_ids() {
    echo "==> job ids that remain"
    python3 -I -B "$ROOT/scripts/lib/wiki_text.py" "$STAGE" && echo "    none"
}
check job_ids
warn "the internal test" '[Ii]nternal (test|testing|GitLab|source|package|ZIP)|team ZIP'

if [[ -n "$OUT" ]]; then
    mkdir -p "$OUT"
    cp -a "$STAGE"/. "$OUT"/
    echo "==> dry run: $(find "$OUT" -type f | wc -l) files of the handbook at ${SOURCE_COMMIT:0:12} in $OUT; nothing committed or pushed"
    if [[ "${#FOUND[@]}" -gt 0 ]]; then
        echo "!! the export would be refused by: ${FOUND[*]}" >&2
        exit 1
    fi
    exit 0
fi

rb_public_prepare_clone "$WIKI_DIR" "$WIKI_REMOTE" "$WIKI_BRANCH"
rb_public_replace_tree "$WIKI_DIR" "$STAGE"

if pub rev-parse --verify --quiet HEAD > /dev/null && pub diff --cached --quiet HEAD; then
    echo "==> the public wiki already carries exactly this handbook"
else
    pub -c user.name="$AUTHOR_NAME" -c user.email="$AUTHOR_EMAIL" commit --quiet \
        -m "Handbook for $VERSION"
    echo "==> committed $(pub rev-parse --short HEAD) on $WIKI_BRANCH"
fi

if [[ "$DO_PUSH" -eq 1 ]]; then
    rb_public_verify_push HEAD
    pub push origin "$WIKI_BRANCH"
    echo "==> pushed $WIKI_BRANCH to $WIKI_REMOTE"
else
    echo "==> not pushed. Publish with: git -C $WIKI_DIR push origin $WIKI_BRANCH"
fi
