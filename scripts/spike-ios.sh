#!/usr/bin/env bash
# The iOS spike of RB-095-04, on a Mac: what safaridriver does with the iOS Simulator and a
# connected iPhone or iPad, measured, as one block to paste into
# docs/roadmap/jobs/095-04-ios-simulator-und-geraet.md (section "Spike").
#
#   scripts/spike-ios.sh [--simulator "<model>"] [--second "<model>"] [--device <udid>]
#                        [--port <n>] [--shots <n>]
#
# Run it on a Mac with Xcode (the simulator part), and once on a Mac without Xcode with an
# iPhone connected over USB (`--device`, the UDID from the inventory it prints first). It answers:
#   1. the inventory: Xcode, the iOS simulators, the devices on USB and their UDIDs (the serial
#      number system_profiler reports, the dash rb-safari puts in)
#   2. simulator: boot time, a session on safari:deviceUDID, its user agent and viewport, a second
#      simulator at once
#   3. touch and wheel actions: what reaches the page (click, touchstart, isTrusted)
#   4. device: a session without Xcode, the refusal text while Remote Automation is off
#   5. live image: `xcrun simctl io <udid> screenshot` and WebDriver's screenshot, time per shot
# Needs macOS, curl and python3; Xcode for the simulator part. Starts safaridriver and a local
# web server on 127.0.0.1 (both ended), boots the simulators named (left running).
set -euo pipefail

model="iPhone 16"
second=""
device=""
port=4447
shots=5
while [ $# -gt 0 ]; do
  case "$1" in
    --simulator) model="$2"; shift 2 ;;
    --second) second="$2"; shift 2 ;;
    --device) device="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    --shots) shots="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] || { echo "the iOS spike runs on macOS only" >&2; exit 1; }

work=$(mktemp -d)
base="http://127.0.0.1:$port"
pids=()
finish() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  rm -rf "$work"
}
trap finish EXIT

