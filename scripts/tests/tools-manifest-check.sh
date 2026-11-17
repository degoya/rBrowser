#!/usr/bin/env bash
#
# scripts/tools-manifest-check.sh (RB-080-02) against a stub `curl` on PATH: every address of the
# manifest is asked, a dead address or a download of another size fails the check and is named,
# a live manifest passes, `--list` reads the built-in manifest without the network.
#
#   scripts/tests/tools-manifest-check.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

bin="$SCRATCH/bin"
mkdir -p "$bin"
calls="$SCRATCH/calls"
# The stub answers per address from $STUB_ANSWERS (`url code length` per line): it writes the
# headers where --dump-header points and prints the status code, as curl's --write-out does.
cat > "$bin/curl" <<EOF
#!/usr/bin/env bash
dump=""; url=""
while [[ \$# -gt 0 ]]; do
    case "\$1" in
        --dump-header) dump="\$2"; shift 2 ;;
        --output|--max-time|--retry|--write-out) shift 2 ;;
        -*) shift ;;
        *) url="\$1"; shift ;;
    esac
done
echo "\$url" >> "$calls"
read -r _ code length < <(grep -F "\$url " "\$STUB_ANSWERS" || echo "x 000 0")
printf 'HTTP/2 %s\r\ncontent-length: %s\r\n\r\n' "\$code" "\$length" > "\$dump"
printf '%s' "\$code"
EOF
chmod +x "$bin/curl"

manifest="$SCRATCH/tools.json"
cat > "$manifest" <<'EOF'
{ "version": 1, "tools": [ {
  "name": "ffmpeg", "platform": "x86_64-pc-windows-msvc", "version": "n9",
  "url": "https://mirror.test/ffmpeg.zip", "sha256": "ab", "size": 1234, "archive": "zip",
  "executable": "ffmpeg.exe", "members": ["ffmpeg.exe"], "license": "LGPL-3.0-or-later",
  "source_url": "https://mirror.test/ffmpeg-source.tar.gz" } ] }
EOF
answers="$SCRATCH/answers"

printf '%s\n' "https://mirror.test/ffmpeg.zip 200 1234" "https://mirror.test/ffmpeg-source.tar.gz 200 99" > "$answers"
run_status env PATH="$bin:$PATH" STUB_ANSWERS="$answers" bash "$ROOT/scripts/tools-manifest-check.sh" "$manifest"
expect_status "a live manifest passes" 0
expect "the download and the source are asked" "2" "$(wc -l < "$calls" | tr -d ' ')"

printf '%s\n' "https://mirror.test/ffmpeg.zip 404 0" "https://mirror.test/ffmpeg-source.tar.gz 200 99" > "$answers"
run_status env PATH="$bin:$PATH" STUB_ANSWERS="$answers" bash "$ROOT/scripts/tools-manifest-check.sh" "$manifest"
expect_status "a dead download fails" 1
expect_output "the dead address is named" "DEAD https://mirror.test/ffmpeg.zip: HTTP 404"

printf '%s\n' "https://mirror.test/ffmpeg.zip 200 1234" "https://mirror.test/ffmpeg-source.tar.gz 404 0" > "$answers"
run_status env PATH="$bin:$PATH" STUB_ANSWERS="$answers" bash "$ROOT/scripts/tools-manifest-check.sh" "$manifest"
expect_status "a dead source offer fails" 1
expect_output "the source address is named" "DEAD https://mirror.test/ffmpeg-source.tar.gz"

printf '%s\n' "https://mirror.test/ffmpeg.zip 200 999" "https://mirror.test/ffmpeg-source.tar.gz 200 99" > "$answers"
run_status env PATH="$bin:$PATH" STUB_ANSWERS="$answers" bash "$ROOT/scripts/tools-manifest-check.sh" "$manifest"
expect_status "a download of another size fails" 1
expect_output "the size is named" "SIZE https://mirror.test/ffmpeg.zip: 999, the manifest says 1234"

: > "$calls"
run_status env PATH="$bin:$PATH" bash "$ROOT/scripts/tools-manifest-check.sh" --list
expect_status "the built-in manifest lists" 0
expect_output "with the pinned ffmpeg and its size" "ffmpeg-n9.0.2-39-g7adfbe40dc-win64-lgpl-9.0.zip	168715328"
expect "--list asks no address" "0" "$(wc -l < "$calls" | tr -d ' ')"

run_status bash "$ROOT/scripts/tools-manifest-check.sh" "$SCRATCH/missing.json"
expect_status "a missing manifest is a usage error" 2

finish_tests tools-manifest-check
