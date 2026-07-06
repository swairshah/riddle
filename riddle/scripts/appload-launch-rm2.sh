#!/bin/sh
# AppLoad entry point for the windowed qtfb build (reMarkable 2).
# AppLoad allocates the framebuffer and passes QTFB_KEY in our environment;
# riddle sees it and picks the qtfb backend. xochitl keeps running — no
# takeover, no systemd units, nothing outside this folder.
HERE=$(cd "$(dirname "$0")" && pwd)

# Oracle config: put your API key in oracle.env next to this script.
if [ -f "$HERE/oracle.env" ]; then
    set -a; . "$HERE/oracle.env"; set +a
fi

exec "$HERE/riddle" >>/tmp/riddle.log 2>&1
