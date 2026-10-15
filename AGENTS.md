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

## Layout rules that tests rely on

`grid_columns` and `tile_size` in `src/app.rs` decide the wall layout, and
`wanted_url` decides which stream a camera runs. Their unit tests in
`src/app.rs` are the contract. Change them together.