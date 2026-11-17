#!/usr/bin/env bash
#
# The security advisories against the dependencies as they are today (RB-090-04, run weekly by
# .github/workflows/advisories.yml and by hand at any time): `cargo deny check advisories` for the
# Rust crates, and `npm audit --omit=dev` in every npm package with a package-lock.json (ui/,
# agent/, sidecar/ and the plugins' workspace plugins/) - the shipped dependencies, not the build
# tools. Every check runs even after one failed; the run fails when any did. Licences, bans and
# sources change only with the tree and are checked by `scripts/check.sh --preflight`.
#
#   scripts/advisories.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failed=()
echo "==> cargo deny check advisories"
cargo deny check advisories || failed+=(cargo)

shopt -s nullglob
for dir in ui agent sidecar plugins; do
    [[ -f "$dir/package.json" ]] || continue
    if [[ ! -f "$dir/package-lock.json" ]]; then
        echo "==> $dir: no package-lock.json, skipped"
        continue
    fi
    echo "==> npm audit --omit=dev in $dir"
    (cd "$dir" && npm audit --omit=dev) || failed+=("$dir")
done

if [[ ${#failed[@]} -gt 0 ]]; then
    echo "!! advisories found in: ${failed[*]}" >&2
    exit 1
fi
echo "    no advisories"
