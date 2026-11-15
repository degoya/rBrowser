#!/usr/bin/env bash
#
# Every download the managed-tools manifest names still answers (RB-080-02, after rDownloader's
# RD-1101-13): the manifest is compiled into the release, so an address that dies after it - BtbN
# deletes its daily builds after two weeks - breaks the ffmpeg download for every user until the
# next release. Each distinct `url` is asked with a HEAD request (redirects followed, nothing
# downloaded) and its final Content-Length compared with the manifest's `size`; each
# `source_url` (the LGPL's source offer) must answer 200. The hash is not checked here - that
# needs the bytes, and the app checks it on every download.
#
# Usage:
#   scripts/tools-manifest-check.sh                  # the built-in manifest
#   scripts/tools-manifest-check.sh <tools.json>     # another manifest
#   scripts/tools-manifest-check.sh --list [file]    # print `url<TAB>size`, no network
#
# Exit 0 when every address answers as the manifest says, 1 when one does not, 2 on a usage or
# manifest error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

list_only=0
if [[ "${1:-}" == "--list" ]]; then
    list_only=1
    shift
fi
manifest="${1:-$ROOT/crates/rbrowser/src/tools/tools.json}"
[[ -f "$manifest" ]] || { echo "no manifest at $manifest" >&2; exit 2; }

# One line per distinct address: `url<TAB>size`, a source address with the size `-`.
entries="$(python3 - "$manifest" <<'EOF'
import json, sys
document = json.load(open(sys.argv[1], encoding="utf-8"))
seen = {}
for tool in document["tools"]:
    if seen.setdefault(tool["url"], str(tool["size"])) != str(tool["size"]):
        sys.exit(f"{tool['url']} is named with two different sizes")
    seen.setdefault(tool["source_url"], "-")
for url, size in seen.items():
    print(f"{url}\t{size}")
EOF
)" || { echo "the manifest at $manifest cannot be read" >&2; exit 2; }
[[ -n "$entries" ]] || { echo "the manifest at $manifest names no download" >&2; exit 2; }

if [[ "$list_only" -eq 1 ]]; then
    printf '%s\n' "$entries"
    exit 0
fi

headers="$(mktemp)"
trap 'rm -f "$headers"' EXIT

checked=0
dead=0
while IFS=$'\t' read -r url size; do
    checked=$((checked + 1))
    code="$(curl --silent --head --location --retry 2 --max-time 60 \
        --output /dev/null --dump-header "$headers" --write-out '%{http_code}' "$url" 2>/dev/null)" || true
    if [[ "$code" != "200" ]]; then
        [[ "$code" =~ ^[1-9] ]] && code="HTTP $code" || code="no answer"
        echo "DEAD $url: $code"
        dead=$((dead + 1))
        continue
    fi
    if [[ "$size" != "-" ]]; then
        # The last response's length: a redirect's own `content-length: 0` comes first.
        length="$(tr -d '\r' < "$headers" | awk 'tolower($1) == "content-length:" { n = $2 } END { print n }')"
        if [[ "$length" != "$size" ]]; then
            echo "SIZE $url: ${length:-no Content-Length}, the manifest says $size"
            dead=$((dead + 1))
            continue
        fi
    fi
    echo "ok   $url"
done <<< "$entries"

echo
if [[ "$dead" -gt 0 ]]; then
    echo "$dead of $checked address(es) in $manifest do not answer as the manifest says"
    exit 1
fi
echo "$checked address(es) in $manifest answer as the manifest says"
