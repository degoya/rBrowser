#!/usr/bin/env bash
#
# Writes plugins/schema-org/data/vocabulary.json, the schema.org vocabulary the structured data
# panel validates against (RB-090-96), from one pinned schema.org release: its
# schemaorg-current-https.jsonld, downloaded from the release's tag and held to the SHA-256 below,
# then compacted by scripts/lib/schema_org_vocabulary.py (classes with supertypes, data types,
# enumeration members, properties with domains, ranges and successor). The vocabulary is CC BY-SA
# 3.0 (plugins/schema-org/NOTICE).
#
# A new release: set VERSION, URL and SHA256 together, run the script, review the diff, and
# name the version in plugins/schema-org/NOTICE.
#
# --check writes nothing and exits 1 when the checked-in file is not what the pinned release
# gives; it downloads the release, so it is not part of check.sh --preflight (its test,
# scripts/tests/schema-org-vocabulary.sh, is). --from <file> reads the release from a file
# instead of the network; it must have the pinned SHA-256 too. Exit 2: a usage error, a download
# that failed or a release that is not the pinned one.
#
# Usage:
#   scripts/schema-org-vocabulary.sh [--check] [--from <release.jsonld>]
set -euo pipefail

VERSION="30.1"
URL="https://raw.githubusercontent.com/schemaorg/schemaorg/v30.1/data/releases/30.1/schemaorg-current-https.jsonld"
SHA256="0a4b8c4c910fc831ec8695196510973390906a5ee34dc6b862dfec4d21a12f74"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/plugins/schema-org/data/vocabulary.json"

usage() {
    echo "usage: scripts/schema-org-vocabulary.sh [--check] [--from <release.jsonld>]" >&2
    exit 2
}

check=()
from=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) check=(--check) ;;
        --from)
            [[ $# -ge 2 ]] || usage
            from="$2"
            shift
            ;;
        *) usage ;;
    esac
    shift
done

if [[ -z "$from" ]]; then
    scratch="$(mktemp -d)"
    trap 'rm -rf "$scratch"' EXIT
    from="$scratch/schemaorg-current-https.jsonld"
    if ! curl --fail --silent --show-error --location --max-time 120 --proto '=https' \
        --output "$from" "$URL"; then
        echo "schema-org-vocabulary: the download of $URL failed" >&2
        exit 2
    fi
fi
[[ -f "$from" ]] || { echo "schema-org-vocabulary: no file $from" >&2; exit 2; }

exec python3 -I "$ROOT/scripts/lib/schema_org_vocabulary.py" "$from" "$OUT" \
    --version "$VERSION" --url "$URL" --sha256 "$SHA256" "${check[@]}"
