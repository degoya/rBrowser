#!/usr/bin/env bash
#
# scripts/export-public-wiki.sh against a scratch wiki (RB-090-12, after rCommander's test): a
# private page and its sidebar line stay behind, a private section is cut out, and every way of
# leaking one - a link to the page, a link to a heading inside the section, a marker that does
# not pair up - refuses the export before anything reaches the clone, as do a private name, a
# link into docs/ of the repository and two pages with one base name. Nothing is pushed without
# --push; a pre-release is refused; a job id left after the conversion is refused with its page
# and line (RB-099-22); --out writes the converted tree, reports every finding and the warning
# (the internal test) and touches no clone.
#
# Local only: the "public" wiki is a bare repository in the scratch directory, gitleaks a stub,
# the private names a made-up list beside a copy of the scripts.
#
#   scripts/tests/export-public-wiki.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

# The scripts, with their own lists: the export reads the private names beside itself.
TOOLS="$SCRATCH/tools"
mkdir -p "$TOOLS/scripts/lib"
cp "$ROOT/scripts/export-public-wiki.sh" "$TOOLS/scripts/"
cp "$ROOT/scripts/lib/public.sh" "$ROOT/scripts/lib/public_wiki.py" \
    "$ROOT/scripts/lib/public-links.py" "$ROOT/scripts/lib/public-names.py" \
    "$ROOT/scripts/lib/wiki_text.py" \
    "$TOOLS/scripts/lib/"
HOST="private.example""-host.test"
printf '%s\n' 'docs/' > "$TOOLS/scripts/public-exclude.txt"
printf '%s\n' "${HOST//./\\.}" > "$TOOLS/scripts/public-names.txt"
printf '# none\n' > "$TOOLS/scripts/public-names-allow.txt"
git -C "$TOOLS" init -q

export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.invalid
export TMPDIR="$SCRATCH"
export RB_WIKI="$SCRATCH/wiki"
export RB_PUBLIC_WIKI_DIR="$SCRATCH/public-wiki"
export RB_PUBLIC_WIKI_REMOTE="$SCRATCH/public-wiki.git"
export GITLEAKS="$SCRATCH/gitleaks"
# The export takes its author from `git config` of this checkout; a machine without one still runs.
export GIT_CONFIG_COUNT=2 GIT_CONFIG_KEY_0=user.name GIT_CONFIG_VALUE_0=test
export GIT_CONFIG_KEY_1=user.email GIT_CONFIG_VALUE_1=test@example.invalid
printf '#!/bin/sh\nexit 0\n' > "$GITLEAKS"
chmod +x "$GITLEAKS"
git init -q --bare -b master "$RB_PUBLIC_WIKI_REMOTE"
remote_head() { git -C "$RB_PUBLIC_WIKI_REMOTE" rev-parse --verify --quiet master || true; }

# A wiki with one of everything: a public page with a private section, a private page, and a
# sidebar that lists both pages.
write_wiki() {
    rm -rf "$RB_WIKI"
    mkdir -p "$RB_WIKI/using"
    cat > "$RB_WIKI/home.md" <<'EOF'
# Home

See [downloads](using/downloads).
EOF
    cat > "$RB_WIKI/_sidebar.md" <<'EOF'
- [Home](home)
- [Downloads](using/downloads)
- [Maintainer notes](using/maintainer)
EOF
    cat > "$RB_WIKI/using/downloads.md" <<'EOF'
# Downloads

Public text.

<!-- private -->
## Release checklist

Only for the maintainer.
<!-- /private -->

## Queue

More public text.
EOF
    cat > "$RB_WIKI/using/maintainer.md" <<'EOF'
<!-- private page -->
# Maintainer notes

Secret process.
EOF
}
commit_wiki() {
    git -C "$RB_WIKI" init -q -b main 2> /dev/null || true
    git -C "$RB_WIKI" add -A
    git -C "$RB_WIKI" commit -q -m wiki
}
export_wiki() { run_status "$TOOLS/scripts/export-public-wiki.sh" 1.0.0; }
exported() { git -C "$RB_PUBLIC_WIKI_REMOTE" show "master:$1" 2> /dev/null; }

