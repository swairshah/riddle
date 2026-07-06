#!/bin/bash
# Launch the diary in full-takeover mode on the reMarkable 2: stop xochitl,
# host the e-ink engine with the bundled rm2fb server (timower/rM2-stuff),
# run riddle against it (instant ink), ALWAYS restore xochitl on exit.
#
# Exit the diary: 5-finger tap or SIGTERM; the power button sleeps instead.
# Escape hatch if anything wedges: ssh rm2 'systemctl start xochitl'.

SERVER_PID=

restore() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill -INT "$SERVER_PID" 2>/dev/null   # SIGINT = its clean shutdown
        for _ in 1 2 3 4 5 6 7 8 9 10; do
            kill -0 "$SERVER_PID" 2>/dev/null || break
            sleep 0.3
        done
        kill -9 "$SERVER_PID" 2>/dev/null
    fi
    systemctl start xochitl
}
trap restore EXIT INT TERM

HERE=$(cd "$(dirname "$0")" && pwd)

# Oracle config: put your API key in oracle.env next to this script.
if [ -f "$HERE/oracle.env" ]; then
    set -a; . "$HERE/oracle.env"; set +a
fi

systemctl stop xochitl
sleep 0.5

# The bundled server dlopens the vendor libqsgepaper.so and hosts the panel.
LD_LIBRARY_PATH="$HERE" "$HERE/rm2fb_server" >/tmp/rm2fb.log 2>&1 &
SERVER_PID=$!

# Wait for its control socket (init takes a moment: waveform table load).
for _ in $(seq 1 100); do
    [ -S /var/run/rm2fb.sock ] && break
    kill -0 "$SERVER_PID" 2>/dev/null || { echo "rm2fb server died, see /tmp/rm2fb.log"; exit 1; }
    sleep 0.1
done
sleep 0.5

cd "$HERE"
HOME=/home/root "$HERE/riddle" >>/tmp/riddle.log 2>&1
echo "riddle-takeover: diary closed ($?), restoring xochitl"
