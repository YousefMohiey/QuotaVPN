//! The packet driver, loaded at runtime.
//!
//! The `pcap` crate on crates.io does not build against this toolchain, and a
//! build-time SDK dependency would stop anyone from compiling the project
//! without downloading Npcap's SDK first. Calling wpcap.dll directly fixes both:
//! nothing is linked at build time, and the app simply reports that the driver
//! is missing when it is not installed.

#![allow(non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uchar, c_uint, c_void, CStr, CString};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub const PCAP_ERRBUF_SIZE: usize = 256;

#[repr(C)]
pub struct pcap_if_t {
    pub next: *mut pcap_if_t,
    pub name: *mut c_char,
    pub description: *mut c_char,
    pub addresses: *mut c_void,
    pub flags: c_uint,
}

#[repr(C)]
pub struct pcap_pkthdr {
    // Windows timeval uses 32-bit longs, so this struct is 16 bytes, not 24.
    // Reading it as 64-bit put caplen at the wrong offset and the length read
    // came back as garbage, which is what crashed the app.
    pub ts_sec: i32,
    pub ts_usec: i32,
    pub caplen: c_uint,
    pub len: c_uint,
}

type FnFindAllDevs = unsafe extern "system" fn(*mut *mut pcap_if_t, *mut c_char) -> c_int;
type FnFreeAllDevs = unsafe extern "system" fn(*mut pcap_if_t);
type FnOpenLive =
    unsafe extern "system" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut c_void;
type FnSendPacket = unsafe extern "system" fn(*mut c_void, *const c_uchar, c_int) -> c_int;
type FnNextEx = unsafe extern "system" fn(*mut c_void, *mut *mut pcap_pkthdr, *mut *const c_uchar) -> c_int;
type FnClose = unsafe extern "system" fn(*mut c_void);
type FnGetErr = unsafe extern "system" fn(*mut c_void) -> *const c_char;

/// Same note, callable from other modules.
pub fn note_public(line: &str) {
    note(line);
}

/// Load failures leave a note: a driver that is present but will not load is
/// indistinguishable from a missing one otherwise.
fn note(line: &str) {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::from("."));
    let dir = std::path::Path::new(&base).join("quotavpn");
    let _ = std::fs::create_dir_all(&dir);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("wpcap.log"))
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{stamp} {line}")
        });
}

pub struct Wpcap {
    _lib: *mut c_void,
    pub findalldevs: FnFindAllDevs,
    pub freealldevs: FnFreeAllDevs,
    pub open_live: FnOpenLive,
    pub sendpacket: FnSendPacket,
    pub next_ex: FnNextEx,
    pub close: FnClose,
    pub geterr: FnGetErr,
}

/// One interface the driver can open.
pub struct Iface {
    pub name: String,
    pub description: String,
}

impl Wpcap {
    /// Load wpcap.dll from the places Npcap installs it, or from PATH.
    pub fn load() -> Option<Wpcap> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

            fn wide(s: &str) -> Vec<u16> {
                std::ffi::OsStr::new(s)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
            }

            let mut handle: *mut c_void = std::ptr::null_mut();
            for path in [
                r"C:\Windows\System32\Npcap\wpcap.dll",
                r"C:\Windows\System32\wpcap.dll",
                "wpcap.dll",
            ] {
                let h = unsafe { LoadLibraryW(wide(path).as_ptr()) };
                if !h.is_null() {
                    handle = h;
                    break;
                }
            }
            if handle.is_null() {
                note(&format!(
                    "load failed: every path refused, last error {}",
                    std::io::Error::last_os_error()
                ));
                return None;
            }
            note("wpcap loaded");

            unsafe fn sym(handle: *mut c_void, name: &str) -> Option<*const c_void> {
                let c = CString::new(name).ok()?;
                let p = unsafe { GetProcAddress(handle, c.as_ptr() as *const u8) }?;
                Some(p as *const c_void)
            }

