#!/bin/sh
# AppLoad entry point for takeover mode on the reMarkable 2. AppLoad runs this
# inside xochitl's world, which is about to be stopped — so detach the real
# launch into a transient systemd unit (PID-1-owned, survives xochitl) and
# exit immediately.
HERE=$(cd "$(dirname "$0")" && pwd)
systemctl is-active --quiet riddle-takeover && exit 0
systemd-run --unit=riddle-takeover --collect /bin/bash "$HERE/riddle-takeover-rm2.sh"
exit 0
