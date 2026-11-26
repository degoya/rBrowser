#!/usr/bin/env bash
#
# The real lists of the public exports (RB-090-12): scripts/public-exclude.txt leaves out the
# internal documentation and planning, the agents' card and configuration, the private names'
# own list and the maintainer's workflow, and nothing it names is an absolute path or climbs out
# with `..`; scripts/public-names.txt compiles and guards the internal host, the owner's home
# and the session folders; every entry of scripts/public-names-allow.txt is still needed by a
# line of the tree (rCommander's --unused rule). The exports' behaviour on a scratch repository is
# scripts/tests/export-public.sh and scripts/tests/export-public-wiki.sh.
#
#   scripts/tests/public-lists.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
EXCLUDE="$ROOT/scripts/public-exclude.txt"
NAMES="$ROOT/scripts/public-names.txt"
ALLOW="$ROOT/scripts/public-names-allow.txt"

# The entries as export-public.sh reads them: comments and blanks dropped, a trailing `/` kept.
entries="$(sed 's/#.*//; s/^[[:space:]]*//; s/[[:space:]]*$//; /^$/d' "$EXCLUDE")"
listed() { grep -qxF "$1" <<< "$entries"; }

for path in docs/ ROADMAP.md AGENTS.md CLAUDE.md .claude/ scripts/public-names.txt \
    scripts/tests/guard-bash.sh artifacts/ scripts/archive-jobs.sh scripts/session-state.sh \
    scripts/worktree.sh; do
    expect_true "the export leaves out $path" 'listed "$path"'
done
for path in README.md LICENSE CHANGELOG.md sdk/ scripts/export-public.sh scripts/public-exclude.txt \
    scripts/public-names-allow.txt scripts/lib/public.sh .github/ packaging/public/README.md; do
    expect_true "the export keeps $path" '! listed "$path" && ! listed "${path%/}"'
done
# The public repository's README, which export-public.sh puts in place of the private one; in
# the public export (no private names list) it is README.md itself.
if [[ -f "$NAMES" ]]; then
    expect_true "packaging/public/README.md exists" '[[ -f "$ROOT/packaging/public/README.md" ]]'
fi
expect "no entry is absolute or climbs out" "" "$(grep -E '^/|\.\.' <<< "$entries" || true)"
expect "no entry is listed twice" "" "$(sort <<< "$entries" | uniq -d)"
# What the exports read must stay public, or a public export could not run at all.
for script in export-public.sh export-public-wiki.sh lib/public.sh lib/public_wiki.py \
    lib/public-links.py lib/public-names.py lib/wiki_text.py public-names-allow.txt; do
    expect_true "scripts/$script exists and stays public" \
        '[[ -f "$ROOT/scripts/$script" ]] && ! listed "scripts/$script"'
done

# The private names stay in the private repository (the list names what it guards): the public
# export has no scripts/public-names.txt, and nothing to hold here.
if [[ ! -f "$NAMES" ]]; then
    echo "skip the private names: no scripts/public-names.txt in this tree (the public export)"
else
    run_status python3 -I -c 'import re, sys
for line in open(sys.argv[1], encoding="utf-8"):
    line = line.strip()
    if line and not line.startswith("#"):
        re.compile(line, re.IGNORECASE)' "$NAMES"
    expect_status "every private name is a regular expression" 0
    # Built from pieces, so that this public file names none of them itself.
    for probe in 'https://git.dego''ya.net/x' '/home/dego''ya/projects' '/var/tmp/rbrowser-ses''sion/x' \
        '/tmp/claude-''1000/job/x.log' '/mnt/c/User''s/someone'; do
        run_status python3 -I -c 'import re, sys
terms = [l.strip() for l in open(sys.argv[1], encoding="utf-8") if l.strip() and not l.startswith("#")]
sys.exit(0 if any(re.search(t, sys.argv[2], re.IGNORECASE) for t in terms) else 1)' "$NAMES" "$probe"
        expect_status "the private names catch $probe" 0
    done

    # Every allow entry is needed by a line of the tree; the private names themselves are
    # reported by the export (and its --out dry run), not here.
    run_status python3 -I -B "$ROOT/scripts/lib/public-names.py" "$ROOT" --terms "$NAMES" \
        --allow "$ALLOW" --exclude "$EXCLUDE" --unused
    expect "no allow entry outlived its line" "" "$(grep -F 'no line needs this entry' <<< "$output" || true)"
fi

finish_tests public-lists