            unsafe {
                Some(Wpcap {
                    _lib: handle,
                    findalldevs: std::mem::transmute(sym(handle, "pcap_findalldevs")?),
                    freealldevs: std::mem::transmute(sym(handle, "pcap_freealldevs")?),
                    open_live: std::mem::transmute(sym(handle, "pcap_open_live")?),
                    sendpacket: std::mem::transmute(sym(handle, "pcap_sendpacket")?),
                    next_ex: std::mem::transmute(sym(handle, "pcap_next_ex")?),
                    close: std::mem::transmute(sym(handle, "pcap_close")?),
                    geterr: std::mem::transmute(sym(handle, "pcap_geterr")?),
                })
            }
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Every interface with a name and a human description.
    pub fn devices(&self) -> Vec<Iface> {
        let mut list: *mut pcap_if_t = std::ptr::null_mut();
        let mut err = [0 as c_char; PCAP_ERRBUF_SIZE];
        let rc = unsafe { (self.findalldevs)(&mut list, err.as_mut_ptr()) };
        if rc != 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut cur = list;
        while !cur.is_null() {
            let node = unsafe { &*cur };
            let name = if node.name.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(node.name) }.to_string_lossy().to_string()
            };
            let desc = if node.description.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(node.description) }.to_string_lossy().to_string()
            };
            if !name.is_empty() {
                out.push(Iface { name, description: desc });
            }
            cur = node.next;
        }
        unsafe { (self.freealldevs)(list) };
        out
    }

    /// Open one interface for capture and injection.
    pub fn open(&self, device: &str) -> Option<Capture> {
        let c = CString::new(device).ok()?;
        let mut err = [0 as c_char; PCAP_ERRBUF_SIZE];
        let h = unsafe { (self.open_live)(c.as_ptr(), 65536, 1, 20, err.as_mut_ptr()) };
        if h.is_null() {
            return None;
        }
        Some(Capture { handle: h })
    }
}

pub struct Capture {
    handle: *mut c_void,
}

impl Capture {
    /// Put one frame on the wire. This is how an ARP reply is sent.
    pub fn send(&self, frame: &[u8]) -> bool {
        // Safety: the handle belongs to this capture and the buffer outlives the call.
        let Some(wp) = loaded() else { return false };
        let f = wp.sendpacket;
        unsafe { f(self.handle, frame.as_ptr(), frame.len() as c_int) == 0 }
    }

    /// Wait for one frame, up to the open timeout.
    pub fn next(&self) -> Option<Vec<u8>> {
        let wp = loaded()?;
        let f = wp.next_ex;
        let mut hdr: *mut pcap_pkthdr = std::ptr::null_mut();
        let mut data: *const c_uchar = std::ptr::null();
        let rc = unsafe { f(self.handle, &mut hdr, &mut data) };
        if rc != 1 || data.is_null() || hdr.is_null() {
            return None;
        }
        let len = (unsafe { (*hdr).caplen } as usize).min(65_535);
        if len == 0 {
            return None;
        }
        Some(unsafe { std::slice::from_raw_parts(data, len) }.to_vec())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(wp) = loaded() {
            unsafe { (wp.close)(self.handle) };
        }
    }
}

/// One handle for the whole process. It used to be per thread, which meant a
/// worker thread could report the driver missing while another thread had it
/// loaded, and that is exactly how a working driver looked broken.
static WPCAP: std::sync::OnceLock<Option<Wpcap>> = std::sync::OnceLock::new();

// The handle and the function pointers are process-wide facts; sharing them
// across threads is safe, calling them is what needs care.
unsafe impl Send for Wpcap {}
unsafe impl Sync for Wpcap {}

fn loaded() -> Option<&'static Wpcap> {
    WPCAP.get_or_init(Wpcap::load).as_ref()
}

/// Load the driver once and report whether it is usable.
pub fn ready() -> bool {
    loaded().is_some()
}

/// Run something with the loaded driver.
pub fn with<R>(f: impl FnOnce(&Wpcap) -> R) -> Option<R> {
    loaded().map(f)
}
