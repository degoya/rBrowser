#!/usr/bin/env bash
#
# The gates of the release's channel jobs (RB-090-13, RB-110-04) against a local origin with tags:
# scripts/release-channels.sh names the newest plain vX.Y.Z tag (v1.10.0 after v1.9.0, a
# pre-release never), moves a channel only for that tag and with its secret, and `push` of a
# channel the release rendered nothing for (the Homebrew tap before RB-097-05/02) ends with a
# notice; scripts/release-repositories.sh gates the apt and dnf repository on the tag and both of
# its secrets. Nothing here reaches GitHub.
#
#   scripts/tests/release-channels.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

# An origin with release tags, and a checkout of it whose scripts/ are this tree's.
git init --quiet --bare "$SCRATCH/origin.git"
work="$SCRATCH/work"
git init --quiet "$work"
git -C "$work" -c user.name=t -c user.email=t@example.invalid commit --quiet --allow-empty -m init
for tag in v1.2.0 v1.9.0 v1.10.0 v1.11.0-beta.1 nightly; do git -C "$work" tag "$tag"; done
git -C "$work" remote add origin "$SCRATCH/origin.git"
git -C "$work" push --quiet origin --tags
ln -s "$ROOT/scripts" "$work/scripts"
cd "$work"

# Runs `env <NAME=value...> <command>` with GITHUB_OUTPUT in a fresh file; $output gets the run's
# output and the file's lines after it.
gate() {
    : > "$SCRATCH/output"
    run_status env GITHUB_OUTPUT="$SCRATCH/output" "$@"
    output+=$'\n'"$(cat "$SCRATCH/output")"
}

run_status scripts/release-channels.sh newest
expect "the newest plain tag, by version and not by text" "v1.10.0" "$output"

gate REF_NAME=v1.10.0 SCOOP_BUCKET_DEPLOY_KEY=key scripts/release-channels.sh gate bucket
expect_status "the gate runs" 0
expect_output "the newest tag with its secret moves the bucket" "move=true"
gate REF_NAME=v1.10.0 SCOOP_BUCKET_DEPLOY_KEY="" scripts/release-channels.sh gate bucket
expect_output "without its secret the bucket stays" "move=false"
expect_output "and says which secret" "SCOOP_BUCKET_DEPLOY_KEY is not set"
gate REF_NAME=v1.9.0 WINGET_TOKEN=token scripts/release-channels.sh gate winget
expect_output "an older tag moves nothing back" "move=false"
expect_output "naming the newest" "not the newest release tag (v1.10.0)"
gate REF_NAME=v1.11.0-beta.1 HOMEBREW_TAP_DEPLOY_KEY=key scripts/release-channels.sh gate tap
expect_output "a pre-release moves nothing" "move=false"
gate scripts/release-channels.sh gate flatpak
expect_status "an unknown channel is a usage error" 2
run_status scripts/release-channels.sh
expect_status "no command is a usage error" 2

run_status env REF_NAME=v1.10.0 CHECKOUT=tap RENDERED="$SCRATCH/rendered/homebrew" scripts/release-channels.sh push
expect_status "a channel rendered nothing for is no failure" 0
expect_output "but a notice" "renders nothing for tap; it stays on its last release"

# The apt and dnf repository needs both of its secrets.
gate REF_NAME=v1.10.0 PACKAGES_DEPLOY_KEY=key PACKAGES_GPG_KEY=gpg scripts/release-repositories.sh gate
expect_output "the newest tag with both secrets publishes" "publish=true"
gate REF_NAME=v1.10.0 PACKAGES_DEPLOY_KEY=key PACKAGES_GPG_KEY="" scripts/release-repositories.sh gate
expect_output "without the signing key nothing is published" "publish=false"
expect_output "and says which secret" "PACKAGES_GPG_KEY is not set"
gate REF_NAME=v1.2.0 PACKAGES_DEPLOY_KEY=key PACKAGES_GPG_KEY=gpg scripts/release-repositories.sh gate
expect_output "an older tag publishes nothing" "publish=false"
# Signing refuses without the public half of the key in the tree.
if [[ ! -f "$ROOT/packaging/repository/rbrowser-packages.asc" ]]; then
    run_status env RUNNER_TEMP="$SCRATCH" PACKAGES_GPG_KEY=gpg OWNER=degoya \
        GITHUB_REPOSITORY=degoya/rBrowser scripts/release-repositories.sh sign
    expect_status "no key in the tree signs nothing" 1
    expect_output "and says what is missing" "packaging/repository/rbrowser-packages.asc is missing"
else
    echo "skip signing without the key: packaging/repository/rbrowser-packages.asc is committed"
fi

finish_tests "release-channels.sh"
