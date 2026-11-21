#!/usr/bin/env bash
# The Safari spike of RB-095-03, on a Mac: what safaridriver allows, measured, as one block to
# paste into docs/roadmap/jobs/095-03-safari-view-macos.md (section "Spike").
#
#   scripts/spike-safari.sh [--technology-preview] [--port <n>]
#
# Run it twice: once before `safaridriver --enable` (or with Develop > Allow Remote Automation
# off) - it records the exact refusal text - and once after. It answers:
#   1. several WebDriver sessions at once (one per Safari view)?
#   2. does Safari lock a person's input in an automated window? (asks you to click and type)
#   3. do pointer, wheel and key actions (POST /actions) reach the page as trusted events?
#   4. window rect: points or pixels, the narrowest window, the chrome above the viewport
#   5. does an element screenshot of <html> cover the whole page?
#   6. does POST /window bring the window to the front?
# Needs macOS, Safari, curl and python3. Starts nothing but safaridriver and a local web server
# on 127.0.0.1, and ends both.
set -euo pipefail

driver=/usr/bin/safaridriver
browser=safari
port=4446
while [ $# -gt 0 ]; do
  case "$1" in
    --technology-preview)
      driver="/Applications/Safari Technology Preview.app/Contents/MacOS/safaridriver"
      browser="Safari Technology Preview"
      shift
      ;;
    --port) port="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] || { echo "the Safari spike runs on macOS only" >&2; exit 1; }
[ -x "$driver" ] || { echo "no safaridriver at $driver" >&2; exit 1; }

work=$(mktemp -d)
base="http://127.0.0.1:$port"
pids=()
finish() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  rm -rf "$work"
}
trap finish EXIT

# The fixture: logs clicks, keys, wheel and scroll as trusted or synthetic, and its viewport.
cat > "$work/index.html" <<'HTML'
<!doctype html><meta charset="utf-8"><title>rBrowser Safari spike</title>
<style>body{margin:0;height:4000px;font:16px system-ui}#go{position:absolute;left:20px;top:20px;width:200px;height:60px}</style>
<button id="go">Click me</button><input id="field" style="position:absolute;left:20px;top:120px">
<script>
window.__log = [];
for (const type of ['click', 'keydown', 'wheel', 'scroll', 'pointerdown']) {
  addEventListener(type, (e) => __log.push({ type, trusted: e.isTrusted, y: scrollY }), true);
}
</script>
HTML
(cd "$work" && python3 -u -m http.server 0 --bind 127.0.0.1 > "$work/http.log" 2>&1) &
pids+=($!)
"$driver" --port "$port" > "$work/driver.log" 2>&1 &
pids+=($!)

# pick <a.b.c>: the value at that path of the JSON on stdin (text as it is, the rest as JSON).
pick() {
  python3 -c '
import json, sys
value = json.load(sys.stdin)
for key in sys.argv[1].split("."):
    value = value.get(key, "") if isinstance(value, dict) else ""
print(value if isinstance(value, str) else json.dumps(value))
' "$1"
}
call() {
  local args=(-s -X "$1" -H 'Content-Type: application/json')
  if [ $# -ge 3 ]; then args+=(-d "$3"); fi
  curl "${args[@]}" "$base$2"
}

for _ in $(seq 1 100); do
  call GET /status 2>/dev/null | grep -q '"ready":true' && break
  sleep 0.1
done
for _ in $(seq 1 50); do
  web=$(grep -o 'port [0-9]*' "$work/http.log" | head -1 | cut -d' ' -f2 || true)
  [ -n "$web" ] && break
  sleep 0.1
done
page="http://127.0.0.1:$web/index.html"

echo "### Spike result ($(date -u +%Y-%m-%d), $(sw_vers -productName) $(sw_vers -productVersion), $(uname -m))"
echo "- safaridriver: $("$driver" --version 2>&1 | head -1)"
caps="{\"capabilities\":{\"alwaysMatch\":{\"browserName\":\"$browser\"}}}"
first=$(call POST /session "$caps")
sid=$(printf '%s' "$first" | pick value.sessionId)
if [ -z "$sid" ]; then
  echo "- new session refused: $(printf '%s' "$first" | pick value.error): $(printf '%s' "$first" | pick value.message)"
  echo "  (the text rb_safari::protocol must recognise as remote automation off)"
  exit 0
fi
echo "- Safari $(printf '%s' "$first" | pick value.capabilities.browserVersion)"
second=$(call POST /session "$caps")
sid2=$(printf '%s' "$second" | pick value.sessionId)
if [ -n "$sid2" ]; then
  echo "- 1. several sessions: yes (a second session opened)"
  call DELETE "/session/$sid2" > /dev/null
else
  echo "- 1. several sessions: no - $(printf '%s' "$second" | pick value.message)"
fi

s="/session/$sid"
exec_js() { call POST "$s/execute/sync" "{\"script\":\"$1\",\"args\":[]}" | pick value; }
call POST "$s/url" "{\"url\":\"$page\"}" > /dev/null
call POST "$s/window/rect" '{"x":40,"y":60,"width":320,"height":700}' > /dev/null
echo "- 4. window rect after asking 40,60 320x700: $(call GET "$s/window/rect" | pick value)"
echo "     viewport (inner) and chrome: $(exec_js 'return [innerWidth, innerHeight, outerWidth-innerWidth, outerHeight-innerHeight, devicePixelRatio];')"
call POST "$s/window/rect" '{"width":1024,"height":800}' > /dev/null

actions='{"actions":[{"type":"pointer","id":"mouse","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":120,"y":50,"origin":"viewport"},{"type":"pointerDown","button":0},{"type":"pointerUp","button":0}]},{"type":"key","id":"keyboard","actions":[{"type":"pause"},{"type":"pause"},{"type":"pause"},{"type":"keyDown","value":"a"},{"type":"keyUp","value":"a"}]}]}'
click=$(call POST "$s/actions" "$actions")
wheel=$(call POST "$s/actions" '{"actions":[{"type":"wheel","id":"wheel","actions":[{"type":"scroll","x":100,"y":300,"deltaX":0,"deltaY":400,"origin":"viewport"}]}]}')
echo "- 3. actions answered: pointer+key $(printf '%s' "$click" | pick value), wheel $(printf '%s' "$wheel" | pick value)"
echo "     page log: $(exec_js 'return JSON.stringify(__log);')"

echo
echo ">>> 2. Click into the Safari window and type a few keys now, then press Enter here." >&2
read -r _
echo "- 2. after the person's click and keys, page log: $(exec_js 'return JSON.stringify(__log.slice(-6));')"

html=$(call POST "$s/element" '{"using":"css selector","value":"html"}' | pick value.element-6066-11e4-a52e-4f735466cecf)
call GET "$s/element/$html/screenshot" | pick value | base64 --decode > "$work/page.png"
echo "- 5. element screenshot of html: $(python3 -c "import struct,sys; b=open(sys.argv[1],'rb').read(24); print('%dx%d px' % struct.unpack('>II', b[16:24]))" "$work/page.png"), page height $(exec_js 'return document.documentElement.scrollHeight;') CSS px"

handle=$(call GET "$s/window" | pick value)
echo ">>> 6. Bring another app to the front, then press Enter here." >&2
read -r _
call POST "$s/window" "{\"handle\":\"$handle\"}" > /dev/null
echo ">>> Is the Safari window in front now? (y/n)" >&2
read -r front
echo "- 6. POST /window brought the window to the front: $front"
call DELETE "$s" > /dev/null
