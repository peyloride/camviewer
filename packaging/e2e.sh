#!/usr/bin/env bash
#
# End to end: the real binary, a real RTSP stream, a real GPU render, checked.
#
#   ./packaging/e2e.sh
#
# Starts an RTSP server, publishes a colour-bar test pattern to it with ffmpeg,
# runs the app offscreen against that stream, and then looks at what the app
# drew:
#
#   1. the wall shows a picture, not the empty state or an error
#   2. the picture carries the test pattern's colours
#   3. the camera's audio is arriving and being read
#
# Nothing here touches the desktop or the user's own cameras. Needs ffmpeg,
# cage, grim, curl and a way to run tesseract-free checks.
#
# The RTSP server is mediamtx. Set MEDIAMTX to a binary you already have, or
# the script downloads one into packaging/cache.
#
# E2E_URL points the app somewhere else, which is how the checks themselves are
# tested: a wrong address must make this fail.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cache="$root/packaging/cache"
work="$(mktemp -d)"
version="1.15.4"
pattern="testsrc=size=640x360:rate=15"
stream_url="${E2E_URL:-rtsp://127.0.0.1:8554/test}"

mediamtx_pid=""
publisher_pid=""
cleanup() {
  [[ -n "$publisher_pid" ]] && kill "$publisher_pid" 2>/dev/null || true
  [[ -n "$mediamtx_pid" ]] && kill "$mediamtx_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

fail() {
  echo
  echo "FAIL: $*" >&2
  exit 1
}

for tool in ffmpeg cage grim; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is not installed"
done

# ---------------------------------------------------------------- RTSP server
mediamtx="${MEDIAMTX:-}"
if [[ -z "$mediamtx" ]]; then
  mkdir -p "$cache"
  mediamtx="$cache/mediamtx"
  if [[ ! -x "$mediamtx" ]]; then
    echo "fetching mediamtx $version"
    url="https://github.com/bluenviron/mediamtx/releases/download/v${version}/mediamtx_v${version}_linux_amd64.tar.gz"
    curl -fsSL "$url" -o "$work/mediamtx.tar.gz" || fail "could not download mediamtx"
    tar xzf "$work/mediamtx.tar.gz" -C "$cache" mediamtx || fail "could not unpack mediamtx"
    chmod +x "$mediamtx"
  fi
fi

echo "starting the RTSP server"
cat > "$work/mediamtx.yml" <<'YAML'
paths:
  all_others:
YAML
"$mediamtx" "$work/mediamtx.yml" >"$work/mediamtx.log" 2>&1 &
mediamtx_pid=$!
for _ in $(seq 1 20); do
  ss -ltn 2>/dev/null | grep -q ':8554' && break
  sleep 0.5
done
ss -ltn 2>/dev/null | grep -q ':8554' || fail "the RTSP server did not start"

echo "publishing a test pattern"
ffmpeg -hide_banner -loglevel error -re \
  -f lavfi -i "$pattern" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000" \
  -c:v libx264 -preset ultrafast -tune zerolatency -pix_fmt yuv420p -g 15 \
  -c:a aac -b:a 96k \
  -f rtsp rtsp://127.0.0.1:8554/test >"$work/publish.log" 2>&1 &
publisher_pid=$!
sleep 4

kill -0 "$publisher_pid" 2>/dev/null || fail "the publisher stopped: $(tail -2 "$work/publish.log")"

# ------------------------------------------------------------- run the app
echo "running the app against it"
cat > "$work/cameras.json" <<JSON
[
  {
    "name": "Test pattern",
    "url": "$stream_url"
  }
]
JSON

mkdir -p "$work/config/camviewer"
cp "$work/cameras.json" "$work/config/camviewer/cameras.json"

WLR_BACKENDS=headless XDG_CONFIG_HOME="$work/config" cage -- sh -c '
  "'"$root"'/target/release/camviewer" >"'"$work"'/app.log" 2>&1 &
  app=$!
  sleep 14

  port=$(pgrep -P $app -a ffmpeg | grep -oE "tcp://127.0.0.1:[0-9]+" | head -1 | grep -oE "[0-9]+$")
  if [ -n "$port" ]; then
    before=$(ss -tin state established "( sport = :$port )" 2>/dev/null | grep -oE "bytes_received:[0-9]+" | head -1 | cut -d: -f2)
    sleep 4
    after=$(ss -tin state established "( sport = :$port )" 2>/dev/null | grep -oE "bytes_received:[0-9]+" | head -1 | cut -d: -f2)
    echo "${before:-0} ${after:-0}" > "'"$work"'/audio.txt"
  else
    echo "none" > "'"$work"'/audio.txt"
  fi

  grim "'"$work"'/app.png"
  kill $app 2>/dev/null
' >/dev/null 2>&1 || fail "the offscreen run failed"

[[ -s "$work/app.png" ]] || fail "no screenshot was taken"

# ------------------------------------------------------------- look at it
python3 - "$work" <<'PY' || exit 1
import os, sys
from PIL import Image

work = sys.argv[1]
image = Image.open(os.path.join(work, "app.png")).convert("RGB")
width, height = image.size
problems = []

# 1. The wall must have drawn something. The empty state and the error state
#    are both nearly black, so a picture means colour.
tile = image.crop((0, 46, width, height - 40))
pixels = list(tile.get_flattened_data())
strong = [p for p in pixels if max(p) > 120 and max(p) - min(p) > 60]
share = len(strong) / max(len(pixels), 1)
print(f"  picture: {share:.1%} of the wall carries colour")
if share < 0.10:
    problems.append(
        f"the wall is {share:.1%} colour; it is showing no picture. "
        "Look at app.png."
    )

# 2. The test pattern is saturated bars, so several distinct colours must be
#    present. A single tint would mean something else was drawn.
buckets = {(p[0] // 64, p[1] // 64, p[2] // 64) for p in strong}
print(f"  colours: {len(buckets)} distinct groups")
if len(buckets) < 4:
    problems.append(f"only {len(buckets)} distinct colours; the test pattern is not there")

# 3. The camera's audio must be arriving and being read.
audio = open(os.path.join(work, "audio.txt")).read().split()
if audio[0] == "none":
    problems.append("no audio socket: the app did not open the camera's sound")
else:
    before, after = int(audio[0]), int(audio[1])
    moved = after - before
    print(f"  audio: {moved} bytes in 4s")
    if moved < 1_000_000:
        problems.append(f"only {moved} bytes of audio in 4s; the socket is not being read")

if problems:
    print()
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    sys.exit(1)

print()
print("PASS: the wall drew the test pattern and the camera's audio arrived")
PY
