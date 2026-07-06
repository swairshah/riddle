//! Power button, for takeover mode. The device is GRABBED so logind doesn't
//! also act on the press: the diary draws its sleep page first, then triggers
//! the suspend itself. If the grab fails we still see the press and draw, and
//! leave the actual suspend to logind.

use std::io;
use std::os::fd::RawFd;

use crate::evdev;

const KEY_POWER: u16 = 116;

pub struct PowerButton {
    fd: RawFd,
    pub grabbed: bool,
}

impl PowerButton {
    pub fn open() -> io::Result<Self> {
        // "snvs-powerkey" on both the rM2 and the Paper Pro; "power button"
        // kept as a fallback for other kernels.
        let (fd, path, grabbed) = evdev::open_by_name(&["powerkey", "power button"])?;
        eprintln!("riddle: power button {path} (grabbed: {grabbed})");
        Ok(Self { fd, grabbed })
    }

    /// True if a power-key press (value 1) was seen since the last drain.
    pub fn drain_pressed(&mut self) -> bool {
        let mut pressed = false;
        let mut buf = [0u8; evdev::EV_SIZE * 16];
        loop {
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
            for chunk in buf[..n as usize].chunks_exact(evdev::EV_SIZE) {
                let (etype, code, value) = evdev::parse(chunk);
                if etype == evdev::EV_KEY && code == KEY_POWER && value == 1 {
                    pressed = true;
                }
            }
        }
        pressed
    }
}

impl Drop for PowerButton {
    fn drop(&mut self) {
        evdev::ungrab(self.fd);
        unsafe {
            libc::close(self.fd);
        }
    }
}

/// The kernel's successful-suspend counter — the authoritative "we slept"
/// signal. (Clock heuristics fail here: on this kernel CLOCK_MONOTONIC keeps
/// advancing across deep sleep, verified on-device.)
pub fn suspend_count() -> u64 {
    std::fs::read_to_string("/sys/power/suspend_stats/success")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// After resume, Wi-Fi is often stranded: wpa_supplicant fails a few attempts
/// while the radio settles and marks the network TEMP-DISABLED, and with
/// xochitl stopped nobody clears it. Nudge it back, detached, best-effort.
pub fn wifi_heal() {
    let script = "for i in 1 2 3 4 5 6 7 8 9 10; do \
        state=$(wpa_cli -i wlan0 status 2>/dev/null | grep ^wpa_state | cut -d= -f2); \
        [ \"$state\" = COMPLETED ] && exit 0; \
        wpa_cli -i wlan0 enable_network all >/dev/null 2>&1; \
        wpa_cli -i wlan0 reassociate >/dev/null 2>&1; \
        sleep 3; \
        done";
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
