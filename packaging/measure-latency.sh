#!/usr/bin/env bash
#
# How far behind live is the picture?
#
#   ./packaging/measure-latency.sh [samples]
#
# The cameras burn their own clock into the video, so that clock is ground
# truth. The script shows the app in a headless session, runs a reference
# ffmpeg that keeps the newest frame of the same camera in a file, and reads
# the clock out of both pictures with tesseract.
#
#   reference lag  how far the camera's clock runs ahead of the wall clock
#   app lag        the same for the app
#   extra          app lag - reference lag, the latency this app adds
#
# Needs tesseract, grim and cage. Runs offscreen; it does not touch the
# desktop.

set -euo pipefail

samples="${1:-3}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
config="${XDG_CONFIG_HOME:-$HOME/.config}/camviewer/cameras.json"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if ! command -v tesseract >/dev/null 2>&1; then
  echo "tesseract is not installed" >&2
  exit 1
fi

url="$(
  python3 - "$config" <<'PY'
import json, sys
cameras = json.load(open(sys.argv[1]))
first = cameras[0]
print(first.get("url_sd") or first["url"])
PY
)"

if [[ -z "$url" ]]; then
  echo "no camera in $config" >&2
  exit 1
fi

echo "camera: $url"
echo "taking $samples samples of each pipeline, one at a time: the camera"
echo "refuses a second session."
echo

cd "$root"
binary="$root/target/release/camviewer"

# Phase 1: the app, offscreen.
export WLR_BACKENDS=headless
cage -- sh -c '
  "'"$binary"'" >"'"$work"'/app.log" 2>&1 &
  app=$!
  sleep 16
  i=0
  while [ $i -lt '"$samples"' ]; do
    i=$((i + 1))
    printf "%s\n" "$(date +%s.%N)" > "'"$work"'/app-wall-$i.txt"
    grim "'"$work"'/app-$i.png"
    # Vary the phase: the camera clock only shows whole seconds.
    sleep 2.$((RANDOM % 9))$((RANDOM % 9))
  done
  kill $app 2>/dev/null
' >/dev/null 2>&1 || true
sleep 2
pkill -x camviewer 2>/dev/null || true
sleep 3

# Phase 2: a bare ffmpeg, no app, keeping the newest frame on disk.
# The camera may still hold the session the app just released, so retry.
start_reference() {
  rm -f "$work/reference.png"
  ffmpeg -hide_banner -loglevel error \
    -rtsp_transport tcp \
    -fflags nobuffer -flags low_delay -avioflags direct \
    -max_delay 0 -reorder_queue_size 0 -threads 1 \
    -i "$url" -an -f image2 -update 1 -y "$work/reference.png" \
    >>"$work/ffmpeg.log" 2>&1 &
  echo $!
}

reference=""
for attempt in 1 2 3 4; do
  sleep 4
  reference="$(start_reference)"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [[ -s "$work/reference.png" ]] && break
    sleep 1
  done
  if [[ -s "$work/reference.png" ]]; then
    break
  fi
  kill "$reference" 2>/dev/null || true
  reference=""
done

if [[ -z "$reference" ]]; then
  echo "the reference ffmpeg could not open the camera; log follows" >&2
  tail -3 "$work/ffmpeg.log" >&2 || true
  exit 1
fi

sleep 6
i=0
while [ $i -lt "$samples" ]; do
  i=$((i + 1))
  printf "%s\n" "$(date +%s.%N)" > "$work/ref-wall-$i.txt"
  cp "$work/reference.png" "$work/reference-$i.png" 2>/dev/null || true
  sleep 2.$((RANDOM % 9))$((RANDOM % 9))
done
kill $reference 2>/dev/null || true
wait 2>/dev/null || true

python3 - "$work" "$samples" <<'PY'
import datetime, re, subprocess, sys, os
from PIL import Image

work, samples = sys.argv[1], int(sys.argv[2])
pattern = re.compile(r"(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):\s?(\d{2}):\s?(\d{2})")


def read_clock(png, box):
    """OCR a crop and return the first date-time it contains."""
    if not os.path.exists(png):
        return None
    path = os.path.join(work, "crop.png")
    Image.open(png).crop(box).save(path)
    text = subprocess.run(
        ["tesseract", path, "-", "--psm", "6"],
        capture_output=True, text=True,
    ).stdout
    match = pattern.search(text)
    if not match:
        return None
    year, month, day, hour, minute, second = (int(g) for g in match.groups())
    return datetime.datetime(year, month, day, hour, minute, second)


# The clock sits at the top left of the first tile.
APP_BOX = (0, 48, 640, 200)
REFERENCE_BOX = (0, 0, 460, 90)

def sample(prefix, box, label):
    """(wall clock, camera clock) for each sample of one pipeline."""
    out = []
    for i in range(1, samples + 1):
        wall_path = f"{work}/{prefix}-wall-{i}.txt"
        if not os.path.exists(wall_path):
            continue
        wall = datetime.datetime.fromtimestamp(float(open(wall_path).read().strip()))
        camera = read_clock(f"{work}/{prefix}-{i}.png", box)
        if camera is None:
            print(f"{label} sample {i}: could not read the clock")
            continue
        out.append((wall, camera))
    return out


app_samples = sample("app", APP_BOX, "app")
ref_samples = sample("reference", REFERENCE_BOX, "reference")

if not ref_samples:
    log = os.path.join(work, "ffmpeg.log")
    if os.path.exists(log):
        text = open(log).read().strip()
        if text:
            print("the reference ffmpeg said:")
            print("  " + text[:400])

def spread(samples):
    """Each clock shows whole seconds, so a figure has a one second spread."""
    lags = [(wall - camera).total_seconds() for wall, camera in samples]
    if not lags:
        return None
    return min(lags), max(lags), sum(lags) / len(lags)


app_lag = spread(app_samples)
ref_lag = spread(ref_samples)

print()
if ref_lag:
    print(f"bare ffmpeg ：  {ref_lag[2]:.2f}s behind  ({ref_lag[0]:.2f} to {ref_lag[1]:.2f})")
if app_lag:
    print(f"this app     :  {app_lag[2]:.2f}s behind  ({app_lag[0]:.2f} to {app_lag[1]:.2f})")
if app_lag and ref_lag:
    extra = app_lag[2] - ref_lag[2]
    print()
    print(f"the app adds {extra:.2f}s over a bare ffmpeg pipeline")
    if extra < 0.6:
        print("that is within the clock's one second resolution: no real delay")
PY
