#!/usr/bin/env bash
#
# Exports a release to the public repository (RB-090-12). Copied and adapted from rCommander's
# scripts/export-public.sh (RC-100-30), itself from rDownloader's.
#
# Development happens in the private repository; github.com/degoya/rBrowser carries one fresh
# commit per release and no history. This script builds that commit: it takes `git archive` of
# the release tag, puts packaging/public/README.md in place of the private README.md (RB-090-24;
# a tree without it is refused), drops what scripts/public-exclude.txt names, refuses a link from
# what is left into what was dropped and a private name (scripts/public-names.txt) outside
# scripts/public-names-allow.txt, scans the rest with gitleaks, lists every remaining reference
# to the internal planning as a warning, and replaces the tree of a local clone of the public
# repository with it - never touching the clone's `.git`. The commit is "Release <version>",
# authored by the git identity of this repository, and tagged v<version>.
#
# One export works in the clone at a time (a lock in its .git, lib/public.sh), and before a push
# the committed tree is compared with the one staged. A second call finds the tag with exactly
# this tree and only pushes. A tag staged here that never reached the public repository is
# replaced when the tree changed since; one the public repository has is never moved.
#
# Nothing leaves this machine without --push, and every push is the owner's word: the public
# repository is what strangers read, so publishing it is a step somebody takes on purpose, not a
# side effect of exporting. No push here is a force push but --branch's, on a ci/ branch.
#
# Usage:
#   scripts/export-public.sh 0.9.0                     # export v0.9.0 into the clone, commit, tag
#   scripts/export-public.sh 0.9.0 --push              # ... and push main and the tag
#   scripts/export-public.sh 0.9.0 --ref <commit>      # export another ref than v0.9.0
#   scripts/export-public.sh 0.9.0 --branch ci/0.9.0   # an unreleased export (default ref: HEAD),
#                                                      # committed to that branch and pushed there
#   scripts/export-public.sh 0.9.0 --branch ci/0.9.0 --skip-push-ci
#                                                      # ... with no CI started by the push itself
#   scripts/export-public.sh 0.9.0 --out <dir>         # a dry run (default ref: HEAD): the tree
#                                                      # into the empty or new <dir>, every check
#                                                      # reported rather than stopping at the
#                                                      # first; no clone, no commit, no push
#
# A pre-release (`X.Y.Z-beta.N`) is exported like a release: the commit "Release <version>" on
# main, tagged there; the public main and its README show the beta until the stable release
# replaces it.
#
# The user handbook is not part of this tree; it goes to the repository's GitHub wiki through
# scripts/export-public-wiki.sh.
#
# --branch is for CI: it lets the public workflows run on a tree before it is a release. It
# starts the branch afresh from the clone's main each time, commits without a tag, and
# force-pushes the branch, because a re-export replaces the previous one. It pushes by itself -
# naming a branch is the explicit request - and only a branch under ci/, never main or a tag.
#
# --skip-push-ci puts `[skip ci]` into that commit's message, which GitHub honours for push
# events only: for a run started by hand for fewer platforms, so the push does not start a second.
#
# --out exits 1 when any check found something, after writing the whole tree, so the findings
# and the tree can be read together before the first push.
#
# Environment:
#   RB_PUBLIC_DIR     the local clone (default: ~/projects/rBrowser-public)
#   RB_PUBLIC_REMOTE  where to clone it from when missing (default: git@github.com:degoya/rBrowser.git)
#   RB_PUBLIC_LOCK_WAIT  seconds to wait for another export in the same clone (default: 600)
#   RB_PUBLIC_FETCH_PAUSE  seconds between the three tries of the clone's fetch (default: 10)
#   GITLEAKS          the gitleaks binary (default: `gitleaks` on PATH; nothing else is looked for)
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/public.sh
source "$ROOT/scripts/lib/public.sh"
cd "$ROOT"

PUBLIC_DIR="${RB_PUBLIC_DIR:-$HOME/projects/rBrowser-public}"
PUBLIC_REMOTE="${RB_PUBLIC_REMOTE:-git@github.com:degoya/rBrowser.git}"
PUBLIC_MAIN="main"

