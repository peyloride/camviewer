#!/usr/bin/env bash
#
# End to end: the real binary, a real RTSP stream, a real GPU render, checked.
#
#   ./packaging/e2e.sh
#
# Starts an RTSP server (mediamtx) and runs the app offscreen against streams
# published to it with ffmpeg, then looks at what the app drew. Five checks:
#
#   1. the wall draws the picture, with the camera's sound arriving
#   2. --version prints the build without opening a window
#   3. a stream with no sound still draws
#   4. the picture comes back after the stream drops and returns
#   5. a camera that cannot be reached is reported, not silently blank
#
# Nothing here touches the desktop or the user's own cameras. Needs ffmpeg,
# cage, grim, curl and python3 with Pillow.
#
# Set MEDIAMTX to an existing binary, or the script downloads one into
# packaging/cache. Set E2E_URL to point check 5 at something else.
#
# The checks are themselves checked. With nothing published there is no
# picture, so this must fail:
#
#   E2E_BREAK=1 ./packaging/e2e.sh    # must exit non-zero

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cache="$root/packaging/cache"
binary="$root/target/release/camviewer"
work="$(mktemp -d)"
version="1.15.4"
pattern="testsrc=size=640x360:rate=15"
tone="sine=frequency=440:sample_rate=48000"

