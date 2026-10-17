# Changelog

Every release gets a section here. The release workflow takes the section that
matches the tag and publishes it as the release notes, and a tag with no
section fails the build. Write the section before you tag.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.4] - 2026-10-17

### Changed

- The tooltip that appeared when the pointer rested on a camera is gone.
  Enlarging a camera now prints a data strip under the title block: the host,
  which of the camera's two streams is on screen, the decoded picture size, the
  measured frame rate, the codec, and how long the connection has held. The
  rate reads 0.0 FPS when a picture stops, so a frozen camera cannot look live.
- The "RACK ELEVATION" subtitle under the app name is gone.
- The numbered margin gutter (`R1`, `R2`…) is gone. The wall now uses the 64px
  it reserved.
- The measured width row above the wall is gone. The wall now uses its 26px
  height.
- The mute button now sits beside the Live state. The remove button keeps its
  width while it is hidden, now before the mute button, so the caption does not
  move when it appears.

## [0.4.3] - 2026-10-17

### Fixed

- The wall no longer stutters when a camera moves. The window asked for a new
  picture only every 33 ms. A 25 fps camera makes one every 40 ms, so each
  picture was shown for 33 ms and the next for 66 ms, and every fifth one
  hitched. The window now looks every 5 ms.
- The picture no longer freezes for up to half a second. The decoder was
  re-timing each stream to its nominal frame rate, and these cameras skip
  pictures and jump their timestamps, so it held some pictures and dropped
  others. Every decoded picture now reaches the wall, in order, as the camera
  sends it. A camera that skips a picture still holds that picture's time,
  because there is no picture to show.

### Changed

- The sheet follows the system's light or dark appearance. On a dark desktop it
  is the same drawing on dark paper: the rules, the names and the blue are
  lifted until they read, and the picture field keeps its dark ground, so a
  camera is judged against the same field at any hour. Changing the system
  appearance changes the sheet while the app runs.

## [0.4.2] - 2026-10-17

### Fixed

- The bottom of the last row of cameras is no longer cut off. The sheet is now
  measured against the box the window frame actually gave it rather than the
  whole window surface, which is a little larger wherever the frame keeps its
  shadow and its border inside the window. Before, the last row of captions was
  clipped by the bottom of the window, and every unit was squeezed a little
  narrower than the picture drawn in it.
- One camera drawn alone is measured the same way, so its picture and its
  caption fit the window instead of running past the bottom of it.
- Enlarging a camera now really shows the stream it is enlarged for. The
  enlarged view was asking for the wall's picture, so the sharp stream the
  camera had been connected for was never the one on screen.

### Changed

- A camera's full-resolution connection is no longer held for the rest of the
  session after the first look. Going back to the wall switches the picture to
  the low-resolution stream at once and gives the second connection back after
  30 seconds. Looking at the same camera again within those 30 seconds still
  costs nothing, and a wall left open no longer keeps sessions that the camera
  may want for something else. If you are listening to a camera when its second
  connection goes, the sound moves to the stream the wall draws.

## [0.4.1] - 2026-10-16

### Fixed

- The app no longer asks about a new release once and then forgets. It asks
  again every six hours, so a wall left open for a week still hears about a
  release.
- A check that could not reach GitHub is retried ten minutes later instead of
  being lost until the next restart. The first check runs a second after the
  window opens, which is exactly when a machine's network may still be
  settling.
- Answering **Later** now means that build is not offered again in the same
  run, and a newer one still is. Before, the next check would have offered the
  same build a second time.

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
