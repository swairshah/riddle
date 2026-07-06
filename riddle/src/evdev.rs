//! Raw evdev plumbing shared by the pen, touch, and power-button readers.
//!
//! `struct input_event` starts with a `struct timeval`, whose size follows the
//! userspace ABI: 16 bytes on 64-bit (Paper Pro, aarch64), 8 bytes on 32-bit
//! (reMarkable 2, armv7). type/code/value follow it, so both the record size
//! and the field offsets differ per architecture.

use std::io;
use std::os::fd::RawFd;

pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_ABS: u16 = 3;
pub const SYN_REPORT: u16 = 0;

// _IOW('E', 0x90, int). The ioctl request parameter is c_ulong on glibc but
// c_int on musl, hence the `as _` casts in grab()/ungrab().
const EVIOCGRAB: u32 = 0x40044590;

/// EVIOCGRAB the device; returns true when the grab succeeded.
pub fn grab(fd: RawFd) -> bool {
    unsafe { libc::ioctl(fd, EVIOCGRAB as _, 1i32) == 0 }
}

pub fn ungrab(fd: RawFd) {
    unsafe { libc::ioctl(fd, EVIOCGRAB as _, 0i32) };
}

#[cfg(target_pointer_width = "64")]
pub const EV_SIZE: usize = 24;
#[cfg(target_pointer_width = "64")]
const TYPE_OFF: usize = 16;

#[cfg(target_pointer_width = "32")]
pub const EV_SIZE: usize = 16;
#[cfg(target_pointer_width = "32")]
const TYPE_OFF: usize = 8;

/// Decode one raw input_event record into (type, code, value).
#[inline]
pub fn parse(chunk: &[u8]) -> (u16, u16, i32) {
    (
        u16::from_le_bytes(chunk[TYPE_OFF..TYPE_OFF + 2].try_into().unwrap()),
        u16::from_le_bytes(chunk[TYPE_OFF + 2..TYPE_OFF + 4].try_into().unwrap()),
        i32::from_le_bytes(chunk[TYPE_OFF + 4..TYPE_OFF + 8].try_into().unwrap()),
    )
}

/// Open (non-blocking) and EVIOCGRAB the first /dev/input/eventN whose device
/// name contains one of `needles` (case-insensitive). Returns (fd, path,
/// grabbed). A failed grab is reported to the caller, not fatal.
pub fn open_by_name(needles: &[&str]) -> io::Result<(RawFd, String, bool)> {
    for i in 0..8 {
        let name = std::fs::read_to_string(format!("/sys/class/input/event{i}/device/name"))
            .unwrap_or_default()
            .to_lowercase();
        if !needles.iter().any(|n| name.contains(n)) {
            continue;
        }
        let path = format!("/dev/input/event{i}");
        let cpath = std::ffi::CString::new(path.clone()).unwrap();
        let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let grabbed = grab(fd);
        return Ok((fd, path, grabbed));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no input device matching {needles:?}"),
    ))
}
