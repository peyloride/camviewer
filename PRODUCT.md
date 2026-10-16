# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Users

People who own RTSP cameras and a desktop computer: home lab and small site
owners who already run a recorder such as Frigate. They watch their own
cameras, on their own machine, with no account and no cloud.

Primary situation, confirmed by the user: the wall is left open all day. It is
pushed to full screen on a second display when the user wants it, and it is
also run small beside other work on the main display.

Confirmed scale: the user runs 3 cameras. Typical owners run 4 to 8. More than
9 is not a case worth designing for.

Other people install this app from the releases page. They are not known to
the author and they get no help, so the first run has to explain itself.

## Product Purpose

Show every camera at once in one native window, and prove the wall is telling
the truth. Success means the user trusts what they see without checking any
camera by hand.

## Positioning

One binary on the desktop. No server, no browser tab, no account, no cloud.
The app decodes RTSP itself through a child `ffmpeg` and draws the pictures
with the GPU. A recorder beside it stores footage; this app shows only what is
happening now, and it removes everything between the camera and the screen
that a browser-based viewer cannot.

## Operating Context

- The user already runs an NVR, which holds several sessions on each camera.
  A camera often allows only two or three sessions, so the app holds one
  connection per camera and carries the sound on the same connection.
- Cameras sit on a local network address and usually publish a second,
  low-resolution stream that the wall uses.
- The window runs for hours. The user glances at it. They do not sit in front
  of it.
- Cameras refuse sessions, the network drops, and machines sleep. The app has
  to stay honest through all of it.

## Capabilities and Constraints

Confirmed function:

- Add a camera by RTSP URL, with an optional second low-resolution URL. The
  wall uses the low-resolution stream; enlarging a camera switches to the
  full-resolution one.
- A wall of tiles that fills the window, and one camera drawn alone.
- Sound from one camera at a time.
- Remove a camera from its tile.
- The camera list is one JSON file in the config directory.
- The app asks GitHub for a newer release, replaces its own executable, and
  restarts. `--version` prints and exits before a window opens.
- One fixed appearance: the sheet is vellum and graphite, and it never follows
  the operating system into a dark mode. The operating system still draws the
  window's own frame.

Confirmed boundary, stated by the user:

- Live only. No recording, no playback, no events, no motion detection. The
  recorder beside it owns all of that.
- No account, no cloud, no server component.
- Desktop only: Linux, macOS and Windows, one binary per platform.
- `ffmpeg` must be on `PATH`.

Constraints future work must preserve:

- One connection per camera, carrying both picture and sound. A second
  connection for the sound is refused by real cameras and fails silently.
- The ffmpeg arguments in `src/stream.rs` are measured. Quote
  `packaging/measure-latency.sh` numbers in any commit that changes them.
- `grid_columns`, `tile_size`, `enlarged_size` and `draw_solo` in `src/app.rs`
  are the layout contract, and their unit tests change with them.
- Frames are dropped one tick after they are replaced, because the GPU reads
  an image while it is drawn.

Undecided:

- Whether the app should also offer a dark rendition of the sheet. The build
  holds one appearance on purpose, so this is a product decision rather than a
  styling one.

## Brand Commitments

The name is CamViewer. The voice is plain and technical: it states what the
app does and what it costs, and it admits limits instead of hiding them. No
hype, no exclamation marks, no marketing adjectives. This voice is taken from
`README.md` and `CHANGELOG.md`, and it is binding.

Assets: `packaging/camviewer.svg` and the 256 and 512 pixel PNG renders of it.

## Evidence on Hand

- `README.md`: features, install, use, requirements.
- `CHANGELOG.md`: release notes, written for the person updating the app.
- `packaging/e2e.sh`: five end-to-end checks against real RTSP streams.
- `tests/ui.rs`: six interaction tests that drive the real view.
- `packaging/install.sh`, `packaging/measure-latency.sh`.
- Latency numbers measured over 25 samples on a 1080p stream, recorded in
  `AGENTS.md`.

No customers, benchmarks, testimonials or press exist. Do not invent any.

## Product Principles

1. Never lie about what is on screen. A frozen, refused or missing picture is
   reported as such.
2. The wall answers "is everything up?" in one glance, with no click.
3. One connection per camera. Camera sessions and bandwidth are the scarce
   resources.
4. One window, one binary. No server, no daemon, no account.
5. Stay close to live. Latency is a feature with measurements behind it.

## Accessibility & Inclusion

No product-specific standard has been agreed. Preserve what exists: tiles are
focusable, Enter enlarges the focused camera, and every control has a tooltip.
