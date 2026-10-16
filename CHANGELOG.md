# Changelog

Every release gets a section here. The release workflow takes the section that
matches the tag and publishes it as the release notes, and a tag with no
section fails the build. Write the section before you tag.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-10-16

### Added

- The wall tells you when a picture has stopped moving. A camera that is still
  connected but has sent no frame for three seconds is marked `Frozen`, so a
  still picture can no longer pass for a live one.
- Every camera shows what it is doing, all the time: `Live`, `Connecting`,
  `Frozen` or `No signal`. A connection that is still trying counts the time
  it has spent trying, so a camera that will never answer cannot be mistaken
  for one that is about to.
- A camera that cannot be reached says why, in words: *Address not found*,
  *Password refused*, *Session refused*, *Timed out*, *Host unreachable*. The
  decoder's own message is on the tile's tooltip.
- **Full screen**, for a wall left up on a second screen.

### Changed

- The interface is redrawn as a rack elevation on a sheet: a title block, a
  dimension line that measures a tile as you resize the window, a numbered
  margin, and one ruled entry per camera carrying its name, its address and
  its state. Square corners, graphite rules, and one blue spent on nothing but
  a camera that has something wrong with it.
- The window holds one appearance instead of following the system. A drawing
  on vellum cannot be inverted and stay the same drawing, and a picture is
  then judged against the same ground at any hour.
- The app carries its own two type faces instead of borrowing whatever the
  machine has installed. Both are under the SIL Open Font Licence.
- The camera count and the version moved into the sheet's meta line, where a
  drawing keeps them.

### Fixed

- A camera name is no longer cut down to a few letters. On a narrow tile the
  address and the controls step aside first, so the name and the state keep
  their room.
- An empty wall now says what to do and offers the button to do it, instead of
  leaving a large blank window.
- Every control is a real button, so the keyboard reaches all of them and
  Enter or Space works them.
- The state printed on a tile is readable. The old text sat below the contrast
  floor on the black picture field.
- The first frame no longer flashes in the wrong appearance on startup.

## [0.3.2] - 2026-10-16

### Added

- A test suite for the interface: clicking a tile enlarges its camera, the
  picture and the back button return to the wall, the speaker toggles one
  camera at a time, and the bin removes a camera.

## [0.3.1] - 2026-10-16

### Fixed

- Sound works again, and needs no connection of its own. The camera's audio
  now comes off the same `ffmpeg` process that decodes the picture, written to
  a loopback socket. A separate audio process was being refused by cameras
  that limit how many sessions they allow, which is why unmuting did nothing.
- A camera with no audio track still shows its picture; the audio output is
  dropped once ffmpeg reports that there is nothing to carry.

### Changed

- Two sessions per camera became one. That leaves room for a second
  connection when you enlarge a camera.

## [0.3.0] - 2026-10-16

### Changed

- Switching between the wall and an enlarged camera no longer reconnects. The
  full-resolution feed starts the first time you enlarge that camera and then
  stays connected, so moving between the two views changes the picture at once.
- A camera sits at one connection while you watch the wall, and two after you
  have enlarged it once.
- Decoding uses one thread instead of one per core. Measured against the
  camera's own on-screen clock, that is 0.22s closer to live on a 1080p stream.
  The other low-latency flags were measured as well; they gave that back and
  are not used.

### Fixed

- An enlarged camera whose full-resolution stream is refused, because the
  camera limits how many sessions it allows, now shows the low-resolution
  picture instead of an error.
- The wall keeps working on a camera that allows only one session.

## [0.2.0] - 2026-10-15

### Added

- The version shows in the toolbar.
- After an update the app shows what changed, once, as a popup. Dismiss it and
  it stays dismissed.
- `--version` prints the build and exits before a window opens, so scripts and
  packaging can ask which version is installed.
- An install script, `packaging/install.sh`, that puts the app in
  `~/.local/bin` and adds it to the desktop launcher.
- An application icon.

### Fixed

- Windows no longer opens a console window behind the app.
- Camera streams that are not the shape of their tile are letterboxed rather
  than cropped.
- The label bar on a tile no longer covers the bottom of the picture.

### Changed

- Every release now carries written notes. The release workflow publishes the
  matching section of `CHANGELOG.md`, and a tag without one fails the build.

## [0.1.2] - 2026-10-15

Superseded by 0.2.0.

## [0.1.1] - 2026-10-15

### Added

- An install script, `packaging/install.sh`, that puts the app in
  `~/.local/bin` and adds it to the desktop launcher.
- An application icon.

### Fixed

- Windows no longer opens a console window behind the app.
- Camera streams that are not the shape of their tile are letterboxed rather
  than cropped.
- The label bar on a tile no longer covers the bottom of the picture.

## [0.1.0] - 2026-10-15

### Added

- First release.
- A wall of RTSP cameras that fills the window, with a grid that grows with the
  number of cameras.
- Click a tile to watch one camera alone; click the picture to go back.
- Optional low-resolution stream per camera, used in the wall. Enlarging a
  camera switches to the full-resolution stream.
- Mute and unmute, one camera at a time. Audio comes from `ffmpeg` through
  `rodio`.
- Cameras are saved to a JSON file and reloaded at startup.
- The window follows the operating system light or dark appearance.
- Builds for Linux x86_64, macOS arm64, macOS x86_64 and Windows x86_64.
- Self-update: the app checks GitHub for a newer release, and installs it on
  approval.
