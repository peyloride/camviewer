# CamViewer

A small RTSP camera viewer for Windows, Linux and macOS. Written in Rust with
[GPUI](https://gpui.rs) and the [GPUI Kit](https://gpui-kit.com) component
library.

## Features

- Add cameras by RTSP URL. The list is saved to disk and reloaded at startup.
- Watch every camera at once in a grid that fills the window.
- Click a tile to show that camera alone. Click the picture again to go back.
- Give a camera a second, low-resolution URL. The grid uses it to save
  bandwidth and CPU; enlarging the camera switches to the full-resolution URL.
- Mute or unmute a camera. Only one camera plays sound at a time.
- Remove a camera from its tile.
- The window follows the operating system light or dark appearance.

## Install

Download the build for your machine from the
[releases page](https://github.com/peyloride/camviewer/releases/latest), then
make it runnable:

| Platform | File |
| --- | --- |
| Linux x86_64 | `camviewer-x86_64-unknown-linux-gnu` |
| macOS Apple silicon | `camviewer-aarch64-apple-darwin` |
| macOS Intel | `camviewer-x86_64-apple-darwin` |
| Windows x86_64 | `camviewer-x86_64-pc-windows-msvc.exe` |

```bash
# Linux and macOS
chmod +x camviewer-x86_64-unknown-linux-gnu
./camviewer-x86_64-unknown-linux-gnu
```

The builds are not code signed. On macOS, clear the download flag once:

```bash
xattr -d com.apple.quarantine ./camviewer-aarch64-apple-darwin
```

On Windows, SmartScreen warns about an unknown publisher. Choose **More
info** then **Run anyway**.

## Updates

The app checks GitHub for a newer release at startup. When one exists, a strip
appears under the toolbar with the version and two choices: **Later** or
**Install and restart**. Nothing is downloaded or replaced until you choose
**Install and restart**.

An update replaces the running executable and restarts the app. The check is
silent when the network is unavailable.

After the restart the app shows what changed, once. Dismiss it and it does not
come back; the next update brings its own notes.

The version in use appears in the toolbar, next to the app name.

The repository the app checks is `peyloride/camviewer`. Point a fork at another
repository at build time:

```bash
CAMVIEWER_REPOSITORY=you/camviewer cargo build --release
```

## Requirements

- Rust 1.90 or newer to build.
- The `ffmpeg` executable on `PATH`. Decoding runs in an `ffmpeg` child
  process, so the app itself needs no FFmpeg headers or libraries at build
  time.
- Linux only: the usual GPUI system packages (Wayland or X11, Vulkan or GL,
  fontconfig, and ALSA for audio).

Check that ffmpeg is reachable:

```bash
ffmpeg -version
```

## Build and run

```bash
cargo run --release
```

The first build takes several minutes: GPUI and its graphics stack are large.

### Install it for your user

```bash
cargo build --release
./packaging/install.sh
```

That puts the binary in `~/.local/bin/camviewer`, adds
`~/.local/share/applications/camviewer.desktop`, and installs the icon, so the
app appears in your launcher.

It installs into your home directory on purpose. The app replaces its own
executable to update, so it needs a location you can write to.

## Use

1. Click **Add camera**.
2. Type a name, then the RTSP URL, for example
   `rtsp://admin:secret@192.168.1.64:554/stream1`.
3. Optional: type a low-resolution URL, for example
   `rtsp://admin:secret@192.168.1.64:554/stream2`.
4. Click **Add**, or press Enter in any field.

| Action | How |
| --- | --- |
| Enlarge a camera | Click its tile, or focus it with Tab and press Enter. |
| Return to the wall | Click the picture, or the back arrow in the header. |
| Mute or unmute | Click the speaker icon on the tile or in the enlarged header. |
| Remove a camera | Hover a tile and click the bin icon. |

### The wall layout

The grid is always the squarest one that holds every camera:

| Cameras | Grid |
| --- | --- |
| 1 | One tile, as large as it fits. |
| 2 | Two side by side. |
| 3–4 | Two by two. |
| 5–9 | Three by three. |
| 10–16 | Four by four. |

Tiles fill their cell, so the wall covers the whole window. Each tile has a
label bar along the bottom, and the picture sits above it, letterboxed rather
than stretched. An empty cell in the last row offers to add the next camera.

## Configuration

The camera list is saved as soon as you add or remove a camera, and it is read
back at startup. Cameras survive closing and reopening the app.

The list lives in one JSON file:

| Platform | Path |
| --- | --- |
| Linux | `~/.config/camviewer/cameras.json` |
| macOS | `~/Library/Application Support/camviewer/cameras.json` |
| Windows | `%APPDATA%\camviewer\cameras.json` |

```json
[
  {
    "name": "Front door",
    "url": "rtsp://192.168.1.64:554/stream1",
    "url_sd": "rtsp://192.168.1.64:554/stream2"
  }
]
```

`url_sd` is optional. Passwords sit in this file in plain text, because an
RTSP URL carries them; protect the file with normal user permissions.

## How it works

```
  camera ──rtsp──▶ ffmpeg ──raw BGRA frames──▶ decoder thread
                                                    │
                                      shared frame slot (newest frame wins)
                                                    │
                                             33 ms UI timer
                                                    │
                                    GPUI image element ──▶ GPU
```

Each camera gets one decoder thread and one `ffmpeg` process. The thread keeps
only the newest frame, so a slow repaint drops frames instead of queueing them
and latency stays flat. Unmuting a camera starts a second `ffmpeg` process that
decodes the audio track to raw PCM, which `rodio` plays.

Switching between the low- and high-resolution URLs replaces the decoder
thread, so enlarging a camera costs a reconnect. Sound always comes from the
full-resolution URL, so a view change never interrupts it.

### Tuning

| Constant | File | Effect |
| --- | --- | --- |
| `MAX_WIDTH` | `src/stream.rs` | Longest decoded edge. Lower it to cut CPU and memory for a large camera wall. |
| `POLL_INTERVAL` | `src/app.rs` | How often the UI picks up new frames. |
| `IO_TIMEOUT_US` | `src/stream.rs` | How long a silent socket is tolerated before the stream reconnects. |
| `GAP`, `TOOLBAR_H`, `BAR_H`, `HEADER_H` | `src/app.rs` | Chrome sizes. `tile_size` and `enlarged_size` turn them into the picture boxes. |

### A note on the picture element

GPUI gives an image element an `aspect-ratio` taken from the picture when the
element has none. That ratio fights a percentage height: the element grows
taller than its parent and the parent clips it. `video()` in `src/app.rs`
states a definite size and a matching `aspect_ratio` instead. Keep it that way,
or the picture will be cropped rather than letterboxed.

To force one theme, replace the appearance observer in `CamViewer::new`
(`src/app.rs`) with `Theme::change(ThemeMode::Dark, None, cx)`.

## Tests

```bash
cargo test
```

The tests cover the wall layout, the enlarged box, and the low-resolution
stream choice. They do not open a window and do not need a camera.

To look at the app without touching the desktop, run it under a headless
compositor:

```bash
WLR_BACKENDS=headless cage -- sh -c '
  ./target/release/camviewer >/tmp/camviewer.log 2>&1 &
  sleep 10
  grim /tmp/camviewer.png
'
```

See `AGENTS.md` for the rest of the harness.

## Project layout

| File | Contents |
| --- | --- |
| `src/main.rs` | Application bootstrap and the main window. |
| `src/app.rs` | The camera wall: toolbar, grid, enlarged view, add dialog. |
| `src/stream.rs` | RTSP to BGRA frames, on a background thread. |
| `src/audio.rs` | Camera audio through `rodio`. |
| `src/config.rs` | Camera list persistence. |
| `src/update.rs` | GitHub release check and self-replacement. |
| `packaging/` | App icon, install script and release-note extractor. |
| `CHANGELOG.md` | The release notes, one section per version. |

## Releasing

Release notes are written in `CHANGELOG.md`. The workflow publishes the section
that matches the tag, so a tag without notes fails the build:

```bash
# 1. write the notes under '## [x.y.z]' in CHANGELOG.md
# 2. set the same version in Cargo.toml
./packaging/release-notes.sh v0.2.0   # preview exactly what will be published
git tag -a v0.2.0 -m "CamViewer 0.2.0"
git push origin v0.2.0
```

The asset names come from the Rust target triple, and `update::asset_name()`
builds the same name at run time. Change one, change the other.

Both workflows run on GitHub-hosted runners by default. To use your own Linux
runner, set a repository variable:

```
gh variable set LINUX_RUNNER --body 'self-hosted'
```

A runner registered to an organisation is only offered to repositories in that
organisation.

## Known limits

- Video is delivered as one image per frame through GPUI's image atlas. This
  works and stays smooth for a handful of cameras, but it is not a video
  pipeline: each frame is uploaded to the GPU again.
- The audio path opens a second RTSP session per unmuted camera. Cameras with a
  low session limit may refuse it; video is unaffected.
- Enlarging a camera reconnects, which takes about a second.
- The mute and remove icons answer the mouse only. Reach them with a pointer.
- The add dialog takes URLs as given. It does not discover cameras over ONVIF.
- The builds are not code signed, so macOS and Windows show a first-run warning.
- An update replaces the executable in place. On Linux, run it from a location
you can write to.

## Licence

MIT.