now() { python3 -c 'import time; print(time.monotonic())'; }
since() { python3 -c 'import sys, time; print("%.1f s" % (time.monotonic() - float(sys.argv[1])))' "$1"; }
# pick <a.b.c>: the value at that path of the JSON on stdin (text as it is, the rest as JSON).
pick() {
  python3 -c '
import json, sys
try:
    value = json.load(sys.stdin)
except ValueError:
    value = ""
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

# The fixture: logs taps, clicks, keys, wheel and scroll as trusted or synthetic.
cat > "$work/index.html" <<'HTML'
<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>rBrowser iOS spike</title>
<style>body{margin:0;height:4000px;font:16px system-ui}#go{position:absolute;left:20px;top:20px;width:200px;height:60px}</style>
<button id="go">Tap me</button>
<script>
window.__log = [];
for (const type of ['click', 'touchstart', 'pointerdown', 'keydown', 'wheel', 'scroll']) {
  addEventListener(type, (e) => __log.push({ type, trusted: e.isTrusted, pointer: e.pointerType }), true);
}
</script>
HTML
(cd "$work" && python3 -u -m http.server 0 --bind 127.0.0.1 > "$work/http.log" 2>&1) &
pids+=($!)
/usr/bin/safaridriver --port "$port" > "$work/driver.log" 2>&1 &
pids+=($!)
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
echo "- safaridriver: $(/usr/bin/safaridriver --version 2>&1 | head -1)"

# 1. The inventory.
if xcrun --find simctl > /dev/null 2>&1; then xcode=yes; else xcode=no; fi
echo "- 1. Xcode (xcrun --find simctl): $xcode ($(xcode-select -p 2>/dev/null || echo 'no developer dir'))"
if [ "$xcode" = yes ]; then
  xcrun simctl list devices available --json > "$work/simctl.json"
  echo "     iOS simulators: $(python3 -c '
import json, sys
runtimes = json.load(open(sys.argv[1]))["devices"]
sims = [(r.rsplit(".", 1)[-1], d["name"]) for r, ds in runtimes.items() if ".iOS-" in r for d in ds]
print(len(sims), "-", ", ".join(sorted({n for _, n in sims})[:12]))
' "$work/simctl.json")"
fi
system_profiler SPUSBDataType SPUSBHostDataType -json > "$work/usb.json" 2> /dev/null || true
echo "     devices on USB (name, serial -> UDID): $(python3 -c '
import json, sys
def walk(v, out):
    if isinstance(v, dict):
        name = str(v.get("_name", ""))
        serial = next((str(x) for k, x in v.items() if "serial" in k.lower()), "")
        if name.startswith(("iPhone", "iPad", "iPod")) and serial:
            udid = serial[:8] + "-" + serial[8:] if len(serial) == 24 else serial
            out.append("%s %s...%s" % (name, udid[:4], udid[-4:]))
        for x in v.values(): walk(x, out)
    elif isinstance(v, list):
        for x in v: walk(x, out)
out = []
walk(json.load(open(sys.argv[1])), out)
print(", ".join(out) or "none")
' "$work/usb.json")"
if command -v xcrun > /dev/null && xcrun devicectl list devices > "$work/devicectl.txt" 2>&1; then
  echo "     devicectl (Xcode 15+) lists: $(grep -c -E 'iPhone|iPad' "$work/devicectl.txt" || true) device(s) - compare the UDIDs above"
fi

# session <capabilities JSON>: the session id, or the refusal on stderr.
session() {
  local answer
  answer=$(call POST /session "{\"capabilities\":{\"alwaysMatch\":$1}}")
  local sid
  sid=$(printf '%s' "$answer" | pick value.sessionId)
  if [ -z "$sid" ]; then
    echo "refused: $(printf '%s' "$answer" | pick value.error): $(printf '%s' "$answer" | pick value.message)" >&2
  fi
  printf '%s' "$sid"
}
exec_js() { call POST "/session/$1/execute/sync" "{\"script\":\"$2\",\"args\":[]}" | pick value; }

# probe <session> <label>: load the fixture, print the user agent, viewport, taps and timings.
probe() {
  local s="/session/$1"
  call POST "$s/url" "{\"url\":\"$page\"}" > /dev/null
  echo "     $2 user agent: $(exec_js "$1" 'return navigator.userAgent;')"
  echo "     $2 viewport, dpr: $(exec_js "$1" 'return [innerWidth, innerHeight, devicePixelRatio];')"
  local tap mouse wheel
  tap=$(call POST "$s/actions" '{"actions":[{"type":"pointer","id":"finger","parameters":{"pointerType":"touch"},"actions":[{"type":"pointerMove","x":120,"y":50,"origin":"viewport"},{"type":"pointerDown","button":0},{"type":"pointerUp","button":0}]}]}')
  mouse=$(call POST "$s/actions" '{"actions":[{"type":"pointer","id":"mouse","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":120,"y":50,"origin":"viewport"},{"type":"pointerDown","button":0},{"type":"pointerUp","button":0}]}]}')
  wheel=$(call POST "$s/actions" '{"actions":[{"type":"wheel","id":"wheel","actions":[{"type":"scroll","x":100,"y":300,"deltaX":0,"deltaY":400,"origin":"viewport"}]}]}')
  echo "- 3. $2 actions: touch $(printf '%s' "$tap" | pick value.error), mouse $(printf '%s' "$mouse" | pick value.error), wheel $(printf '%s' "$wheel" | pick value.error) (empty: accepted)"
  echo "     $2 page log: $(exec_js "$1" 'return JSON.stringify(__log);')"
  local start
  start=$(now)
  for _ in $(seq 1 "$shots"); do call GET "$s/screenshot" > /dev/null; done
  echo "- 5. $2 WebDriver screenshot: $shots shots in $(since "$start")"
}

# 2. The simulator.
if [ "$xcode" = yes ]; then
  udid=$(python3 -c '
import json, sys
runtimes = json.load(open(sys.argv[1]))["devices"]
found = [(r, d["udid"]) for r, ds in runtimes.items() if ".iOS-" in r for d in ds if d["name"] == sys.argv[2]]
print(sorted(found)[-1][1] if found else "")
' "$work/simctl.json" "$model")
  if [ -z "$udid" ]; then
    echo "- 2. no simulator named $model"
  else
    start=$(now)
    xcrun simctl boot "$udid" 2> /dev/null || true
    open -a Simulator --args -CurrentDeviceUDID "$udid"
    xcrun simctl bootstatus "$udid" > /dev/null 2>&1 || true
    echo "- 2. $model ${udid:0:4}...${udid: -4}: booted in $(since "$start")"
    start=$(now)
    sid=$(session "{\"browserName\":\"safari\",\"platformName\":\"iOS\",\"safari:useSimulator\":true,\"safari:deviceUDID\":\"$udid\"}" 2> "$work/refused.txt" || true)
    if [ -z "$sid" ]; then
      echo "     session $(cat "$work/refused.txt")"
    else
      echo "     session in $(since "$start")"
      probe "$sid" simulator
      start=$(now)
      for _ in $(seq 1 "$shots"); do xcrun simctl io "$udid" screenshot "$work/shot.png" > /dev/null 2>&1; done
      echo "     simctl io screenshot: $shots shots in $(since "$start")"
      if [ -n "$second" ]; then
        udid2=$(python3 -c '
import json, sys
runtimes = json.load(open(sys.argv[1]))["devices"]
found = [(r, d["udid"]) for r, ds in runtimes.items() if ".iOS-" in r for d in ds if d["name"] == sys.argv[2]]
print(sorted(found)[-1][1] if found else "")
' "$work/simctl.json" "$second")
        xcrun simctl boot "$udid2" 2> /dev/null || true
        xcrun simctl bootstatus "$udid2" > /dev/null 2>&1 || true
        sid2=$(session "{\"browserName\":\"safari\",\"platformName\":\"iOS\",\"safari:useSimulator\":true,\"safari:deviceUDID\":\"$udid2\"}" 2> "$work/refused2.txt" || true)
        if [ -n "$sid2" ]; then
          echo "     a second simulator ($second) at once: yes"
          call DELETE "/session/$sid2" > /dev/null
        else
          echo "     a second simulator ($second) at once: no - $(cat "$work/refused2.txt")"
        fi
      fi
      call DELETE "/session/$sid" > /dev/null
    fi
  fi
fi

# 4. The device.
if [ -n "$device" ]; then
  start=$(now)
  sid=$(session "{\"browserName\":\"safari\",\"platformName\":\"iOS\",\"safari:useSimulator\":false,\"safari:deviceUDID\":\"$device\"}" 2> "$work/refused.txt" || true)
  if [ -z "$sid" ]; then
    echo "- 4. device ${device:0:4}...${device: -4} (Xcode: $xcode): session $(cat "$work/refused.txt")"
    echo "     (turn on Settings > Safari > Advanced > Remote Automation, run again; the text above is what rb_safari::protocol must read)"
  else
    echo "- 4. device ${device:0:4}...${device: -4} (Xcode: $xcode): session in $(since "$start")"
    probe "$sid" device
    call DELETE "/session/$sid" > /dev/null
  fi
fi
