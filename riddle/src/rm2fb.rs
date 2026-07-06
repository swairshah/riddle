//! rm2fb client: the reMarkable 2 takeover display backend.
//!
//! The rM2 has no vendor display library to interpose (its e-ink TCON lives
//! inside the xochitl binary), so the community equivalent of quill is
//! rm2fb (ddvk/remarkable2-framebuffer): a server process that hosts the
//! extracted software TCON and exposes
//!   - a shared-memory framebuffer at /dev/shm/swtfb.01
//!     (1404 x 1872, RGB565, 2 bytes/px), and
//!   - a SysV message queue (key 0x2257c) accepting mxcfb-style update
//!     requests (mtype UPDATE_t = 2).
//!
//! We are a plain client: map the buffer, draw, msgsnd update rects. The
//! rm2fb server must be running (on a Toltec/xovi rM2 it already is — it is
//! the thing xochitl itself draws through).

use std::io;

pub const WIDTH: usize = 1404;
pub const HEIGHT: usize = 1872;
pub const BPP: usize = 2;

const SHM_PATH: &str = "/dev/shm/swtfb.01\0";
const MSGQ_KEY: libc::key_t = 0x2257c;

/// swtfb_update mtype for an mxcfb_update_data payload.
const UPDATE_T: libc::c_long = 2;

// mxcfb waveform / update modes (i.MX EPDC convention, as rm2fb expects).
pub const WAVEFORM_DU: u32 = 1; // 1-bit, fastest — live ink
pub const WAVEFORM_GC16: u32 = 2; // 16-level, highest quality
pub const WAVEFORM_GL16: u32 = 3; // 16-level, no flash — text/animation
pub const UPDATE_MODE_PARTIAL: u32 = 0;
pub const UPDATE_MODE_FULL: u32 = 1;

const TEMP_USE_REMARKABLE: i32 = 0x0018;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MxcfbRect {
    top: u32,
    left: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MxcfbAltBufferData {
    phys_addr: u32,
    width: u32,
    height: u32,
    alt_update_region: MxcfbRect,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MxcfbUpdateData {
    update_region: MxcfbRect,
    waveform_mode: u32,
    update_mode: u32,
    update_marker: u32,
    temp: i32,
    flags: u32,
    dither_mode: i32,
    quant_bit: i32,
    alt_buffer_data: MxcfbAltBufferData,
}

/// Wire message: `struct swtfb_update { long mtype; union { ... } mdata; }`.
/// msgsnd's size argument counts the payload only (sizeof mdata.update).
#[repr(C)]
struct SwtfbMsg {
    mtype: libc::c_long,
    update: MxcfbUpdateData,
}

// The server reads exactly this layout off the queue; every field is
// 4-byte-aligned so the struct must pack to 72 bytes with no padding.
const _: () = assert!(std::mem::size_of::<MxcfbUpdateData>() == 72);
const _: () = assert!(std::mem::size_of::<SwtfbMsg>() == 76);

pub struct Rm2fb {
    shm: *mut u8,
    shm_len: usize,
    msqid: libc::c_int,
    marker: std::cell::Cell<u32>,
}

// Single-threaded writer over a long-lived mapping.
unsafe impl Send for Rm2fb {}

impl Rm2fb {
    pub fn open() -> io::Result<Self> {
        let fd = unsafe { libc::open(SHM_PATH.as_ptr() as *const libc::c_char, libc::O_RDWR) };
        if fd < 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "rm2fb framebuffer /dev/shm/swtfb.01 not found — is the rm2fb server running? \
                 (systemctl start rm2fb)",
            ));
        }
        let shm_len = WIDTH * HEIGHT * BPP;
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                shm_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        unsafe { libc::close(fd) };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }

        let msqid = unsafe { libc::msgget(MSGQ_KEY, 0) };
        if msqid < 0 {
            let e = io::Error::last_os_error();
            unsafe { libc::munmap(ptr, shm_len) };
            return Err(io::Error::new(
                e.kind(),
                "rm2fb update queue (key 0x2257c) not found — is the rm2fb server running?",
            ));
        }

        eprintln!("riddle: rm2fb connected ({WIDTH}x{HEIGHT} RGB565, msqid {msqid})");
        Ok(Self { shm: ptr as *mut u8, shm_len, msqid, marker: std::cell::Cell::new(1) })
    }

    pub fn framebuffer(&self) -> (*mut u8, usize) {
        (self.shm, self.shm_len)
    }

    /// Ask the server to push a region to glass.
    pub fn update(&self, x: i32, y: i32, w: i32, h: i32, waveform: u32, mode: u32) {
        let x = x.clamp(0, WIDTH as i32 - 1);
        let y = y.clamp(0, HEIGHT as i32 - 1);
        let w = w.clamp(1, WIDTH as i32 - x);
        let h = h.clamp(1, HEIGHT as i32 - y);
        self.marker.set(self.marker.get().wrapping_add(1));
        let msg = SwtfbMsg {
            mtype: UPDATE_T,
            update: MxcfbUpdateData {
                update_region: MxcfbRect {
                    top: y as u32,
                    left: x as u32,
                    width: w as u32,
                    height: h as u32,
                },
                waveform_mode: waveform,
                update_mode: mode,
                update_marker: self.marker.get(),
                temp: TEMP_USE_REMARKABLE,
                flags: 0,
                ..Default::default()
            },
        };
        let rc = unsafe {
            libc::msgsnd(
                self.msqid,
                &msg as *const SwtfbMsg as *const libc::c_void,
                std::mem::size_of::<MxcfbUpdateData>(),
                0,
            )
        };
        if rc != 0 {
            eprintln!("riddle: rm2fb msgsnd failed: {}", io::Error::last_os_error());
        }
    }
}

impl Drop for Rm2fb {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.shm as *mut libc::c_void, self.shm_len);
        }
    }
}
