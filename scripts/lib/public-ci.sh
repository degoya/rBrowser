# shellcheck shell=bash
#
# The public CI run of scripts/public-ci.sh (RB-090-12, copied and adapted from rCommander's
# scripts/lib/public-ci.sh, RC-100-31, RC-110-32, itself from rDownloader's): export a tree to the
# public repository as a ci/ branch, let GitHub's runners check it on the systems this machine
# cannot, wait, and delete the branch on green. A red run keeps its branch, so the failure can be
# read next to its tree (rb_public_ci_failed_log).
#
# Sourced, never run. Every function returns non-zero on a failure instead of exiting, so a
# caller without errexit (the release pipeline's step) reads the status itself.
#
# A green run is recorded per runner image and tree in rb-verified-ci of the repository's common
# git directory - one file for every worktree - and a tree green on an image is not run there
# again. A red run records nothing.
#
# Environment:
#   RB_PUBLIC_DIR         the local clone of the public repository (~/projects/rBrowser-public)
#   RB_PUBLIC_REPO        the GitHub repository (degoya/rBrowser)
#   RB_PUBLIC_CI_TIMEOUT  seconds to wait for a run to appear at all (900)
#   RB_PUBLIC_CI_CEILING  seconds after which a run still going is given up (21600, the six hours
#                         a hosted job may run; every job of ci.yml has its own timeout)
#   RB_PUBLIC_CI_POLL     seconds between two looks (60)
#   RB_PUBLIC_CI_RERUNS   how often a run whose only red jobs never got a runner is started again
#                         (1; 0: such a run is red at once)
#   RB_PUBLIC_CI_CLOCK    tests only: a file whose number is the clock both limits are read on, in
#                         place of bash's SECONDS; the tests' stand-in gh moves it

PUBLIC_DIR="${RB_PUBLIC_DIR:-$HOME/projects/rBrowser-public}"
PUBLIC_REPO="${RB_PUBLIC_REPO:-degoya/rBrowser}"
PUBLIC_CI_TIMEOUT="${RB_PUBLIC_CI_TIMEOUT:-900}"
PUBLIC_CI_CEILING="${RB_PUBLIC_CI_CEILING:-21600}"
PUBLIC_CI_POLL="${RB_PUBLIC_CI_POLL:-60}"
PUBLIC_CI_RERUNS="${RB_PUBLIC_CI_RERUNS:-1}"
# Every system ci.yml checks; read by the script that sources this file. macOS joins with
# RB-090-15's job in ci.yml.
# shellcheck disable=SC2034
RB_PUBLIC_CI_ALL="linux,windows"
# One run as a line, "<id> <status> <conclusion or -> <url> <name>", from `gh run list`'s JSON.
RB_PUBLIC_CI_RUN_JQ='.[] | "\(.databaseId) \(.status) \(if (.conclusion // "") == "" then "-" else .conclusion end) \(.url) \(.name)"'

# gh usable for github.com: 0 yes; 2 no gh, or gh's own answer that nobody is signed in or the
# token is invalid; 1 when `gh auth status` failed three times otherwise - a timeout or the
# network, seen in a freshly detached session - which is "not finished", not a missing login.
rb_public_ci_gh_ready() {
    command -v gh > /dev/null || { echo "gh is required to watch the public CI" >&2; return 2; }
    local attempt out=""
    for attempt in 1 2 3; do
        [[ $attempt -eq 1 ]] || sleep "${RB_PUBLIC_CI_AUTH_PAUSE:-10}"
        out="$(timeout "${RB_PUBLIC_CI_AUTH_TIMEOUT:-30}" gh auth status --hostname github.com 2>&1)" \
            && return 0
        if grep -qiE 'not logged in|token .* is invalid' <<< "$out"; then
            echo "gh is not signed in to github.com (gh auth login)" >&2
            return 2
        fi
    done
    echo "gh auth status failed three times, not for want of a login (timeout or network?):" >&2
    printf '%s\n' "${out:-(no output; timed out after ${RB_PUBLIC_CI_AUTH_TIMEOUT:-30} s)}" >&2
    return 1
}

