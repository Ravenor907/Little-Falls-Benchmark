#!/bin/sh
# Little Falls installer: downloads the latest release from GitHub and adds
# it to your application menu. No root needed; everything goes in your home
# folder, so it works on immutable systems like SteamOS and Bazzite.
#
#   curl -fsSL https://raw.githubusercontent.com/Ravenor907/Little-Falls-Benchmark/main/install.sh | sh
#
# Run it again to update. Uninstall with:  sh install.sh --uninstall
set -eu

REPO="Ravenor907/Little-Falls-Benchmark"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/little-falls"
BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
APPIMAGE="$APP_DIR/LittleFalls.AppImage"

say() { printf '\033[1;32m::\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$1" -O "$2"
    else
        die "curl or wget is required"
    fi
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$APPIMAGE" "$BIN_DIR/little-falls" "$DESKTOP_DIR/io.github.Ravenor907.LittleFalls.desktop" \
          "$ICON_DIR/io.github.Ravenor907.LittleFalls.svg"
    rmdir "$APP_DIR" 2>/dev/null || true
    say "Little Falls removed. Your saved results in $APP_DIR/results were kept."
    exit 0
fi

[ "$(uname -m)" = "x86_64" ] || die "Little Falls currently supports x86_64 only"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

say "Finding the latest release..."
fetch "https://api.github.com/repos/$REPO/releases" "$TMP/releases.json"
# First AppImage asset of the newest release (pre-releases included).
URL=$(grep -o '"browser_download_url": *"[^"]*\.AppImage"' "$TMP/releases.json" | head -n1 | sed 's/.*"\(https[^"]*\)"/\1/')
[ -n "$URL" ] || die "no release with an AppImage found at https://github.com/$REPO/releases"
VERSION=$(printf '%s' "$URL" | sed 's|.*/download/\([^/]*\)/.*|\1|')

say "Downloading Little Falls $VERSION..."
mkdir -p "$APP_DIR" "$BIN_DIR" "$DESKTOP_DIR" "$ICON_DIR"
fetch "$URL" "$TMP/LittleFalls.AppImage"
chmod +x "$TMP/LittleFalls.AppImage"
mv -f "$TMP/LittleFalls.AppImage" "$APPIMAGE"
ln -sf "$APPIMAGE" "$BIN_DIR/little-falls"

fetch "https://raw.githubusercontent.com/$REPO/main/packaging/appimage/little-falls.svg" \
      "$ICON_DIR/io.github.Ravenor907.LittleFalls.svg" || true

cat > "$DESKTOP_DIR/io.github.Ravenor907.LittleFalls.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Little Falls
GenericName=System Benchmark
Comment=Graphics, CPU and memory benchmark
Exec=$APPIMAGE
Icon=io.github.Ravenor907.LittleFalls
Terminal=false
Categories=Game;System;Monitor;
Keywords=benchmark;gpu;cpu;vulkan;performance;
EOF
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

say "Installed Little Falls $VERSION"
echo "   Launch it from your application menu, or run: little-falls"
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) echo "   (Add $BIN_DIR to your PATH to use the little-falls command.)";; esac
echo "   On SteamOS game mode: in desktop mode, add Little Falls to Steam as a non-Steam game."
