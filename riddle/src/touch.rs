//! Raw touch input for takeover mode: quit gestures — a 5-finger tap, or
//! (rM2) a swipe down from the top edge, matching the AppLoad close gesture
//! muscle memory from windowed apps.

use std::io;
use std::os::fd::RawFd;

use crate::pen::EV_SIZE;

const EV_SYN: u16 = 0;
const SYN_REPORT: u16 = 0;
const EV_ABS: u16 = 3;
const ABS_MT_SLOT: u16 = 47;
const ABS_MT_POSITION_X: u16 = 53;
const ABS_MT_POSITION_Y: u16 = 54;
const ABS_MT_TRACKING_ID: u16 = 57;
const EVIOCGRAB: libc::c_ulong = 0x40044590;
const MAX_SLOTS: usize = 16;

// rM2 touch panel (pt_mt/cyttsp5): raw X matches the screen, raw Y is
// INVERTED (raw_y = 1871 - screen_y) — so the top edge is raw_y near max
// and a downward swipe is raw_y decreasing.
#[cfg(feature = "rm2")]
const TOUCH_MAX_Y: i32 = 1871;
/// A close-swipe must start within this band of the top edge…
#[cfg(feature = "rm2")]
const TOP_BAND: i32 = 48;
/// …travel at least this far down…
#[cfg(feature = "rm2")]
const SWIPE_TRAVEL: i32 = 300;

pub struct TouchDevice {
    fd: RawFd,
    slots: [bool; MAX_SLOTS],
    cur: usize,
    // Per-slot live position and gesture start (MT protocol B).
    pos: [(i32, i32); MAX_SLOTS],
    start: [Option<(i32, i32)>; MAX_SLOTS],
}

impl TouchDevice {
    pub fn open() -> io::Result<Self> {
        for i in 0..8 {
            let name_path = format!("/sys/class/input/event{i}/device/name");
            if let Ok(name) = std::fs::read_to_string(&name_path) {
                let name = name.to_lowercase();
                // "touch" on the Paper Pro, "pt_mt" (cyttsp5) on the rM2.
                if name.contains("touch") || name.contains("pt_mt") || name.contains("cyttsp5") {
                    let path = std::ffi::CString::new(format!("/dev/input/event{i}")).unwrap();
                    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
                    if fd < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    unsafe { libc::ioctl(fd, EVIOCGRAB as _, 1i32) };
                    return Ok(Self {
                        fd,
                        slots: [false; MAX_SLOTS],
                        cur: 0,
                        pos: [(0, 0); MAX_SLOTS],
                        start: [None; MAX_SLOTS],
                    });
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "no touch device"))
    }

    /// Returns true if a quit gesture was seen: 5 fingers down at once, or
    /// (rM2) a single contact that started at the top edge and swiped down.
    pub fn drain_check_quit(&mut self) -> bool {
        let mut quit = false;
        let mut buf = [0u8; EV_SIZE * 64];
        loop {
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
            for chunk in buf[..n as usize].chunks_exact(EV_SIZE) {
                let etype = u16::from_le_bytes(chunk[EV_SIZE - 8..EV_SIZE - 6].try_into().unwrap());
                let code = u16::from_le_bytes(chunk[EV_SIZE - 6..EV_SIZE - 4].try_into().unwrap());
                let value = i32::from_le_bytes(chunk[EV_SIZE - 4..].try_into().unwrap());
                if etype == EV_ABS && code == ABS_MT_SLOT {
                    self.cur = (value.max(0) as usize).min(MAX_SLOTS - 1);
                } else if etype == EV_ABS && code == ABS_MT_TRACKING_ID {
                    self.slots[self.cur] = value != -1;
                    self.start[self.cur] = None; // (re)armed on next position
                    if self.slots.iter().filter(|&&s| s).count() >= 5 {
                        quit = true;
                    }
                } else if etype == EV_ABS && code == ABS_MT_POSITION_X {
                    self.pos[self.cur].0 = value;
                } else if etype == EV_ABS && code == ABS_MT_POSITION_Y {
                    self.pos[self.cur].1 = value;
                } else if etype == EV_SYN && code == SYN_REPORT {
                    let s = self.cur;
                    if self.slots[s] {
                        match self.start[s] {
                            None => self.start[s] = Some(self.pos[s]),
                            Some((sx, sy)) => {
                                if top_swipe_down(sx, sy, self.pos[s].0, self.pos[s].1) {
                                    quit = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        quit
    }
}

/// rM2: contact began in the top-edge band (raw Y near max, since raw Y is
/// inverted) and traveled down the screen (raw Y decreasing), mostly
/// vertically — the AppLoad-style close swipe.
#[cfg(feature = "rm2")]
fn top_swipe_down(sx: i32, sy: i32, x: i32, y: i32) -> bool {
    let dy = sy - y; // downward screen travel = raw Y decrease
    let dx = (x - sx).abs();
    sy >= TOUCH_MAX_Y - TOP_BAND && dy >= SWIPE_TRAVEL && dy >= 2 * dx
}

/// Paper Pro touch geometry is different; only the 5-finger quit is wired.
#[cfg(not(feature = "rm2"))]
fn top_swipe_down(_sx: i32, _sy: i32, _x: i32, _y: i32) -> bool {
    false
}

impl Drop for TouchDevice {
    fn drop(&mut self) {
        unsafe {
            libc::ioctl(self.fd, EVIOCGRAB as _, 0i32);
            libc::close(self.fd);
        }
    }
}