# The runner images, one per line, of a comma-separated list of systems (linux, windows) or
# their runner images: ci.yml's jobs run on ubuntu-24.04 (Linux, and the Windows lint through
# cargo-xwin) and windows-2025.
rb_public_ci_images() {
    local item
    local -a items
    IFS=',' read -r -a items <<< "$1"
    [[ ${#items[@]} -gt 0 ]] || { echo "no platform named" >&2; return 2; }
    for item in "${items[@]}"; do
        case "$item" in
            linux | ubuntu-24.04) printf '%s\n' ubuntu-24.04 ;;
            windows | windows-2025) printf '%s\n' windows-2025 ;;
            *) echo "unknown platform: $item (linux, windows, ubuntu-24.04 or windows-2025)" >&2; return 2 ;;
        esac
    done
}

# The value of ci.yml's `platforms` input (a choice: all, linux, windows) that covers the runner
# images $@.
rb_public_ci_input() {
    local linux=0 windows=0 image
    for image in "$@"; do
        case "$image" in
            ubuntu-24.04) linux=1 ;;
            windows-2025) windows=1 ;;
        esac
    done
    if [[ $linux -eq 1 && $windows -eq 1 ]]; then echo all
    elif [[ $windows -eq 1 ]]; then echo windows
    else echo linux
    fi
}

