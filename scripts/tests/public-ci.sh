#!/usr/bin/env bash
#
# scripts/public-ci.sh without the network (RB-090-12, after rCommander's test): a scratch
# repository with the script and its libraries, a stand-in scripts/export-public.sh that commits onto the ci/ branch of a local
# "public" clone and pushes it to a bare origin, and a stand-in `gh` on PATH that answers from
# files and moves the clock the limits are read on. Checked: the platform names and their runner
# images, the export's arguments for a local branch and for a ci/ name, the dispatch with the
# images still to run, a green run recorded per image and tree with its branch deleted, a second
# call for the same tree running nothing, a red run and a red job ending the wait with the branch
# kept and nothing recorded, no run within the timeout, `gh auth status` timing out (retried,
# exit 1) apart from gh not signed in (exit 2), a run of the same tree another call
# started waited for instead of exported again (and when not), a run whose jobs never got a runner
# started again once and only once, a wrong platform and a wrong ref.
#
# Pure bash and git, a few seconds.
#
#   scripts/tests/public-ci.sh
# shellcheck disable=SC2016,SC2034  # the conditions are single-quoted on purpose: expect_true
# evaluates them, and reads the variables they name
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
TREE="$SCRATCH/repo"
PUBLIC="$SCRATCH/public"
ORIGIN="$SCRATCH/origin.git"
FAKE="$SCRATCH/fake"
BIN="$SCRATCH/bin"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=../lib/public-ci.sh
source "$ROOT/scripts/lib/public-ci.sh"

git_quiet() { git -c user.name=test -c user.email=test@example.invalid -c init.defaultBranch=main "$@"; }

mkdir -p "$TREE/scripts/lib" "$FAKE" "$BIN"
cp "$ROOT/scripts/public-ci.sh" "$TREE/scripts/"
cp "$ROOT/scripts/lib/public-ci.sh" "$ROOT/scripts/lib/workspace-version.sh" "$TREE/scripts/lib/"
cat > "$TREE/scripts/export-public.sh" << 'SH'
#!/usr/bin/env bash
set -euo pipefail
echo "$*" >> "$RB_FAKE/export.log"
branch=""
while [[ $# -gt 0 ]]; do
    case "$1" in --branch) branch="$2"; shift 2 ;; *) shift ;; esac