mediamtx_pid=""
publisher_pids=()
cleanup() {
  for pid in "${publisher_pids[@]:-}"; do kill "$pid" 2>/dev/null || true; done
  [[ -n "$mediamtx_pid" ]] && kill "$mediamtx_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

fail() {
  echo
  echo "FAIL: $*" >&2
  exit 1
}

for tool in ffmpeg cage grim curl python3; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is not installed"
done
[[ -x "$binary" ]] || fail "build it first: cargo build --release"

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

# Only RTSP is needed here: this publishes a stream and reads it back. Every
# other listener the server would open is a port this test does not use and a
# way for an unrelated program on the machine to stop the test from starting.
cat > "$work/mediamtx.yml" <<'YAML'
rtsp: yes
rtmp: no
hls: no
webrtc: no
srt: no
api: no
metrics: no
playback: no
paths:
  all_others:
YAML
"$mediamtx" "$work/mediamtx.yml" >"$work/mediamtx.log" 2>&1 &
mediamtx_pid=$!
for _ in $(seq 1 20); do
  ss -ltn 2>/dev/null | grep -q ':8554' && break
  sleep 0.5
done
# The log says why, and without it the only symptom is a missing port.
ss -ltn 2>/dev/null | grep -q ':8554' || {
  tail -5 "$work/mediamtx.log" >&2
  fail "the RTSP server did not start"
}

# ------------------------------------------------------------- the publisher
# publish <path> <with-sound>
publish() {
  local path="$1" sound="$2"
  # E2E_BREAK leaves the stream unpublished. That is how the checks themselves
  # are checked: with nothing being published, this script must fail.
  [[ "${E2E_BREAK:-}" == "1" ]] && return 0
  local audio=()
  [[ "$sound" == "yes" ]] && audio=(-f lavfi -i "$tone")

  ffmpeg -hide_banner -loglevel error -re \
    -f lavfi -i "$pattern" "${audio[@]}" \
    -c:v libx264 -preset ultrafast -tune zerolatency -pix_fmt yuv420p -g 15 \
    -c:a aac -b:a 96k \
    -f rtsp "rtsp://127.0.0.1:8554/$path" >"$work/publish-$path.log" 2>&1 &
  publisher_pids+=($!)
}

# ------------------------------------------------------------ running the app
# run_app <directory> <extra shell inside the session>
run_app() {
  local dir="$1" body="$2"
  mkdir -p "$dir/config/camviewer"
  cp "$dir/cameras.json" "$dir/config/camviewer/cameras.json"

  # Bounded: a background process inside the session can keep the compositor
  # alive after the script has finished with it.
  timeout 90 env WLR_BACKENDS=headless XDG_CONFIG_HOME="$dir/config" cage -- sh -c '
    "'"$binary"'" >"'"$dir"'/app.log" 2>&1 &
    app=$!
    '"$body"'
    kill $app 2>/dev/null
    wait 2>/dev/null
  ' >/dev/null 2>&1 || true
}

# Looks at a screenshot and writes what it found to <dir>/verdict.
# look <png> <dir> <expect: picture|error>
look() {
  local png="$1" dir="$2" expect="$3"
  python3 - "$png" "$dir" "$expect" <<'PY'
import sys
from PIL import Image

png, out_dir, expect = sys.argv[1], sys.argv[2], sys.argv[3]
image = Image.open(png).convert("RGB")
width, height = image.size

# Bytes rather than pixels: Pillow's accessors differ between the version on
# this machine and the one on the CI runner.
raw = image.crop((0, 46, width, height - 40)).tobytes()
total = len(raw) // 3
strong = 0
buckets = set()
fault = 0
for i in range(0, total * 3, 3):
    red, green, blue = raw[i], raw[i + 1], raw[i + 2]
    high, low = max(red, green, blue), min(red, green, blue)
    if high > 120 and high - low > 60:
        strong += 1
        buckets.add((red // 64, green // 64, blue // 64))
    # The sheet spends its one colour on a fault and on nothing else, so a unit
    # it cannot reach wears the drafting blue. Counting the picture alone would
    # not tell a reported failure from a drawing that never happened.
    if abs(red - 29) < 40 and abs(green - 78) < 40 and abs(blue - 216) < 40:
        fault += 1

share = strong / max(total, 1)
if expect == "picture":
    ok = share >= 0.10 and len(buckets) >= 4
    verdict = f"{share:.1%} colour, {len(buckets)} groups"
else:
    # No picture is drawn, and the sheet says why.
    ok = share < 0.10 and fault >= 500
    verdict = f"{share:.1%} colour, {fault} fault pixels"

open(f"{out_dir}/verdict", "w").write(f"{'ok' if ok else 'bad'}|{verdict}")
PY
}

# compare <png a> <png b> <dir> <expect: same|different>
#
# A frozen picture and a live one look the same in one screenshot. Two shots a
# couple of seconds apart tell them apart, because the test pattern moves.
compare() {
  local a="$1" b="$2" dir="$3" expect="$4"
  mkdir -p "$dir"
  python3 - "$a" "$b" "$dir" "$expect" <<'PY'
import sys
from PIL import Image

a, b, out_dir, expect = sys.argv[1:5]


def wall(path):
    image = Image.open(path).convert("RGB")
    width, height = image.size
    return image.crop((0, 46, width, height - 40)).tobytes()


first, second = wall(a), wall(b)
changed = sum(1 for x, y in zip(first, second) if abs(x - y) > 12)
share = changed / max(len(first), 1)

if expect == "different":
    ok = share >= 0.01
    verdict = f"{share:.2%} of the picture moved"
else:
    ok = share < 0.01
    verdict = f"{share:.2%} moved, so it is standing still"

open(f"{out_dir}/verdict", "w").write(f"{'ok' if ok else 'bad'}|{verdict}")
PY
}

# report <directory holding a verdict> <what was being checked>
report() {
  local dir="$1" label="$2" verdict
  verdict="$(cat "$work/$dir/verdict")"
  printf '  %-34s %s\n' "$label" "${verdict#*|}"
  [[ "${verdict%%|*}" == "ok" ]] || fail "$label: $(tail -2 "$work/$dir/app.log")"
}

# ------------------------------------------------------------------ check 1
echo "check 1: the wall draws the picture, and the camera's sound arrives"
publish test yes
# Check 4 stops this exact process, so remember it now: the last publisher by
# then will be a different stream.
if [[ ${#publisher_pids[@]} -gt 0 ]]; then
  echo "${publisher_pids[-1]}" > "$work/test-publisher.pid"
else
  : > "$work/test-publisher.pid"
fi
sleep 4

mkdir -p "$work/one"
cat > "$work/one/cameras.json" <<'JSON'
[{ "name": "Test pattern", "url": "rtsp://127.0.0.1:8554/test" }]
JSON

run_app "$work/one" '
  sleep 14
  port=$(pgrep -P $app -a ffmpeg | grep -oE "tcp://127.0.0.1:[0-9]+" | head -1 | grep -oE "[0-9]+$")
  if [ -n "$port" ]; then
    before=$(ss -tin state established "( sport = :$port )" 2>/dev/null | grep -oE "bytes_received:[0-9]+" | head -1 | cut -d: -f2)
    sleep 4
    after=$(ss -tin state established "( sport = :$port )" 2>/dev/null | grep -oE "bytes_received:[0-9]+" | head -1 | cut -d: -f2)
    echo "${before:-0} ${after:-0}" > "'"$work"'/one/audio.txt"
  else
    echo "none" > "'"$work"'/one/audio.txt"
  fi
  grim "'"$work"'/one/app.png"
'

[[ -s "$work/one/app.png" ]] || fail "no screenshot was taken"
look "$work/one/app.png" "$work/one" picture
report one "the picture is drawn"

audio="$(cat "$work/one/audio.txt")"
if [[ "$audio" == "none" ]]; then
  fail "no audio socket: the app did not open the camera's sound"
fi
read -r before after <<< "$audio"
moved=$((after - before))
[[ "$moved" -ge 1000000 ]] || fail "only $moved bytes of audio in 4s; the socket is not read"
printf '  %-34s %s\n' "the camera's sound is read" "$moved bytes in 4s"

# ------------------------------------------------------------------ check 2
echo "check 2: --version prints the build and opens no window"
printed="$(timeout 10 "$binary" --version)"
status=$?
# Exit 124 would mean it never returned, which is what opening a window does.
[[ $status -eq 0 ]] || fail "--version did not exit on its own (status $status); it opened something"
expected="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
[[ "$printed" == "camviewer $expected" ]] || fail "--version printed '$printed', wanted 'camviewer $expected'"
printf '  %-34s %s\n' "--version, without a window" "$printed"

# ------------------------------------------------------------------ check 3
echo "check 3: a stream with no sound still draws"
publish silent no
sleep 4

mkdir -p "$work/silent"
cat > "$work/silent/cameras.json" <<'JSON'
[{ "name": "No sound", "url": "rtsp://127.0.0.1:8554/silent" }]
JSON
run_app "$work/silent" 'sleep 16; grim "'"$work"'/silent/app.png"'

[[ -s "$work/silent/app.png" ]] || fail "no screenshot was taken"
look "$work/silent/app.png" "$work/silent" picture
report silent "a stream with no sound draws"

# ------------------------------------------------------------------ check 4
echo "check 4: the picture comes back after the stream drops"
mkdir -p "$work/again"
cat > "$work/again/cameras.json" <<'JSON'
[{ "name": "Test pattern", "url": "rtsp://127.0.0.1:8554/test" }]
JSON

run_app "$work/again" '
  sleep 14
  grim "'"$work"'/again/before-a.png"
  sleep 3
  grim "'"$work"'/again/before-b.png"

  # Drop the stream and let the app notice. By pid: pkill -f would match this
  # very shell, whose command line carries the address.
  pid="$(cat "'"$work"'/test-publisher.pid")"
  [ -n "$pid" ] && kill "$pid" 2>/dev/null
  sleep 8
  grim "'"$work"'/again/dropped-a.png"
  sleep 3
  grim "'"$work"'/again/dropped-b.png"

  # Publish again. The app has to find it without being restarted.
  ffmpeg -hide_banner -loglevel error -re \
    -f lavfi -i "'"$pattern"'" \
    -c:v libx264 -preset ultrafast -tune zerolatency -pix_fmt yuv420p -g 15 \
    -f rtsp rtsp://127.0.0.1:8554/test >/dev/null 2>&1 &
  sleep 18
  grim "'"$work"'/again/recovered-a.png"
  sleep 3
  grim "'"$work"'/again/recovered-b.png"
'

for shot in before-a before-b dropped-a dropped-b recovered-a recovered-b; do
  [[ -s "$work/again/$shot.png" ]] || fail "no $shot screenshot"
done

# The test pattern moves, so a live picture differs between two shots and a
# frozen one does not. That is what tells recovery apart from a stale frame.
compare "$work/again/before-a.png" "$work/again/before-b.png" "$work/c-before" different
compare "$work/again/dropped-a.png" "$work/again/dropped-b.png" "$work/c-dropped" same
compare "$work/again/recovered-a.png" "$work/again/recovered-b.png" "$work/c-recovered" different

printf '  %-34s %s\n' "before the drop" "$(cut -d'|' -f2 < "$work/c-before/verdict")"
printf '  %-34s %s\n' "while the stream is gone" "$(cut -d'|' -f2 < "$work/c-dropped/verdict")"
printf '  %-34s %s\n' "after it returns" "$(cut -d'|' -f2 < "$work/c-recovered/verdict")"
for d in c-before c-dropped c-recovered; do
  [[ "$(cut -d'|' -f1 < "$work/$d/verdict")" == "ok" ]] || fail "$d: $(cut -d'|' -f2 < "$work/$d/verdict")"
done

# ------------------------------------------------------------------ check 5
echo "check 5: a camera that cannot be reached is reported"
unreachable="${E2E_URL:-rtsp://127.0.0.1:8554/does-not-exist}"
mkdir -p "$work/missing"
cat > "$work/missing/cameras.json" <<JSON
[{ "name": "Missing", "url": "$unreachable" }]
JSON
run_app "$work/missing" 'sleep 16; grim "'"$work"'/missing/app.png"'

[[ -s "$work/missing/app.png" ]] || fail "no screenshot was taken"
look "$work/missing/app.png" "$work/missing" error
report missing "a camera that cannot be reached is reported"

echo
echo "PASS: all five checks"
