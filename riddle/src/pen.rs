//! Raw evdev pen input: the full digitizer, bypassing Qt's filtered view.
//! Gives us full-resolution pressure, hover, and the eraser tip
//! (BTN_TOOL_RUBBER), at the hardware event rate.
//!
//! The device is grabbed (EVIOCGRAB) while the diary is open so xochitl
//! doesn't also react to the pen; released automatically on close/exit.
//!
//! Digitizer profiles (picked at compile time with the panel geometry):
//!  - Paper Pro (aarch64): "Elan marker input", axes already aligned with the
//!    framebuffer, so mapping is a plain scale.
//!  - reMarkable 2 (armv7): "Wacom I2C Digitizer", mounted rotated 90° with
//!    its origin at the portrait bottom-left: raw X runs up the long screen
//!    axis, raw Y runs across the short one. Screen x comes from raw Y, and
//!    screen y from the *inverted* raw X (same transform libremarkable uses).

use std::io;
use std::os::fd::RawFd;

use crate::evdev;
use crate::fb::{SCREEN_H, SCREEN_W};

pub const MAX_PRESSURE: i32 = 4096;

const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;
const ABS_PRESSURE: u16 = 24;
const BTN_TOOL_PEN: u16 = 320;
const BTN_TOOL_RUBBER: u16 = 321;
const BTN_TOUCH: u16 = 330;

#[cfg(target_arch = "arm")]
mod digi {
    /// reMarkable 2: "Wacom I2C Digitizer".
    pub const NAMES: &[&str] = &["wacom"];
    const RAW_MAX_X: i32 = 20966;
    const RAW_MAX_Y: i32 = 15725;

    #[inline]
    pub fn map(raw_x: i32, raw_y: i32) -> (i32, i32) {
        use crate::fb::{SCREEN_H, SCREEN_W};
        (
            raw_y * (SCREEN_W as i32 - 1) / RAW_MAX_Y,
            (RAW_MAX_X - raw_x) * (SCREEN_H as i32 - 1) / RAW_MAX_X,
        )
    }
}

#[cfg(not(target_arch = "arm"))]
mod digi {
    /// Paper Pro: "Elan marker input".
    pub const NAMES: &[&str] = &["marker"];
    const RAW_MAX_X: i32 = 11180;
    const RAW_MAX_Y: i32 = 15340;

    #[inline]
    pub fn map(raw_x: i32, raw_y: i32) -> (i32, i32) {
        use crate::fb::{SCREEN_H, SCREEN_W};
        (
            raw_x * (SCREEN_W as i32 - 1) / RAW_MAX_X,
            raw_y * (SCREEN_H as i32 - 1) / RAW_MAX_Y,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Eraser,
}

#[derive(Debug, Clone, Copy)]
pub struct PenSample {
    /// Screen coordinates.
    pub x: i32,
    pub y: i32,
    /// 0..4096
    pub pressure: i32,
    pub tool: Tool,
    pub touching: bool,
}

pub struct PenDevice {
    fd: RawFd,
    // Accumulated state between SYN_REPORTs.
    raw_x: i32,
    raw_y: i32,
    pressure: i32,
    tool: Tool,
    touching: bool,
    dirty: bool,
}

impl PenDevice {
    /// Find and grab the stylus input device.
    pub fn open() -> io::Result<Self> {
        let (fd, path, grabbed) = evdev::open_by_name(digi::NAMES)?;
        if !grabbed {
            eprintln!("riddle: warning: EVIOCGRAB failed — xochitl will also see the pen");
        }
        eprintln!("riddle: pen device {path} opened (grabbed: {grabbed})");
        Ok(Self {
            fd,
            raw_x: 0,
            raw_y: 0,
            pressure: 0,
            tool: Tool::Pen,
            touching: false,
            dirty: false,
        })
    }

    pub fn raw_fd(&self) -> RawFd {
        self.fd
    }

    /// Drain all pending events; returns one sample per SYN_REPORT frame
    /// that changed state.
    pub fn drain(&mut self) -> Vec<PenSample> {
        let mut out = Vec::new();
        let mut buf = [0u8; evdev::EV_SIZE * 64];
        loop {
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
            for chunk in buf[..n as usize].chunks_exact(evdev::EV_SIZE) {
                let (etype, code, value) = evdev::parse(chunk);
                match (etype, code) {
                    (evdev::EV_ABS, ABS_X) => {
                        self.raw_x = value;
                        self.dirty = true;
                    }
                    (evdev::EV_ABS, ABS_Y) => {
                        self.raw_y = value;
                        self.dirty = true;
                    }
                    (evdev::EV_ABS, ABS_PRESSURE) => {
                        self.pressure = value;
                        self.dirty = true;
                    }
                    (evdev::EV_KEY, BTN_TOOL_PEN) if value == 1 => {
                        self.tool = Tool::Pen;
                    }
                    (evdev::EV_KEY, BTN_TOOL_RUBBER) => {
                        self.tool = if value == 1 { Tool::Eraser } else { Tool::Pen };
                    }
                    (evdev::EV_KEY, BTN_TOUCH) => {
                        self.touching = value == 1;
                        self.dirty = true;
                    }
                    (evdev::EV_SYN, evdev::SYN_REPORT) => {
                        if self.dirty {
                            self.dirty = false;
                            let (x, y) = digi::map(self.raw_x, self.raw_y);
                            out.push(PenSample {
                                x: x.clamp(0, SCREEN_W as i32 - 1),
                                y: y.clamp(0, SCREEN_H as i32 - 1),
                                pressure: self.pressure,
                                tool: self.tool,
                                touching: self.touching,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }
}

impl Drop for PenDevice {
    fn drop(&mut self) {
        evdev::ungrab(self.fd);
        unsafe {
            libc::close(self.fd);
        }
    }
}
