# CamViewer — agent notes

A GPUI desktop app that shows RTSP cameras in a wall. Rust, one binary, Linux,
macOS and Windows. Read `README.md` for what it does; this file is what you need
to work on it without breaking things.

## The user's desktop is off limits

The user works on this PC while you work. Never open the app on their session,
and never send `xdotool`, `ydotool` or `wtype` input to it. Synthetic clicks
land in their windows and steal their focus. Everything below is testable
without touching their session.

If you need the app on screen, ask first. It is also usually unnecessary: this
app's behaviour is decided by pure functions that have tests.

## Testing offscreen

`cage` starts a headless Wayland session on the real GPU. `grim` captures it.

```bash
cd ~/projects/private/camviewer-gpui
WLR_BACKENDS=headless cage -- sh -c '
  ./target/release/camviewer >/tmp/camviewer.log 2>&1 &
  APP=$!
  sleep 14
  grim /tmp/camviewer.png
  kill $APP 2>/dev/null
'
```

The app and `grim` inherit `WAYLAND_DISPLAY` from `cage`. Look at the result
with the `read` tool.

Give it 14 seconds. The streams need to connect and the first keyframe.

### Do not disturb the real configuration

The app reads `$XDG_CONFIG_HOME/camviewer/cameras.json`, which for the user is
`~/.config/camviewer/cameras.json`. Their real cameras live there. Point a test
run somewhere else instead of editing it:

```bash
XDG_CONFIG_HOME=/tmp/testcfg cage -- sh -c '...'
```

Count the app's decoder processes to check the feed lifecycle:

```bash
pgrep -P $APP -a ffmpeg
```

### The cameras are a shared resource

Each camera allows only a few RTSP sessions. The user's Frigate already holds
several, so a test that opens its own connection is often refused, and the
refusal shows up as `406 Not Acceptable` or `Operation not permitted`.

- Prefer a restream over connecting to a camera. The user's Frigate restreams
  the same cameras on port 8554 and serves many clients.
- Measure one pipeline at a time. Two at once will fight.
- Camera addresses and credentials are in the local configuration. Never commit
  them, and never put them in a test.

## Checks

```bash
cargo fmt
cargo clippy --all-targets
cargo test
```

`cargo test` covers layout, feed choice and configuration round trips. It opens
no window and needs no camera. Prefer extending those tests over manual runs.

## Releases

Notes live in `CHANGELOG.md`. The release workflow publishes the section that
matches the tag, and a tag with no section fails the build. CI also fails when
the version in `Cargo.toml` has no section, so a version bump and its notes must
land together.

1. Write the notes under `## [x.y.z]` in `CHANGELOG.md`. Describe what changed
   for the person using the app, not which files moved. Separate fixes from
   additions. This text becomes the release body *and* the popup the app shows
   once after updating, so write it for a human.
2. Set the same version in `Cargo.toml`.
3. Run the checks above.
4. `./packaging/release-notes.sh vx.y.z` prints exactly what the release will
   say. Read it before tagging.
5. Commit, then `git tag -a vx.y.z -m "CamViewer x.y.z"` and
   `git push origin vx.y.z`.
6. Confirm with `gh release view vx.y.z` that all four assets and the notes are
   there. Budget 20 minutes: the macOS runners queue.

Do not use GitHub's generated notes. They list commits, which is not what the
person updating the app needs to read.

### Runners

CI runs on `ubuntu-24.04` on purpose. The repository is public, so a pull
request from a stranger would otherwise run its code on a private machine.
The release workflow starts only from a pushed tag, which is safe, and reads
`vars.LINUX_RUNNER` if you want it on another machine:

```bash
gh variable set LINUX_RUNNER --body 'self-hosted'
```

The user's own runners are registered per repository, so one registered to
another repository cannot serve this one.

### Assets

The four asset names come from the Rust target triple. `update::asset_name()`
builds the same name at run time. Change one, change the other, or every client
stops finding its update.

The builds are not code signed. macOS needs `xattr -d com.apple.quarantine`,
Windows shows a SmartScreen warning.

## Updating

`src/update.rs` asks GitHub for the latest release, and on approval downloads
the asset, replaces the running executable with `self-replace`, and restarts.

- The app must live somewhere the user can write. `packaging/install.sh`
  installs to `~/.local/bin` for that reason. A system directory breaks
  updating for a normal user.
- The notes are written to the config directory *before* the restart and shown
  once by the new build, then deleted. See `config::store_whats_new`.
- `--version` prints and exits before GPUI starts, so scripts can ask without
  opening a window.

## The bits that are easy to get wrong

Each of these cost real time to find. They are not style preferences.

