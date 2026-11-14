#!/bin/bash
# Builds "Animagraph.app", a regular double-click Mac app with its
# icon, in the dist/ folder next to this script.
#
# Usage:  ./make-mac-app.sh [--zip] [--install] [--keep-cache]
#   --zip         also make dist/Animagraph-<version>-macos-<cpu>.zip to share
#   --install     copy the app into /Applications (or ~/Applications)
#   --keep-cache  keep the ~1 GB release build cache (faster next time)
#
# Needs: macOS with Xcode Command Line Tools and Rust (https://rustup.rs).
set -euo pipefail

ZIP=0
INSTALL=0
KEEP_CACHE=0
for arg in "$@"; do
  case "$arg" in
    --zip) ZIP=1 ;;
    --install) INSTALL=1 ;;
    --keep-cache) KEEP_CACHE=1 ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "Unknown option: $arg (try --help)"; exit 2 ;;
  esac
done

cd "$(dirname "$0")"
NAME="Animagraph"
BIN="animagraph"
BUNDLE_ID="org.animagraph.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
CPU="$(uname -m)"
APP="dist/$NAME.app"

echo "1/4 Building the app (release)…"
cargo build --release

echo "2/4 Putting the app together…"
rm -rf "$APP" dist/iconset.tmp
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/release/$BIN" "$APP/Contents/MacOS/$BIN"

echo "3/4 Making the icon…"
"$APP/Contents/MacOS/$BIN" --write-iconset dist/iconset.tmp/AppIcon.iconset
iconutil -c icns dist/iconset.tmp/AppIcon.iconset -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf dist/iconset.tmp

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>$BIN</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.photography</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "4/4 Signing for this Mac (ad hoc)…"
codesign --force --deep --sign - "$APP"

if [ "$ZIP" = 1 ]; then
  ZIPFILE="dist/${NAME// /-}-$VERSION-macos-$CPU.zip"
  rm -f "$ZIPFILE"
  ditto -c -k --keepParent "$APP" "$ZIPFILE"
  echo "Zip to share: $ZIPFILE ($(du -h "$ZIPFILE" | cut -f1))"
fi

if [ "$INSTALL" = 1 ]; then
  DEST="/Applications"
  [ -w "$DEST" ] || DEST="$HOME/Applications"
  mkdir -p "$DEST"
  rm -rf "$DEST/$NAME.app"
  cp -R "$APP" "$DEST/"
  echo "Installed in $DEST"
fi

if [ "$KEEP_CACHE" = 0 ]; then
  cargo clean --release >/dev/null 2>&1 || true
fi

echo "Done: $APP ($(du -sh "$APP" | cut -f1))"
