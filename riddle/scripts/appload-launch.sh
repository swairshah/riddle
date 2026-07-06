#!/bin/sh
# AppLoad entry point, both modes on both devices:
#
#  - Windowed (manifest "qtfb": true): AppLoad hands us QTFB_KEY, so just run
#    the diary in-process against the qtfb window.
#  - Takeover (manifest "qtfb": false): AppLoad runs this inside xochitl's
#    world, which is about to be stopped — so detach the real launch into a
#    transient systemd unit (PID-1-owned, survives xochitl) and exit.
#
# Works wherever the bundle is installed: we resolve our own directory rather
# than hardcoding a path, so dropping this folder into AppLoad just works.
HERE=$(cd "$(dirname "$0")" && pwd)

if [ -n "$QTFB_KEY" ]; then
    # Oracle config: put your API key in oracle.env next to this script.
    if [ -f "$HERE/oracle.env" ]; then
        set -a; . "$HERE/oracle.env"; set +a
    fi
    HOME=/home/root exec "$HERE/riddle"
fi

systemctl is-active --quiet riddle-takeover && exit 0
systemd-run --unit=riddle-takeover --collect /bin/bash "$HERE/riddle-takeover.sh"
exit 0