### A GPUI picture needs a definite size *and* an aspect ratio

GPUI gives an image element an `aspect-ratio` from the picture when the element
has none. That ratio fights a percentage height: the element grows taller than
its parent, and the parent clips it. The picture looks cropped, not
letterboxed. `video()` in `src/app.rs` states both. Keep it that way.

### Frames must be released after they are replaced

The GPU reads an image while it is drawn, so `CamViewer::poll` collects
replaced frames and calls `cx.drop_image` on the *next* tick, once the previous
scene is present. Dropping them at once is a use-after-free.

### ffmpeg-sidecar cannot handle two outputs

`FfmpegCommand` maps streams to outputs by position, and with two outputs it
maps them wrongly: no `OutputFrame` events arrive at all, with no error. This
app needs two outputs, because the sound leaves the same process. So
`stream.rs` calls `take_stdout()` and reads frames itself, and takes the
picture size from the parsed `bgra` output stream in the log. Do not go back to
`filter_frames()`.

### One connection per camera, carrying both picture and sound

Many cameras allow only two or three sessions, and a second process asking for
the audio is refused. It fails silently, which is how "sound does not work"
happened. ffmpeg writes the picture to its stdout and the audio, as raw PCM, to
a loopback socket the app listens on.

The socket is drained all the time, whether or not a player is attached.
Nothing is queued while muted, but the bytes are read, because an unread socket
blocks ffmpeg and stalls the picture. If you change `audio::drain`, keep that
property.

A camera with no audio track makes ffmpeg refuse the audio output with
"Output file does not contain any stream", which kills the process. `decode_loop`
drops the audio output once it sees that and reconnects.

### Latency: measured, not guessed

`packaging/measure-latency.sh N` runs the app offscreen beside a bare `ffmpeg`
reading the same camera and reports how far each sits behind the wall clock,
read with `tesseract` from the clock the cameras burn into the picture. Use it
before and after any change to the ffmpeg arguments in `src/stream.rs`, and
quote the numbers in the commit.

What has been measured, over 25 samples each (±0.06s):

| Setting | Effect on a 1080p stream |
| --- | --- |
| `-threads 1` | 0.22s closer to live. Kept. |
| `-avioflags direct -max_delay 0 -reorder_queue_size 0` | no help alone, and they cancel the thread gain. Not used. |
| `-rtsp_transport udp` | no help. Not used. |

The app adds nothing over a bare `ffmpeg` pipeline: the same camera measured
3.16s behind its own clock directly and 3.23s through a restream. Frigate's UI
feels fresher because its live view passes H.264 through without decoding.
That is a different mechanism, not a faster one of the same kind, and it is not
reachable by tuning ffmpeg.

The camera's clock sits at some fixed offset from this machine, so only the
difference between two runs is meaningful.

### Windows

Release builds carry `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
in `src/main.rs`, or Windows opens a console behind the window. Debug builds
keep the console so `println!` works. `ffmpeg-sidecar` already passes
`CREATE_NO_WINDOW` for the children, so they do not flash a console either.

### GPUI Kit specifics

- Use `gpui_kit::assets::AllAssets`. The default `Assets` set carries a subset
  and renders icons such as `Camera` or `Volume2` as nothing.
- `h_flex` and `v_flex` come from `gpui_kit::component`, not `gpui_kit`.
- `.when_some` and friends need `gpui_kit::prelude::FluentBuilder` in scope.
- `div().id(...)` returns a `Stateful<Div>`, which is what `on_click` needs.
- Overlay controls on top of video use `.occlude()`, so a click does not reach
  the tile behind. Fixed colours, not theme colours: video is unpredictable.
- Do not use `use gpui_kit::*` inside a `#[cfg(test)]` module. It imports
  GPUI's `test` macro and shadows the built-in attribute. Import by name.

## The wall and how it is tested

`grid_columns`, `tile_size`, `enlarged_size` and `draw_solo` in `src/app.rs`
decide the layout and which feed is drawn. Their unit tests are the contract.
Change them together.

- The grid is the squarest one that holds every camera: 1, 2, 3–4 = 2×2,
  5–9 = 3×3.
- Tiles fill their cell, so the wall covers the window.
- `draw_solo` is why an enlarged camera falls back to its low-resolution feed
  when the camera refused the full-resolution connection. A black tile is worse
  than a soft one.

## Feeding a camera

The wall draws the low-resolution stream when a camera has one. The
full-resolution feed starts the first time that camera is enlarged
(`Camera::warm_solo`) and then stays, so moving between the wall and the
enlarged view never reconnects. A camera holds one connection at rest and two
after you have enlarged it once.