# The tree GitHub gets of commit $2 of checkout $1: the commit's tree without the paths its own
# scripts/public-exclude.txt names (the working tree's list for a commit from before it), as
# scripts/export-public.sh leaves them out. The greens are recorded for this tree ,
# not for the private one: a commit that changes only what is left out - a job file, a document -
# changes nothing a public runner sees and keeps the green. Without a list it is the commit's tree.
rb_public_tree() {
    local root="$1" commit="$2" scratch index list line path tree
    scratch="$(mktemp -d)" || return 1
    index="$scratch/index"
    if ! list="$(git -C "$root" show "$commit:scripts/public-exclude.txt" 2> /dev/null)"; then
        list="$(cat "$root/scripts/public-exclude.txt" 2> /dev/null || true)"
    fi
    if ! GIT_INDEX_FILE="$index" git -C "$root" read-tree "$commit"; then
        rm -rf "$scratch"
        return 1
    fi
    while IFS= read -r line || [[ -n "$line" ]]; do
        path="${line%%#*}"
        path="${path#"${path%%[![:space:]]*}"}"
        path="${path%"${path##*[![:space:]]}"}"
        path="${path%/}"
        [[ -n "$path" ]] || continue
        GIT_INDEX_FILE="$index" git -C "$root" rm -r -q --cached --ignore-unmatch -- ":(literal,top)$path" \
            > /dev/null 2>&1 || true
    done <<< "$list"
    tree="$(GIT_INDEX_FILE="$index" git -C "$root" write-tree)" || tree=""
    rm -rf "$scratch"
    [[ -n "$tree" ]] && printf '%s\n' "$tree"
}

# The file the greens are recorded in, for checkout $1.
rb_verified_ci_file() {
    printf '%s/rb-verified-ci\n' "$(git -C "$1" rev-parse --path-format=absolute --git-common-dir)"
}

# Whether runner image $3 has a recorded green for the exported tree $2 (rb_public_tree) of
# checkout $1.
rb_ci_green() {
    local file
    file="$(rb_verified_ci_file "$1")"
    [[ -f "$file" ]] && awk -v tree="$2" -v image="$3" '$1 == tree && $2 == image { found = 1 } END { exit !found }' "$file"
}

# Records a green of tree $2 of checkout $1 on the runner images $3...: "<tree> <image> <time>".
rb_record_ci() {
    local root="$1" tree="$2" file image when
    shift 2
    file="$(rb_verified_ci_file "$root")" || return 1
    when="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    for image in "$@"; do printf '%s %s %s\n' "$tree" "$image" "$when" >> "$file"; done
}

# The marker of the run this machine started on public branch $1, in the public clone's git
# directory: "<public commit> <tree> <image>...". A second public-ci.sh of the same tree - a
# release step beside a run started ahead of it - waits for that run instead of exporting again:
# the re-push would make GitHub's concurrency group cancel the run (rCommander 0.10.0).
rb_public_ci_marker() {
    printf '%s/rb-ci-running/%s\n' "$(git -C "$PUBLIC_DIR" rev-parse --absolute-git-dir)" "${1//\//-}"
}

# Marks public branch $1 at commit $2 as running tree $3 on the runner images $4....
rb_public_ci_mark() {
    local file
    file="$(rb_public_ci_marker "$1")" || return 1
    mkdir -p "${file%/*}" && printf '%s\n' "${*:2}" > "$file"
}

# The public commit of a run of tree $2 on public branch $1 covering the runner images $3..., when
# this machine marked one, the branch still stands on it and none of its finished runs is red;
# nothing and non-zero otherwise.
rb_public_ci_running() {
    local branch="$1" tree="$2" file sha marked images image head listed
    shift 2
    file="$(rb_public_ci_marker "$branch" 2> /dev/null)" && [[ -f "$file" ]] || return 1
    read -r sha marked images < "$file"
    [[ -n "$sha" && "$marked" == "$tree" ]] || return 1
    for image in "$@"; do [[ " $images " == *" $image "* ]] || return 1; done
    head="$(git -C "$PUBLIC_DIR" ls-remote origin "refs/heads/$branch" 2> /dev/null)" || return 1
    [[ "${head%%[[:space:]]*}" == "$sha" ]] || return 1
    listed="$(gh run list --repo "$PUBLIC_REPO" --branch "$branch" --commit "$sha" \
        --event workflow_dispatch --json databaseId,status,conclusion,url,name \
        --jq "$RB_PUBLIC_CI_RUN_JQ" 2> /dev/null < /dev/null)" || return 1
    ! awk '$2 == "completed" && $3 !~ /^(success|skipped|neutral)$/ { found = 1 } END { exit !found }' <<< "$listed" \
        || return 1
    printf '%s\n' "$sha"
}

# Starts ci.yml on public branch $1 with the `platforms` input $2 (all, linux or windows).
rb_public_ci_dispatch() {
    echo "starting ci.yml on $1 for $2"
    gh workflow run ci.yml --repo "$PUBLIC_REPO" --ref "$1" -f platforms="$2"
}

# The clock the limits are read on: bash's SECONDS, or the number in RB_PUBLIC_CI_CLOCK.
rb_public_ci_now() {
    if [[ -n "${RB_PUBLIC_CI_CLOCK:-}" ]]; then cat "$RB_PUBLIC_CI_CLOCK"; else echo "$SECONDS"; fi
}

# The red jobs of run $1 (its latest attempt) in one line, or nothing: "<name>: <conclusion>
# (<url>)" for the first job that failed, timed out or was cancelled while it ran - a job red long
# before its run ends ends the wait at once - or "no-runner: <name>, <name>" when every red job
# was cancelled without a single step: GitHub gave it no runner and dropped it from the queue
# (seen by rCommander on macOS after 15 minutes in the queue).
# shellcheck disable=SC2016  # $red is jq's
rb_public_ci_red_jobs() {
    gh run view "$1" --repo "$PUBLIC_REPO" --json jobs --jq '
        def starved: .conclusion == "cancelled" and ((.steps // []) | length) == 0;
        [.jobs[] | select(.conclusion == "failure" or .conclusion == "timed_out" or .conclusion == "cancelled")] as $red
        | if ($red | length) == 0 then empty
          elif ($red | all(starved)) then "no-runner: " + ([$red[].name] | join(", "))
          else ($red | map(select(starved | not))[0] | "\(.name): \(.conclusion) (\(.url))")
          end' 2> /dev/null < /dev/null | head -n 1 || true
}

# Whether run $1 may be started again: fewer than RB_PUBLIC_CI_RERUNS of the ids $2... are its own.
rb_public_ci_rerun_left() {
    local id="$1" done=0 other
    shift
    for other in "$@"; do [[ "$other" != "$id" ]] || done=$((done + 1)); done
    ((done < PUBLIC_CI_RERUNS))
}

# Waits for the dispatched runs on public branch $1 at commit $2 to finish. Returns 0 when every
# one succeeded, 1 at once when a job or a run failed - its run's id then in RB_PUBLIC_CI_RED_RUN
# for rb_public_ci_failed_log - when no run appeared within RB_PUBLIC_CI_TIMEOUT, or when one was still
# going at RB_PUBLIC_CI_CEILING. A run whose only red jobs never got a runner is no failure yet:
# once it has ended, its failed jobs are started again (`gh run rerun --failed`,
# RB_PUBLIC_CI_RERUNS times), and only the same again is red. Prints what changed.
rb_public_ci_wait() {
    local branch="$1" sha="$2" now deadline ceiling listed last="" seen=0 red line
    local id status conclusion url jobs open settling grace=""
    local -a reruns=()
    RB_PUBLIC_CI_RED_RUN=""
    echo "waiting for the CI of $PUBLIC_REPO on $branch at ${sha:0:12} (for as long as it runs)"
    now="$(rb_public_ci_now)"
    deadline=$((now + PUBLIC_CI_TIMEOUT))
    ceiling=$((now + PUBLIC_CI_CEILING))
    while :; do
        listed="$(gh run list --repo "$PUBLIC_REPO" --branch "$branch" --commit "$sha" \
            --event workflow_dispatch --json databaseId,status,conclusion,url,name \
            --jq "$RB_PUBLIC_CI_RUN_JQ" 2> /dev/null < /dev/null)" || listed=""
        if [[ -n "$listed" ]]; then
            seen=1
            if [[ "$listed" != "$last" ]]; then
                while IFS= read -r line; do echo "  $(date +%H:%M:%S) run $line"; done <<< "$listed"
                last="$listed"
            fi
            # The runs started again at the last look: the list may not say so yet, once.
            red="" open=0 settling="$grace" grace=""
            while [[ -z "$red" ]] && read -r id status conclusion url _; do
                if [[ "$status" != completed ]]; then
                    open=1
                    jobs="$(rb_public_ci_red_jobs "$id")"
                    # Jobs without a runner wait for their run to end: only then can it run again.
                    if [[ "$jobs" != no-runner:* ]]; then
                        red="$jobs"
                    elif ! rb_public_ci_rerun_left "$id" "${reruns[@]}"; then
                        red="run $id: ${jobs#no-runner: } never got a runner ($url)"
                    fi
                elif [[ ! "$conclusion" =~ ^(success|skipped|neutral)$ ]]; then
                    if [[ " $settling " == *" $id "* ]]; then
                        open=1
                        continue
                    fi
                    red="run $id: $conclusion ($url)"
                    jobs="$(rb_public_ci_red_jobs "$id")"
                    if [[ "$jobs" == no-runner:* ]]; then
                        red+="; ${jobs#no-runner: } never got a runner"
                        if rb_public_ci_rerun_left "$id" "${reruns[@]}" \
                            && gh run rerun "$id" --repo "$PUBLIC_REPO" --failed < /dev/null; then
                            echo "  $(date +%H:%M:%S) $red; started again"
                            reruns+=("$id")
                            grace+=" $id"
                            red="" open=1 last=""
                        fi
                    fi
                fi
                # shellcheck disable=SC2034  # read by scripts/public-ci.sh
                [[ -z "$red" ]] || RB_PUBLIC_CI_RED_RUN="$id"
            done <<< "$listed"
            if [[ -n "$red" ]]; then
                echo "the public CI is red: $red; $branch is kept for inspection" >&2
                return 1
            fi
            if [[ "$open" -eq 0 ]]; then
                echo "the public CI is green on ${sha:0:12}"
                return 0
            fi
        fi
        now="$(rb_public_ci_now)"
        if [[ "$seen" -eq 0 ]] && ((now >= deadline)); then
            echo "no CI run appeared within ${PUBLIC_CI_TIMEOUT}s; $branch is kept" >&2
            # A run that never started is nothing the next call should wait for.
            rm -f "$(rb_public_ci_marker "$branch" 2> /dev/null)"
            return 1
        fi
        if ((now >= ceiling)); then
            echo "the public CI did not finish within ${PUBLIC_CI_CEILING}s; $branch is kept" >&2
            return 1
        fi
        sleep "$PUBLIC_CI_POLL"
    done
}

# What broke in run $1: the last lines of its failed steps' logs (`gh run view --log-failed`).
rb_public_ci_failed_log() {
    local log
    log="$(gh run view "$1" --repo "$PUBLIC_REPO" --log-failed 2> /dev/null < /dev/null)" || return 1
    printf '%s\n' "$log" | tail -n "${RB_PUBLIC_CI_LOG_LINES:-80}"
}

# Deletes branch $1 from the public repository and from the local clone, and its run's marker. A
# branch already gone - deleted by the other call that waited for the same run - is no failure.
rb_public_ci_delete() {
    local status=0
    rm -f "$(rb_public_ci_marker "$1" 2> /dev/null)"
    git -C "$PUBLIC_DIR" ls-remote --exit-code --heads origin "refs/heads/$1" > /dev/null 2>&1 || status=$?
    case "$status" in
        0) git -C "$PUBLIC_DIR" push --quiet origin --delete "$1" || return 1 ;;
        2) ;;
        *) return 1 ;;
    esac
    # Before the first release the clone has no main to return to and still stands on the
    # branch; its local ref then stays until the next export.
    git -C "$PUBLIC_DIR" branch -D "$1" > /dev/null 2>&1 || true
    echo "$1 deleted"
}