write_wiki
commit_wiki
rm -rf "$RB_PUBLIC_WIKI_DIR"
export_wiki
expect_status "a well-formed handbook is exported" 0
expect "without --push nothing is pushed" "" "$(remote_head)"
run_status "$TOOLS/scripts/export-public-wiki.sh" 1.0.0 --push
expect_status "--push publishes it" 0
expect "the public wiki carries the clone's commit" "$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)" "$(remote_head)"
expect "the private page is left out" "" "$(exported using/maintainer.md)"
expect "and its sidebar line with it" "- [Home](Home)|- [Downloads](downloads)|" \
    "$(exported _Sidebar.md | tr '\n' '|')"
expect "the private section is cut, one blank line where it stood" \
    "# Downloads||Public text.||## Queue||More public text.|" "$(exported using/downloads.md | tr '\n' '|')"
expect_true "no marker text reaches the export" '! git -C "$RB_PUBLIC_WIKI_REMOTE" grep -q "private" master'
expect "the commit names the version" "Handbook for 1.0.0" "$(git -C "$RB_PUBLIC_WIKI_REMOTE" log -1 --format=%s master)"

refused() {
    local name="$1" needle="$2"
    commit_wiki
    local before; before="$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)"
    export_wiki
    expect_status "$name: refused" 1
    expect_output "$name: named" "$needle"
    expect "$name: the clone is untouched" "$before" "$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)"
    write_wiki
}

printf '\nAsk the [maintainer](maintainer).\n' >> "$RB_WIKI/home.md"
refused "a link to a private page" "link to the private page using/maintainer.md"

printf '\nSee [the checklist](using/downloads#release-checklist).\n' >> "$RB_WIKI/home.md"
refused "a link into a private section" "link into a private section"

printf '\nAsk the [maintainer][m].\n\n[m]: ../using/maintainer.md\n' >> "$RB_WIKI/home.md"
refused "a reference definition to a private page" "link to the private page using/maintainer.md"

printf '\nSee <a href="using/downloads#release-checklist">the checklist</a>.\n' >> "$RB_WIKI/home.md"
refused "an HTML link into a private section" "link into a private section"

printf '\n<a href="using/maintainer">notes</a>\n' >> "$RB_WIKI/home.md"
refused "an HTML link to a private page" "link to the private page using/maintainer.md"

printf '\nSee [gone][g].\n\n[g]: using/nowhere\n' >> "$RB_WIKI/home.md"
refused "a reference definition to no page" "link to a page that does not exist: using/nowhere"

printf '\n<!-- private -->\nnever closed\n' >> "$RB_WIKI/home.md"
refused "an unclosed section" "Home.md:5: private section never closed"

printf '\n<!-- private -->\n<!-- private -->\nx\n<!-- /private -->\n' >> "$RB_WIKI/home.md"
refused "a nested section" "private section opened inside the one from line 5"

printf '\n<!-- /private -->\n' >> "$RB_WIKI/home.md"
refused "a close without an open" "without an open private section"

printf '\n<!-- private page -->\n' >> "$RB_WIKI/home.md"
refused "a page marker below the first line" "counts only as the first line"

printf '\nWrite `<!-- private -->` to hide a section.\n' >> "$RB_WIKI/home.md"
refused "marker text inside a sentence" "marker text outside a marker line would be published"

printf '\nClone it from %s.\n' "$HOST" >> "$RB_WIKI/home.md"
refused "a private name" "wiki/Home.md:5: names something private"

# Built from pieces, so that this file does not link into docs/ itself: it is exported too.
REPO='https://github.com/degoya/rBrowser'
printf '\nThe [design](%s/blob/main/docs/plugins.md).\n' "$REPO" >> "$RB_WIKI/home.md"
refused "a link into docs/ of the repository" "links to docs, which is not public"

mkdir -p "$RB_WIKI/develop"
printf '# Downloads for developers\n' > "$RB_WIKI/develop/downloads.md"
refused "two pages with one base name" "two pages share the base name 'downloads'"
rm -rf "$RB_WIKI/develop"

printf '\nPending on RB-040-07.\n' >> "$RB_WIKI/home.md"
refused "a job id left in prose" "Home.md:5: names the job id RB-040-07"

printf '\nThe helper (see RB-030-04 for it) is not packaged.\n' >> "$RB_WIKI/home.md"
refused "a job id the cleanup leaves inside a parenthesis" "Home.md:5: names the job id RB-030-04"

