# Animagraph

A desktop app for animating photos: still photos or short videos where
only part of the picture moves, played as a seamless loop.

- **Photo:** paint the area that should move, draw arrows for the direction,
  and the app renders a seamless loop.
- **Video:** select an area that stays frozen (or the only area that plays),
  pick the frozen frame, trim, and make a loop.
- Finish with crop, rotation, border (including Polaroid), caption with any
  font, color look, tone curve and grain.
- Save as GIF, MP4, MOV, PNG frames, or a single PNG/JPEG image.

Everything runs on your own computer. Nothing is uploaded.

## Use the ready-made app (macOS)

1. Download `Animagraph-<version>-macos-<cpu>.zip` from the
   repository's **Releases** page. Use `arm64` for Apple silicon Macs.
2. Double-click the zip, then drag **Animagraph.app** into
   **Applications**.
3. The first time, macOS may block it because the app is not notarized by
   Apple. Right-click the app and choose **Open**, then **Open** again.
   If there is no Open button, go to **System Settings → Privacy &
   Security** and click **Open Anyway**.

Requires macOS 11 or later.

## Build it yourself

1. Install the Xcode Command Line Tools: `xcode-select --install`
2. Install Rust from https://rustup.rs
3. Get the code and go into its folder:
   ```
   git clone <repository-url>
   cd <repository>
   ```
4. Run it directly: `cargo run --release`
5. Or make the double-click app with its icon:
   ```
   ./make-mac-app.sh            # creates dist/Animagraph.app
   ./make-mac-app.sh --install  # also copies it into Applications
   ./make-mac-app.sh --zip      # also makes a zip to share
   ```
   `--keep-cache` keeps the ~1 GB build cache so the next build is faster.

The app builds for the processor of the Mac it is built on. To publish both
versions, build once on an Apple silicon Mac and once on an Intel Mac.

## Publishing a release (for the maintainer)

1. Run `./make-mac-app.sh --zip`.
2. On GitHub, open **Releases → Draft a new release**, create a tag such as
   `v0.1.0`, and attach the zip from `dist/`.

To remove the "not notarized" warning for other people, the app has to be
signed with an Apple Developer ID and notarized by Apple (this needs a paid
Apple Developer account).

## Files and data

- Fonts you add, and your panel and caption-font choices, are stored in
  `~/Library/Application Support/Animagraph/`.
- Lightroom presets (.xmp, or .dng preset files from Lightroom mobile) can be
  imported under Styles → Lightroom presets and are kept in the same folder (`presets/`).
  Their settings fill the matching controls in Edit (light, color,
  texture, clarity, sharpening, tone curves, HSL, color grading, grain,
  vignette). The app's own processing gives a close match, not an exact
  copy. Camera and creative profiles, masks, noise reduction, lens
  corrections and camera calibration are not used.
- Loops in the Results strip stay in memory until you close the app.
  Export them to keep them.
- Camera RAW files (RAF, DNG, CR2/CR3, NEF, ARW, ORF, RW2 and more) and
  iPhone HEIC photos are developed with the RAW support built into macOS.
  Very new cameras need a recent macOS version.
- MP4, MOV and M4V videos open with the video support built into macOS.
  WebM, MKV and AVI need ffmpeg (`brew install ffmpeg`).

## Useful options

- `--screenshot <file.png>`: open, draw, save a picture of the window, and quit.
- `--write-iconset <folder>`: write the app icon at all sizes macOS needs, and quit.
- Any file names after the options are opened at start.
