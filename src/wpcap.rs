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
            use windows_sys::Win32::System::LibraryLoader::{
                GetProcAddress, LoadLibraryExW, LoadLibraryW, LOAD_WITH_ALTERED_SEARCH_PATH,
            };

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
                // Npcap without WinPcap-compatible mode keeps Packet.dll beside
                // wpcap.dll in System32\Npcap. A plain LoadLibrary on the full
                // path does not search that folder for the dependency, so the
                // driver reads as missing even when it is installed.
                let h = unsafe {
                    if path.contains('\\') {
                        LoadLibraryExW(wide(path).as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH)
                    } else {
                        LoadLibraryW(wide(path).as_ptr())
                    }
                };
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
        self.open_mode(device, true, 20)
    }

    /// Open with an explicit promiscuous mode and read timeout.
    ///
    /// The relay opens the adapter non-promiscuous. It only ever needs frames
    /// addressed to this machine, and promiscuous mode is the setting Npcap's
    /// own guide tells people to turn off when a wireless adapter misbehaves.
    /// The timeout bounds how long a stop request waits for a quiet link.
    pub fn open_mode(&self, device: &str, promiscuous: bool, timeout_ms: i32) -> Option<Capture> {
        let c = CString::new(device).ok()?;
        let mut err = [0 as c_char; PCAP_ERRBUF_SIZE];
        let h = unsafe {
            (self.open_live)(
                c.as_ptr(),
                65536,
                if promiscuous { 1 } else { 0 },
                timeout_ms,
                err.as_mut_ptr(),
            )
        };
        if h.is_null() {
            let text: Vec<u8> = err
                .iter()
                .take_while(|c| **c != 0)
                .map(|c| *c as u8)
                .collect();
            note(&format!(
                "open failed for {device}: {}",
                String::from_utf8_lossy(&text)
            ));
            return None;
        }
        Some(Capture { handle: h })
    }
}

pub struct Capture {
    handle: *mut c_void,
}

/// One read from a capture handle: a frame, the timeout expiring, or the handle
/// failing. The relay has to tell these apart: a timeout is a quiet link, an
/// error is an adapter that will never carry anything again.
pub enum Read {
    Frame(Vec<u8>),
    Timeout,
    Error(String),
}

impl Capture {
    /// Put one frame on the wire. This is how an ARP reply is sent.
    pub fn send(&self, frame: &[u8]) -> bool {
        // Safety: the handle belongs to this capture and the buffer outlives the call.
        let Some(wp) = loaded() else { return false };
        let f = wp.sendpacket;
        unsafe { f(self.handle, frame.as_ptr(), frame.len() as c_int) == 0 }
    }

    /// Wait for one frame, up to the open timeout, and say why there was none.
    pub fn read(&self) -> Read {
        let Some(wp) = loaded() else {
            return Read::Error("the packet driver is no longer loaded".into());
        };
        let f = wp.next_ex;
        let mut hdr: *mut pcap_pkthdr = std::ptr::null_mut();
        let mut data: *const c_uchar = std::ptr::null();
        match unsafe { f(self.handle, &mut hdr, &mut data) } {
            1 => {
                if data.is_null() || hdr.is_null() {
                    return Read::Timeout;
                }
                let len = (unsafe { (*hdr).caplen } as usize).min(65_535);
                if len == 0 {
                    return Read::Timeout;
                }
                Read::Frame(unsafe { std::slice::from_raw_parts(data, len) }.to_vec())
            }
            0 => Read::Timeout,
            _ => {
                let ptr = unsafe { (wp.geterr)(self.handle) };
                let why = if ptr.is_null() {
                    "the adapter stopped delivering frames".to_string()
                } else {
                    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string()
                };
                Read::Error(why)
            }
        }
    }

    /// Wait for one frame, up to the open timeout.
    pub fn next(&self) -> Option<Vec<u8>> {
        match self.read() {
            Read::Frame(f) => Some(f),
            _ => None,
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(wp) = loaded() {
            unsafe { (wp.close)(self.handle) };
        }
    }
}

// ---------------------------------------------------------------- claim backends

/// Putting the 42-byte claim frame on the wire, with or without a driver.
///
/// That frame is the only thing left that needs a raw handle. Watching, capping,
/// blocking and naming all run on the forward pipeline, so this trait is the
/// whole of the driver's remaining job, and a machine without it keeps every
/// feature except claiming a device on a router it does not control.
pub trait ClaimLink {
    /// True when the frame went out, or when there was nothing to send.
    fn send_claim(&self, frame: &[u8]) -> bool;
    /// Whether this backend needed a driver, for the diagnosis output and for
    /// what the screen should offer.
    fn needs_driver(&self) -> bool;
}

/// No driver and nothing to send: the router's DHCP already hands this machine
/// out as the gateway, so a device's packets already arrive here.
pub struct GatewayMode;

impl ClaimLink for GatewayMode {
    /// Nothing can be sent without a capture device, and saying otherwise was a
    /// lie the caller could not see: `claim` reports success, the screen says the
    /// device is routed, and no frame ever left this machine. Whether the router
    /// already points the device here is not something this process can know, so
    /// it answers false and the caller turns that into a plain refusal.
    fn send_claim(&self, _frame: &[u8]) -> bool {
        false
    }
    fn needs_driver(&self) -> bool {
        false
    }
}

/// A raw handle, opened on demand, for the router that still believes it is the
/// gateway itself.
pub struct NpcapClaimFallback {
    cap: Capture,
}

impl NpcapClaimFallback {
    /// Open the wire when the driver is installed. `None` means gateway mode is
    /// the only option left, which is exactly the machine with no driver.
    pub fn open(device: &str) -> Option<NpcapClaimFallback> {
        if device.is_empty() {
            return None;
        }
        Some(NpcapClaimFallback {
            cap: with(|w| w.open(device)).flatten()?,
        })
    }
}

impl ClaimLink for NpcapClaimFallback {
    fn send_claim(&self, frame: &[u8]) -> bool {
        self.cap.send(frame)
    }
    fn needs_driver(&self) -> bool {
        true
    }
}

/// The backend for this wire.
///
/// The driver when it is there: a claim frame is harmless when the device
/// already points at us and necessary when it does not. Gateway mode when it is
/// not, which is correct exactly when the router's DHCP already points devices
/// at this machine.
pub fn claim_link(device: &str) -> Box<dyn ClaimLink> {
    match NpcapClaimFallback::open(device) {
        Some(link) => Box::new(link),
        None => Box::new(GatewayMode),
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