done
git -C "$RB_PUBLIC_DIR" checkout -q -B "$branch"
git -C "$RB_PUBLIC_DIR" -c user.name=test -c user.email=test@example.invalid commit -q --allow-empty -m "export [skip ci]"
git -C "$RB_PUBLIC_DIR" push -q --force origin "$branch"
SH
printf '%s\n' '[workspace]' '' '[workspace.package]' 'version = "0.9.0-rc.1"' > "$TREE/Cargo.toml"
chmod +x "$TREE"/scripts/*.sh
git_quiet init -q "$TREE"
git_quiet -C "$TREE" add -A
git_quiet -C "$TREE" commit -qm tree
git_quiet -C "$TREE" branch integration/0.9-w1
git_quiet init -q --bare "$ORIGIN"
git_quiet clone -q "$ORIGIN" "$PUBLIC" 2> /dev/null
git_quiet -C "$PUBLIC" commit -q --allow-empty -m first
git_quiet -C "$PUBLIC" push -q origin HEAD:main

# The stand-in gh: `run list` prints $RB_FAKE/runs (already in the --jq shape), moves the
# clock by ten seconds and then puts $RB_FAKE/runs.next, when there, in its place; `run view <id>
# --json jobs` prints $RB_FAKE/jobs-<id>, `run view <id> --log-failed` is noted in failed-log.log; `workflow run` is noted in dispatch.log; `run rerun
# <id>` is noted in rerun.log and puts $RB_FAKE/runs.rerun and jobs-<id>.rerun, when there, in
# the place of runs and jobs-<id> (else the run stays as it was).
cat > "$BIN/gh" << 'SH'
#!/usr/bin/env bash
case "$1 $2" in
    "auth status")
        [[ -e "$RB_FAKE/auth" ]] || exit 0
        echo "$(( $(cat "$RB_FAKE/auth-calls" 2> /dev/null || echo 0) + 1 ))" > "$RB_FAKE/auth-calls"
        cat "$RB_FAKE/auth" >&2; exit 1 ;;
    "workflow run") echo "$*" >> "$RB_FAKE/dispatch.log" ;;
    "run list")
        echo $(( $(cat "$RB_PUBLIC_CI_CLOCK") + 10 )) > "$RB_PUBLIC_CI_CLOCK"
        cat "$RB_FAKE/runs" 2> /dev/null || true
        [[ ! -e "$RB_FAKE/runs.next" ]] || mv "$RB_FAKE/runs.next" "$RB_FAKE/runs" ;;
    "run view")
        if [[ " $* " == *" --log-failed "* ]]; then echo "$3 --repo $5" >> "$RB_FAKE/failed-log.log"; echo "boom"
        else cat "$RB_FAKE/jobs-$3" 2> /dev/null || true; fi ;;
    "run rerun")
        echo "$*" >> "$RB_FAKE/rerun.log"
        [[ ! -e "$RB_FAKE/runs.rerun" ]] || mv "$RB_FAKE/runs.rerun" "$RB_FAKE/runs"
        [[ ! -e "$RB_FAKE/jobs-$3.rerun" ]] || mv "$RB_FAKE/jobs-$3.rerun" "$RB_FAKE/jobs-$3" ;;
    *) echo "unexpected gh $*" >&2; exit 9 ;;
esac
SH
chmod +x "$BIN/gh"

public_ci() {
    echo 0 > "$FAKE/clock"
    rm -f "$FAKE/export.log" "$FAKE/dispatch.log" "$FAKE/failed-log.log" "$FAKE/rerun.log"
    run_status env PATH="$BIN:$PATH" RB_FAKE="$FAKE" RB_PUBLIC_DIR="$PUBLIC" RB_PUBLIC_CI_POLL=0 RB_PUBLIC_CI_AUTH_PAUSE=0 \
        RB_PUBLIC_CI_TIMEOUT=60 RB_PUBLIC_CI_CLOCK="$FAKE/clock" "$TREE/scripts/public-ci.sh" "$@"
}
remote_has() { git -C "$ORIGIN" rev-parse --verify --quiet "refs/heads/$1" > /dev/null; }
VERIFIED="$TREE/.git/rb-verified-ci"
TREE_ID="$(git -C "$TREE" rev-parse 'HEAD^{tree}')"

expect "linux is the Ubuntu image" "ubuntu-24.04" "$(rb_public_ci_images linux | paste -sd' ' -)"
expect "a runner image is itself" "windows-2025 ubuntu-24.04" "$(rb_public_ci_images windows,ubuntu-24.04 | paste -sd' ' -)"
expect "both images are the input all" "all" "$(rb_public_ci_input windows-2025 ubuntu-24.04)"
expect "one image its system" "windows" "$(rb_public_ci_input windows-2025)"

echo "X Timeout trying to log in to github.com account someone (keyring)" > "$FAKE/auth"
public_ci integration/0.9-w1 --platforms windows
expect_status "gh auth status timing out is not finished, not a missing login" 1
expect "and it is tried three times" 3 "$(cat "$FAKE/auth-calls")"
expect_true "and nothing is exported" '[[ ! -e "$FAKE/export.log" ]]'
rm -f "$FAKE/auth-calls"
echo "You are not logged into any GitHub hosts. To log in, run: gh auth login" > "$FAKE/auth"
public_ci integration/0.9-w1 --platforms windows
expect_status "gh not signed in is no usable gh" 2
expect "and it is not tried again" 1 "$(cat "$FAKE/auth-calls")"
rm -f "$FAKE/auth" "$FAKE/auth-calls"

printf '%s\n' "101 completed success https://github.com/x/y/actions/runs/101 CI" > "$FAKE/runs"
public_ci integration/0.9-w1 --platforms windows
expect_status "a green run passes" 0
expect "the branch is exported as ci/ with its slashes flattened, the version without -rc" \
    "0.9.0 --ref $(git -C "$TREE" rev-parse HEAD) --branch ci/integration-0.9-w1 --skip-push-ci" \
    "$(cat "$FAKE/export.log")"
expect_true "ci.yml is started on it for windows" \
    'grep -qF -- "workflow run ci.yml --repo degoya/rBrowser --ref ci/integration-0.9-w1 -f platforms=windows" "$FAKE/dispatch.log"'
expect_true "the green is recorded for the tree and the image" 'grep -q "^$TREE_ID windows-2025 " "$VERIFIED"'
expect_true "and the public branch is deleted" '! remote_has ci/integration-0.9-w1'

public_ci integration/0.9-w1 --platforms windows
expect_status "the same tree again" 0
expect_output "is green already" "nothing was run"
expect_true "without an export" '[[ ! -e "$FAKE/export.log" ]]'

printf '%s\n' "102 completed failure https://github.com/x/y/actions/runs/102 CI" > "$FAKE/runs"
public_ci ci/0.9.0
expect_status "a red run fails" 1
expect_output "and names it" "run 102: failure (https://github.com/x/y/actions/runs/102)"
expect_true "a ci/ name exports HEAD under that name" \
    'grep -qF -- "--ref $(git -C "$TREE" rev-parse HEAD) --branch ci/0.9.0 --skip-push-ci" "$FAKE/export.log"'
expect_true "for Linux only, Windows being green" 'grep -qF -- "-f platforms=linux" "$FAKE/dispatch.log"'
expect_true "its failed steps' logs are read" 'grep -qx "102 --repo degoya/rBrowser" "$FAKE/failed-log.log"'
expect_output "and shown" "boom"
expect_true "the red branch is kept" 'remote_has ci/0.9.0'
expect_true "and nothing is recorded" '! grep -q " ubuntu-24.04 " "$VERIFIED"'

printf '%s\n' "103 in_progress - https://github.com/x/y/actions/runs/103 CI" > "$FAKE/runs"
printf '%s\n' "rust (ubuntu-24.04): failure (https://github.com/x/y/actions/runs/103/job/7)" > "$FAKE/jobs-103"
public_ci ci/0.9.0 --platforms linux
expect_status "a red job ends the wait while its run goes on" 1
expect_output "and names the job" "rust (ubuntu-24.04): failure"
expect_true "whose run's failures are read" 'grep -qx "103 --repo degoya/rBrowser" "$FAKE/failed-log.log"'

: > "$FAKE/runs"
public_ci ci/0.9.0 --platforms linux
expect_status "no run within the timeout fails" 1
expect_output "and says so" "no CI run appeared within 60s"
expect_true "with no log to read" '[[ ! -e "$FAKE/failed-log.log" ]]'

# A run of the same tree that another call on this machine started is waited for, not exported
# again - only while its branch still stands on the marked commit, the marked images cover the
# call's and none of its finished runs is red.
MARKER="$PUBLIC/.git/rb-ci-running/ci-0.9.0"
public_head() { git -C "$ORIGIN" rev-parse "refs/heads/$1"; }
mark() { mkdir -p "${MARKER%/*}"; printf '%s\n' "$*" > "$MARKER"; }
expect_true "a run that never appeared leaves no marker" '[[ ! -e "$MARKER" ]]'

mark "$(public_head ci/0.9.0) $TREE_ID ubuntu-24.04 windows-2025"
printf '%s\n' "104 completed success https://github.com/x/y/actions/runs/104 CI" > "$FAKE/runs"
public_ci ci/0.9.0 --platforms linux
expect_status "a run of the same tree started by another call is waited for" 0
expect_output "saying so" "waiting for it instead of exporting again"
expect_true "without an export" '[[ ! -e "$FAKE/export.log" ]]'
expect_true "or a dispatch" '[[ ! -e "$FAKE/dispatch.log" ]]'
expect_true "its green is recorded" 'grep -q "^$TREE_ID ubuntu-24.04 " "$VERIFIED"'
expect_true "the branch goes" '! remote_has ci/0.9.0'
expect_true "and the marker" '[[ ! -e "$MARKER" ]]'
rm -f "$VERIFIED"

printf '%s\n' "105 completed failure https://github.com/x/y/actions/runs/105 CI" > "$FAKE/runs"
git -C "$PUBLIC" push -q origin HEAD:refs/heads/ci/0.9.0
mark "$(public_head ci/0.9.0) $TREE_ID windows-2025"
public_ci ci/0.9.0 --platforms linux
expect_true "a run for other images: exported" '[[ -e "$FAKE/export.log" ]]'
expect_true "and the new run marked" 'grep -q "^$(public_head ci/0.9.0) $TREE_ID ubuntu-24.04$" "$MARKER"'

mark "$(public_head ci/0.9.0) 0123456789abcdef ubuntu-24.04"
public_ci ci/0.9.0 --platforms linux
expect_true "a run of another tree: exported" '[[ -e "$FAKE/export.log" ]]'

mark "0000000000000000000000000000000000000000 $TREE_ID ubuntu-24.04"
public_ci ci/0.9.0 --platforms linux
expect_true "a branch moved on since: exported" '[[ -e "$FAKE/export.log" ]]'

mark "$(public_head ci/0.9.0) $TREE_ID ubuntu-24.04"
public_ci ci/0.9.0 --platforms linux
expect_true "a run already red: exported again" '[[ -e "$FAKE/export.log" ]]'

# A job GitHub cancelled without a single step never got a runner. Its run is started again once
# it has ended, once; a job that failed, or was cancelled while it ran, is red as before.
url=https://github.com/x/y/actions/runs
starved="no-runner: rust (windows-2025), windows-package"
printf '%s\n' "106 completed cancelled $url/106 CI" > "$FAKE/runs"
printf '%s\n' "106 completed success $url/106 CI" > "$FAKE/runs.rerun"
printf '%s\n' "$starved" > "$FAKE/jobs-106"
: > "$FAKE/jobs-106.rerun"
public_ci ci/0.9.0 --platforms linux
expect_status "a run whose jobs never got a runner, green when run again" 0
expect "is started again once, its failed jobs only" "run rerun 106 --repo degoya/rBrowser --failed" "$(cat "$FAKE/rerun.log")"
expect_output "saying which jobs" "rust (windows-2025), windows-package never got a runner; started again"
expect_true "its green is recorded" 'grep -q "^$TREE_ID ubuntu-24.04 " "$VERIFIED"'

printf '%s\n' "107 completed cancelled $url/107 CI" > "$FAKE/runs"
printf '%s\n' "$starved" > "$FAKE/jobs-107"
public_ci ci/0.9.0 --platforms windows
expect_status "without a runner again after the second start: red" 1
expect "after one new start, not two" 1 "$(wc -l < "$FAKE/rerun.log")"
expect_output "named as that" "run 107: cancelled ($url/107); rust (windows-2025), windows-package never got a runner"
expect_true "the branch is kept" 'remote_has ci/0.9.0'
expect_true "and nothing is recorded" '! grep -q " windows-2025 " "$VERIFIED"'

printf '%s\n' "108 in_progress - $url/108 CI" > "$FAKE/runs"
printf '%s\n' "108 completed failure $url/108 CI" > "$FAKE/runs.next"
printf '%s\n' "108 completed success $url/108 CI" > "$FAKE/runs.rerun"
printf '%s\n' "$starved" > "$FAKE/jobs-108"
: > "$FAKE/jobs-108.rerun"
public_ci ci/0.9.0 --platforms windows
expect_status "a job without a runner in a run still going is waited out, then started again" 0
expect "once the run has ended" 1 "$(wc -l < "$FAKE/rerun.log")"
rm -f "$VERIFIED"

printf '%s\n' "109 in_progress - $url/109 CI" > "$FAKE/runs"
printf '%s\n' "rust (windows-2025): failure ($url/109/job/3)" > "$FAKE/jobs-109"
public_ci ci/0.9.0 --platforms windows
expect_status "a job that failed is red at once, as before" 1
expect_true "and nothing is started again" '[[ ! -e "$FAKE/rerun.log" ]]'

printf '%s\n' "110 completed cancelled $url/110 CI" > "$FAKE/runs"
printf '%s\n' "$starved" > "$FAKE/jobs-110"
RB_PUBLIC_CI_RERUNS=0 public_ci ci/0.9.0 --platforms windows
expect_status "RB_PUBLIC_CI_RERUNS=0: red at once" 1
expect_true "nothing is started again" '[[ ! -e "$FAKE/rerun.log" ]]'
rm -f "$FAKE"/runs.* "$FAKE"/jobs-*

# The green belongs to the tree the export leaves: a commit that changes only what
# scripts/public-exclude.txt names keeps it, one that changes anything else does not.
mkdir -p "$TREE/docs"
printf '%s\n' '# what the public repository does not get' 'docs/' 'AGENTS.md  # the working card' \
    > "$TREE/scripts/public-exclude.txt"
echo "one" > "$TREE/docs/notes.md"
git_quiet -C "$TREE" add -A && git_quiet -C "$TREE" commit -qm "the exclude list"
EXPORTED="$(rb_public_tree "$TREE" HEAD)"
expect_true "the exported tree is the commit's without docs/" \
    '[[ "$EXPORTED" != "$(git -C "$TREE" rev-parse "HEAD^{tree}")" ]] && ! git -C "$TREE" ls-tree -r --name-only "$EXPORTED" | grep -q "^docs/"'
printf '%s\n' "111 completed success $url/111 CI" > "$FAKE/runs"
public_ci ci/0.9.0 --platforms windows
expect_true "its green is recorded for the exported tree" '[[ $status -eq 0 ]] && grep -q "^$EXPORTED windows-2025 " "$VERIFIED"'
echo "two" > "$TREE/docs/notes.md"
echo "the card" > "$TREE/AGENTS.md"
git_quiet -C "$TREE" add -A && git_quiet -C "$TREE" commit -qm "documents only"
public_ci ci/0.9.0 --platforms windows
expect_status "a commit that changes only what the export leaves out" 0
expect_output "keeps the green" "nothing was run"
expect_true "without an export" '[[ ! -e "$FAKE/export.log" ]]'
expect "and the index of the checkout is untouched" "" "$(git -C "$TREE" status --porcelain)"
echo "three" > "$TREE/README.md"
git_quiet -C "$TREE" add -A && git_quiet -C "$TREE" commit -qm "a public file"
public_ci ci/0.9.0 --platforms windows
expect_true "a commit that changes a public file is run again" '[[ $status -eq 0 && -e "$FAKE/export.log" ]]'

public_ci ci/0.9.0 --platforms beos
expect_status "an unknown platform is a wrong call" 2
public_ci integration/none --platforms windows
expect_status "an unknown branch is a wrong call" 2

finish_tests public-ci