# Code is kept as written and not checked: a fenced block and an inline span.
printf '\nRun `rb job RB-030-05` and (`RB-030-06`).\n\n```\nkept (RB-030-07)\n```\n' >> "$RB_WIKI/home.md"
commit_wiki
export_wiki
expect_status "job ids in code are no finding" 0
expect_true "an inline code span keeps its job id" \
    'git -C "$RB_PUBLIC_WIKI_DIR" show HEAD:Home.md | grep -qF "Run \`rb job RB-030-05\` and (\`RB-030-06\`)."'
expect_true "and so does a fenced block" 'git -C "$RB_PUBLIC_WIKI_DIR" show HEAD:Home.md | grep -qxF "kept (RB-030-07)"'
write_wiki

commit_wiki
before="$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)"
run_status "$TOOLS/scripts/export-public-wiki.sh" 1.1.0-beta.1
expect_status "a beta is refused" 2
expect_output "saying why" "1.1.0-beta.1 is a pre-release: the public wiki stays on the last stable release"
expect "and the clone is untouched" "$before" "$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)"
run_status "$TOOLS/scripts/export-public-wiki.sh" 1.1.0-rc.1
expect_status "another pre-release form is no version" 2

# --- the dry run ------------------------------------------------------------------------------
write_wiki
{
    printf '\nThe [queue][q] and <a href="using/downloads#queue">the queue</a>.\n\n[q]: ../using/downloads.md#queue "Queue"\n'
    printf '\n> **Built, in internal testing (0.4, RB-040-02).**\n'
    printf '\nOff by default (RB-030-05). In the views *(0.1, internal test)*; pending on RB-040-07.\n'
    printf '\n```\nkept (RB-030-05)\n```\n'
} >> "$RB_WIKI/home.md"
commit_wiki
before="$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)"
run_status "$TOOLS/scripts/export-public-wiki.sh" 1.0.0 --out "$SCRATCH/dry"
expect_status "--out converts into a new directory, and fails on the job ids left" 1
expect "the converted pages" "Home.md _Sidebar.md using/downloads.md " \
    "$(cd "$SCRATCH/dry" && find . -type f | sed 's#^\./##' | sort | tr '\n' ' ')"
expect_true "a parenthesis of job ids goes, with the internal test's" \
    'grep -qxF "Off by default. In the views; pending on RB-040-07." "$SCRATCH/dry/Home.md"'
expect_true "a job id inside another parenthesis goes" 'grep -qF "testing (0.4).**" "$SCRATCH/dry/Home.md"'
expect_true "code keeps its job ids" 'grep -qxF "kept (RB-030-05)" "$SCRATCH/dry/Home.md"'
expect_true "a reference definition loses its folder" 'grep -qxF "[q]: downloads#queue \"Queue\"" "$SCRATCH/dry/Home.md"'
expect_true "and so does an HTML link" 'grep -qF "<a href=\"downloads#queue\">" "$SCRATCH/dry/Home.md"'
expect_output "a job id left in prose is refused" "Home.md:11: names the job id RB-040-07"
expect_true "one in code is not" '! grep -qF "Home.md:14:" <<< "$output"'
expect_output "by the job id check" "refused by: job_ids"
expect_output "and so is the internal test" "1 line(s) of the handbook name the internal test"
expect "the clone is untouched by it" "$before" "$(git -C "$RB_PUBLIC_WIKI_DIR" rev-parse HEAD)"
run_status "$TOOLS/scripts/export-public-wiki.sh" 1.0.0 --out "$SCRATCH/dry"
expect_status "--out refuses a directory with files in it" 2
write_wiki
printf '\nAsk the [maintainer](maintainer). Clone it from %s.\n' "$HOST" >> "$RB_WIKI/home.md"
commit_wiki
run_status "$TOOLS/scripts/export-public-wiki.sh" 1.0.0 --out "$SCRATCH/dry2"
expect_status "a dry run with findings fails" 1
expect_output "after naming the private page" "link to the private page using/maintainer.md"
expect_output "and the private name" "names something private"
expect_output "and both checks" "refused by: convert rb_public_check_names"

finish_tests export-public-wiki
