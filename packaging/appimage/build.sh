#!/usr/bin/env bash
# Packages the release binary as an AppImage plus a plain tarball.
# Usage: packaging/appimage/build.sh <binary> <output-dir>
#
# The AppImage runs on immutable distros (SteamOS, Bazzite) without
# installing anything. It relies on the system's Vulkan driver, which
# every gaming distro ships.
set -euo pipefail

BIN=${1:?binary path}
OUT=${2:?output dir}
HERE=$(cd "$(dirname "$0")" && pwd)
VERSION=$(grep -m1 '^version' "$HERE/../../Cargo.toml" | cut -d'"' -f2)
ARCH=x86_64

mkdir -p "$OUT"
WORK=$(mktemp -d)
APPDIR="$WORK/LittleFalls.AppDir"

install -Dm755 "$BIN" "$APPDIR/usr/bin/little-falls"
install -Dm644 "$HERE/little-falls.desktop" "$APPDIR/little-falls.desktop"
install -Dm644 "$HERE/little-falls.svg" "$APPDIR/little-falls.svg"
install -Dm644 "$HERE/little-falls.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/little-falls.svg"
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/little-falls" "$@"
EOF
chmod +x "$APPDIR/AppRun"

curl -fsSL -o "$WORK/appimagetool" \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
chmod +x "$WORK/appimagetool"
ARCH=$ARCH "$WORK/appimagetool" --appimage-extract-and-run \
  "$APPDIR" "$OUT/LittleFalls-$VERSION-$ARCH.AppImage"

tar -C "$(dirname "$BIN")" -czf "$OUT/little-falls-$VERSION-linux-$ARCH.tar.gz" little-falls
(cd "$OUT" && sha256sum ./* > SHA256SUMS)
rm -rf "$WORK"
echo "Built:"; ls -l "$OUT"