VERSION=""
REF=""
BRANCH=""
DO_PUSH=0
SKIP_PUSH_CI=0
OUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --push) DO_PUSH=1; shift ;;
        --ref) REF="${2:?--ref needs a ref}"; shift 2 ;;
        --branch) BRANCH="${2:?--branch needs a branch name}"; shift 2 ;;
        --skip-push-ci) SKIP_PUSH_CI=1; shift ;;
        --out) OUT="${2:?--out needs a directory}"; shift 2 ;;
        -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
        -*) echo "unknown argument: $1" >&2; exit 2 ;;
        *)
            [[ -z "$VERSION" ]] || { echo "unexpected argument: $1" >&2; exit 2; }
            VERSION="$1"; shift ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "usage: scripts/export-public.sh <version> [--push] [--ref <ref>] [--branch ci/<name> [--skip-push-ci]] [--out <dir>]" >&2
    exit 2
fi
if ! rb_release_version "$VERSION"; then
    echo "not a release version: $VERSION (X.Y.Z, or X.Y.Z-beta.N for a pre-release)" >&2
    exit 2
fi
if [[ "$SKIP_PUSH_CI" -eq 1 && -z "$BRANCH" ]]; then
    echo "--skip-push-ci goes with --branch" >&2
    exit 2
fi
if [[ -n "$BRANCH" ]]; then
    [[ "$DO_PUSH" -eq 0 ]] || { echo "--branch pushes by itself; drop --push" >&2; exit 2; }
    [[ "$BRANCH" == ci/?* ]] || { echo "--branch takes a ci/ branch: $BRANCH" >&2; exit 2; }
    git check-ref-format --branch "$BRANCH" > /dev/null \
        || { echo "not a branch name: $BRANCH" >&2; exit 2; }
fi

if [[ -n "$OUT" ]]; then
    [[ "$DO_PUSH" -eq 0 && -z "$BRANCH" ]] || { echo "--out is a dry run; drop --push and --branch" >&2; exit 2; }
    if [[ -e "$OUT" ]] && [[ ! -d "$OUT" || -n "$(ls -A "$OUT")" ]]; then
        echo "--out takes a new or empty directory: $OUT" >&2
        exit 2
    fi
fi

TAG="v$VERSION"
if [[ -z "$REF" ]]; then
    if [[ -n "$BRANCH" || -n "$OUT" ]]; then REF="HEAD"; else REF="$TAG"; fi
fi
COMMIT="$(git rev-parse --verify --quiet "$REF^{commit}")" \
    || { echo "no such commit in this repository: $REF" >&2; exit 1; }

# --- the tools, before anything is written ---------------------------------------------------

rb_public_find_gitleaks
[[ -n "$OUT" ]] || rb_public_find_author "$ROOT"

# --- the tree --------------------------------------------------------------------------------

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/rb-public-export.XXXXXX")"
LISTS="$(mktemp -d "${TMPDIR:-/tmp}/rb-public-lists.XXXXXX")"
trap 'rm -rf "$STAGE" "$LISTS"' EXIT

echo "==> exporting $REF (${COMMIT:0:12}) to a staging tree"
git archive --format=tar "$COMMIT" | tar -x -C "$STAGE"

# What is left out and which names are private is the exported commit's own word: the
# lists of the working tree belong to whatever is checked out, which need not be $REF. A commit
# from before a list existed takes the working tree's, and says so.
for list in public-exclude.txt public-names.txt public-names-allow.txt; do
    if [[ -f "$STAGE/scripts/$list" ]]; then
        cp "$STAGE/scripts/$list" "$LISTS/$list"
    elif [[ -f "$ROOT/scripts/$list" ]]; then
        echo "    note: $REF has no scripts/$list; taking the working tree's"
        cp "$ROOT/scripts/$list" "$LISTS/$list"
    fi
done
EXCLUDE_LIST="$LISTS/public-exclude.txt"
[[ -f "$EXCLUDE_LIST" ]] || { echo "no scripts/public-exclude.txt in $REF or the working tree" >&2; exit 1; }
export RB_PUBLIC_LISTS="$LISTS"

# A dry run reports every check and goes on; each runs in a subshell, whose refusal is its exit.
FOUND=()
check() {
    if [[ -n "$OUT" ]]; then
        ("$@") || FOUND+=("$1")
    else
        "$@"
    fi
}

# The public README in place of the private one, before anything is left out or checked: the
# exclude list and every check below see the tree as it will be published.
check rb_public_readme "$STAGE"
rb_public_exclude "$STAGE" "$EXCLUDE_LIST"

