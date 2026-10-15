#!/usr/bin/env bash
#
# Install CamViewer for the current user.
#
#   ./packaging/install.sh [path-to-binary]
#
# The app updates itself in place, so it must live somewhere the user can
# write. That is why the default is ~/.local/bin rather than /usr/local/bin.

set -euo pipefail

BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
APP_DIR="${APP_DIR:-$HOME/.local/share/applications}"
ICON_DIR="${ICON_DIR:-$HOME/.local/share/icons/hicolor/512x512/apps}"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"

source_binary="${1:-$root/target/release/camviewer}"
icon="$here/camviewer-512.png"

if [[ ! -x "$source_binary" ]]; then
  echo "no binary at $source_binary" >&2
  echo "build it first:  cargo build --release" >&2
  exit 1
fi

install -d "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
install -m 755 "$source_binary" "$BIN_DIR/camviewer"
install -m 644 "$icon" "$ICON_DIR/camviewer.png"

cat > "$APP_DIR/camviewer.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=CamViewer
Comment=Watch RTSP cameras
Exec=$BIN_DIR/camviewer
Icon=camviewer
Terminal=false
Categories=AudioVideo;Video;Network;
Keywords=camera;rtsp;nvr;surveillance;
StartupWMClass=camviewer
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$(dirname "$(dirname "$ICON_DIR")")" >/dev/null 2>&1 || true
fi

echo "installed $BIN_DIR/camviewer"
echo "installed $APP_DIR/camviewer.desktop"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "note: $BIN_DIR is not on your PATH" ;;
esac
