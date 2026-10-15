# CamViewer — agent notes

## Never test on the live desktop

The user is working on this PC while you work. Do not open the app on their
session, and do not send `xdotool`, `ydotool` or `wtype` input to it. The
clicks land in their windows.

Test offscreen with `cage`, which is installed:

```bash
cd ~/projects/private/camviewer-gpui
WLR_BACKENDS=headless cage -- sh -c '
  ./target/release/camviewer >/tmp/camviewer.log 2>&1 &
  sleep 10
  grim /tmp/camviewer.png
'
```

`cage` starts a headless Wayland session on the real GPU. The app and `grim`
both inherit `WAYLAND_DISPLAY` from it. The window size comes from the app, so
edit `src/main.rs` if you need another size.

Copy `/tmp/camviewer.png` out and look at it with the `read` tool.

Add cameras for a test run by writing `~/.config/camviewer/cameras.json`
first. Delete it afterwards if the run was only a test.

## Checks

```bash
cargo fmt
cargo clippy --all-targets
cargo test
```

## Releases

Release notes live in `CHANGELOG.md`. The release workflow publishes the
section that matches the tag, and a tag with no section fails the build. CI
also fails when the version in `Cargo.toml` has no section, so a version bump
and its notes must land together.

To cut a release:

1. Write the notes under `## [x.y.z]` in `CHANGELOG.md`. Describe what changed
   for the person using the app, not which files moved. Note the fixes and the
   additions separately. This text becomes the release body and the popup the
   app shows once after updating.
2. Set the same version in `Cargo.toml`.
3. Run the checks above.
4. `./packaging/release-notes.sh vx.y.z` prints exactly what the release will
   say. Read it before tagging.
5. Commit, then `git tag -a vx.y.z -m "CamViewer x.y.z"` and
   `git push origin vx.y.z`.
6. Confirm with `gh release view vx.y.z` that all four assets and the notes
   are there.

Do not use GitHub's generated notes. They list commits, which is not what the
person updating the app needs to read.

## Layout rules that tests rely on

`grid_columns` and `tile_size` in `src/app.rs` decide the wall layout, and
`wanted_url` decides which stream a camera runs. Their unit tests in
`src/app.rs` are the contract. Change them together.