# What stays must not point at what went: a README link into docs/ would be a 404 on GitHub.
check rb_public_check_links "$STAGE" "$EXCLUDE_LIST"
check rb_public_check_names "$STAGE"

# From inside the tree, so that its own .gitleaks.toml applies and its paths match the ones CI
# sees in the public repository.
check rb_public_scan "$STAGE"

echo "==> references to the internal planning that remain (a warning, not a refusal)"
warnings="$(cd "$STAGE" && { grep -rnE 'roadmap/jobs|AGENTS\.md|ROADMAP\.md' . || true; })"
if [[ -n "$warnings" ]]; then
    echo "WARNING: $(wc -l <<< "$warnings") line(s) in the export name an internal path:"
    while IFS= read -r line; do echo "    $line"; done <<< "$warnings"
else
    echo "    none"
fi

if [[ -n "$OUT" ]]; then
    mkdir -p "$OUT"
    cp -a "$STAGE"/. "$OUT"/
    echo "==> dry run: $(find "$OUT" -type f | wc -l) files of ${COMMIT:0:12} in $OUT; nothing committed or pushed"
    if [[ "${#FOUND[@]}" -gt 0 ]]; then
        echo "!! the export would be refused by: ${FOUND[*]}" >&2
        exit 1
    fi
    exit 0
fi

# --- the clone -------------------------------------------------------------------------------

# The very first export meets an empty repository: no main anywhere, HEAD unborn.
rb_public_prepare_clone "$PUBLIC_DIR" "$PUBLIC_REMOTE" "$PUBLIC_MAIN"

if [[ -n "$BRANCH" ]]; then
    if pub rev-parse --verify --quiet HEAD > /dev/null; then
        pub checkout --quiet -B "$BRANCH"
    else
        pub symbolic-ref HEAD "refs/heads/$BRANCH"
    fi
fi

rb_public_replace_tree "$PUBLIC_DIR" "$STAGE"

commit_as() {
    pub -c user.name="$AUTHOR_NAME" -c user.email="$AUTHOR_EMAIL" "$@"
}

if [[ -n "$BRANCH" ]]; then
    message="Unreleased export of ${COMMIT:0:12} (towards $VERSION)"
    [[ "$SKIP_PUSH_CI" -eq 0 ]] || message+=" [skip ci]"
    commit_as commit --quiet --allow-empty -m "$message"
    echo "==> committed $(pub rev-parse --short HEAD) on $BRANCH; pushing it"
    rb_public_verify_push HEAD
    pub push --force origin "$BRANCH"
    pub checkout --quiet "$PUBLIC_MAIN" 2> /dev/null || true
    echo "==> pushed $BRANCH"
    exit 0
fi

if pub rev-parse --verify --quiet "refs/tags/$TAG" > /dev/null; then
    # A resumed release runs this again; the same tree under the same tag is done. Either way
    # the clone goes back to its own HEAD: the replaced tree was only the comparison.
    if pub diff --cached --quiet "$TAG"; then
        pub reset --quiet --hard
        echo "==> $TAG is already exported with exactly this tree"
    elif rb_public_unpublished "$TAG" "$PUBLIC_MAIN"; then
        # Staged before the release's tag, then the release's tree changed: the staged commit
        # never left this machine, so it is replaced, not followed by a second "Release".
        pub tag -d "$TAG" > /dev/null
        commit_as commit --quiet --amend --allow-empty -m "Release $VERSION"
        commit_as tag -a "$TAG" -m "Release $VERSION"
        echo "==> $TAG was staged here with another tree and never pushed; replaced by $(pub rev-parse --short HEAD)"
    else
        pub reset --quiet --hard
        echo "$TAG already exists in the public repository with a different tree" >&2
        exit 1
    fi
else
    commit_as commit --quiet --allow-empty -m "Release $VERSION"
    commit_as tag -a "$TAG" -m "Release $VERSION"
    echo "==> committed $(pub rev-parse --short HEAD) on $PUBLIC_MAIN and tagged $TAG"
fi

if [[ "$DO_PUSH" -eq 1 ]]; then
    rb_public_verify_push "$TAG"
    pub push origin "$PUBLIC_MAIN"
    pub push origin "$TAG"
    echo "==> pushed $PUBLIC_MAIN and $TAG to $PUBLIC_REMOTE"
else
    echo "==> not pushed. Publish with: git -C $PUBLIC_DIR push origin $PUBLIC_MAIN $TAG"
fi
