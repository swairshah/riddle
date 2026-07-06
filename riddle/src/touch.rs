//! Raw touch input for takeover mode: 5-finger tap = quit gesture.
//!
//! Device names: "touchscreen"-ish on the Paper Pro; the reMarkable 2 panel
//! reports as "pt_mt" (Parade) or "cyttsp5_mt" (Cypress) with no "touch" in
//! the name, so match all three.

use std::io;
use std::os::fd::RawFd;

use crate::evdev;

const ABS_MT_SLOT: u16 = 47;
const ABS_MT_TRACKING_ID: u16 = 57;
const MAX_SLOTS: usize = 16;

pub struct TouchDevice {
    fd: RawFd,
    slots: [bool; MAX_SLOTS],
    cur: usize,
}

impl TouchDevice {
    pub fn open() -> io::Result<Self> {
        let (fd, path, grabbed) = evdev::open_by_name(&["touch", "pt_mt", "cyttsp"])?;
        eprintln!("riddle: touch device {path} opened (grabbed: {grabbed})");
        Ok(Self { fd, slots: [false; MAX_SLOTS], cur: 0 })
    }

    /// Returns true if a 5-finger touch was seen.
    pub fn drain_check_quit(&mut self) -> bool {
        let mut quit = false;
        let mut buf = [0u8; evdev::EV_SIZE * 64];
        loop {
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
            for chunk in buf[..n as usize].chunks_exact(evdev::EV_SIZE) {
                let (etype, code, value) = evdev::parse(chunk);
                if etype == evdev::EV_ABS && code == ABS_MT_SLOT {
                    self.cur = (value.max(0) as usize).min(MAX_SLOTS - 1);
                } else if etype == evdev::EV_ABS && code == ABS_MT_TRACKING_ID {
                    self.slots[self.cur] = value != -1;
                    if self.slots.iter().filter(|&&s| s).count() >= 5 {
                        quit = true;
                    }
                }
            }
        }
        quit
    }
}

impl Drop for TouchDevice {
    fn drop(&mut self) {
        evdev::ungrab(self.fd);
        unsafe {
            libc::close(self.fd);
        }
    }
}
