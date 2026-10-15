# Changelog

Every release gets a section here. The release workflow takes the section that
matches the tag and publishes it as the release notes, and a tag with no
section fails the build. Write the section before you tag.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
