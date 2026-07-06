#!/bin/bash
# Cross-build riddle for the reMarkable 2 (armv7 hard-float) and assemble a
# ready-to-drop AppLoad bundle in dist/rm2/riddle/.
#
# Static musl build — no glibc version coupling with the device, runs on any
# rM2 OS. Prereqs:
#   rustup target add armv7-unknown-linux-musleabihf
#   apt install gcc-arm-linux-gnueabihf     (compiles ring's C; rust-lld links)
#
# The one binary contains BOTH display paths, chosen at runtime:
#   - windowed (qtfb/AppLoad) when QTFB_KEY is set
#   - takeover via the rm2fb server otherwise (pure syscalls, nothing to link
#     — no quill/SDK needed on the rM2, unlike the Paper Pro)
set -euo pipefail
cd "$(dirname "$0")"

TARGET=armv7-unknown-linux-musleabihf
export CC_armv7_unknown_linux_musleabihf=${CC_armv7_unknown_linux_musleabihf:-arm-linux-gnueabihf-gcc}
export AR_armv7_unknown_linux_musleabihf=${AR_armv7_unknown_linux_musleabihf:-arm-linux-gnueabihf-ar}

cargo build --release --target $TARGET "$@"

OUT=target/$TARGET/release
DIST=dist/rm2/riddle
rm -rf dist/rm2
mkdir -p "$DIST"
cp "$OUT/riddle" "$DIST/riddle"
cp scripts/appload-launch.sh scripts/riddle-takeover.sh "$DIST/"
cp oracle.env.example icon.png "$DIST/" 2>/dev/null || true

# Windowed by default (runs inside xochitl, safest first try). For full
# takeover — instant ink, needs the rm2fb service — flip "qtfb" to false.
cat > "$DIST/external.manifest.json" <<'EOF'
{
  "name": "The Diary",
  "application": "appload-launch.sh",
  "qtfb": true
}
EOF

echo
echo "bundle ready: $DIST"
echo "install:  scp -O -r $DIST root@10.11.99.1:/home/root/xovi/exthome/appload/"
echo "then add your key:  cp oracle.env.example oracle.env  (in that folder)"
