#!/usr/bin/env bash
# Stage the reMarkable 2 AppLoad bundle into dist/riddle-rm2/.
# Prereq: cargo build --release --target armv7-unknown-linux-musleabihf --features rm2
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=target/armv7-unknown-linux-musleabihf/release/riddle
[ -f "$BIN" ] || { echo "build first: cargo build --release --target armv7-unknown-linux-musleabihf --features rm2" >&2; exit 1; }

rm -rf dist/riddle-rm2
mkdir -p dist/riddle-rm2
install -m 755 "$BIN" dist/riddle-rm2/riddle
install -m 755 scripts/appload-launch-rm2.sh dist/riddle-rm2/
install -m 644 external.manifest.rm2.json dist/riddle-rm2/external.manifest.json
install -m 644 icon.png oracle.env.example dist/riddle-rm2/

echo "staged: $(du -sh dist/riddle-rm2 | cut -f1) in dist/riddle-rm2/"
echo "deploy: scp -O -r dist/riddle-rm2 root@<tablet>:/home/root/xovi/exthome/appload/riddle"
