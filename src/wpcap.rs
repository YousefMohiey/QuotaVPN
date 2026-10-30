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
    pub ts_sec: i64,
    pub ts_usec: i64,
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
                return None;
            }

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
        let wp = WPCAP.with(|w| w.borrow().as_ref().map(|x| x as *const Wpcap));
        let Some(wp) = wp else { return false };
        let f = unsafe { (*wp).sendpacket };
        unsafe { f(self.handle, frame.as_ptr(), frame.len() as c_int) == 0 }
    }

    /// Wait for one frame, up to the open timeout.
    pub fn next(&self) -> Option<Vec<u8>> {
        let wp = WPCAP.with(|w| w.borrow().as_ref().map(|x| x as *const Wpcap))?;
        let f = unsafe { (*wp).next_ex };
        let mut hdr: *mut pcap_pkthdr = std::ptr::null_mut();
        let mut data: *const c_uchar = std::ptr::null();
        let rc = unsafe { f(self.handle, &mut hdr, &mut data) };
        if rc != 1 || data.is_null() || hdr.is_null() {
            return None;
        }
        let len = unsafe { (*hdr).caplen } as usize;
        Some(unsafe { std::slice::from_raw_parts(data, len) }.to_vec())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(wp) = WPCAP.with(|w| w.borrow().as_ref().map(|x| x as *const Wpcap)) {
            unsafe { ((*wp).close)(self.handle) };
        }
    }
}

thread_local! {
    static WPCAP: std::cell::RefCell<Option<Wpcap>> = const { std::cell::RefCell::new(None) };
}

/// Load the driver once for this thread and report whether it is usable.
pub fn ready() -> bool {
    WPCAP.with(|w| {
        if w.borrow().is_some() {
            return true;
        }
        match Wpcap::load() {
            Some(lib) => {
                *w.borrow_mut() = Some(lib);
                true
            }
            None => false,
        }
    })
}

/// Run something with the loaded driver.
pub fn with<R>(f: impl FnOnce(&Wpcap) -> R) -> Option<R> {
    if !ready() {
        return None;
    }
    WPCAP.with(|w| w.borrow().as_ref().map(f))
}
