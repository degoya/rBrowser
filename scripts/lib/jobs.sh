# shellcheck shell=bash
#
# The parallelism of every script in scripts/, in one place.
#
# Usage, after ROOT is known:
#
#     source "$ROOT/scripts/lib/jobs.sh"
#
# Environment (a value that is already set wins):
#   RB_JOBS          cargo's build jobs (`-j`, CARGO_BUILD_JOBS). This bounds memory: the peak is
#                    in rustc and the link, one of each per job. Default 4.
#   RB_CLIPPY_JOBS   the build jobs of a clippy run over the whole workspace (`--workspace
#                    --all-targets`), which holds more rustc processes in memory at once than a
#                    build of one crate. Default 2, never more than RB_JOBS.
#   RB_TEST_THREADS  nextest's `--test-threads`. Default twice RB_JOBS, never more than the machine
#                    has cores: building is over by the time a test runs, and what a test thread
#                    costs is a core, not the memory RB_JOBS caps.
#
# Deliberately capped and not derived from nproc: letting cargo use every core of a WSL machine
# exhausts its memory and takes WSL down. The default moves only after a measurement.
#
# The values land in JOBS, CLIPPY_JOBS and TEST_THREADS. Not exported: a script that starts
# another passes them on explicitly, and a value the caller exported reaches every child anyway.

JOBS="${RB_JOBS:-4}"
CLIPPY_JOBS="${RB_CLIPPY_JOBS:-2}"
rb_jobs_cores="$(nproc 2> /dev/null || getconf _NPROCESSORS_ONLN 2> /dev/null || true)"
[[ "$rb_jobs_cores" =~ ^[1-9][0-9]*$ ]] || rb_jobs_cores=""

# A typo here would reach cargo as `-j` and fail late, or as 0 and mean something else.
for rb_jobs_name in JOBS CLIPPY_JOBS; do
    if [[ ! "${!rb_jobs_name}" =~ ^[1-9][0-9]*$ ]]; then
        echo "RB_$rb_jobs_name must be a positive whole number, not '${!rb_jobs_name}'" >&2
        exit 2
    fi
done
[[ "$CLIPPY_JOBS" -gt "$JOBS" ]] && CLIPPY_JOBS="$JOBS"

if [[ -n "${RB_TEST_THREADS:-}" ]]; then
    TEST_THREADS="$RB_TEST_THREADS"
    if [[ ! "$TEST_THREADS" =~ ^[1-9][0-9]*$ ]]; then
        echo "RB_TEST_THREADS must be a positive whole number, not '$TEST_THREADS'" >&2
        exit 2
    fi
else
    TEST_THREADS=$((JOBS * 2))
    if [[ -n "$rb_jobs_cores" && "$rb_jobs_cores" -lt "$TEST_THREADS" ]]; then
        TEST_THREADS="$rb_jobs_cores"
    fi
fi
unset rb_jobs_cores rb_jobs_name
