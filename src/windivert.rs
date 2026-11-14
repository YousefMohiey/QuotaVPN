//! WinDivert, the packet driver this app ships instead of asking for one.
//!
//! Everything the LAN features do happens on packets that are being routed
//! between two interfaces: a device's packet is already on its way through this
//! machine because the device points its gateway at us. That is the
//! `WINDIVERT_LAYER_NETWORK_FORWARD` layer, which sees exactly those packets and
//! nothing else. The PC's own traffic never appears here, which is why the
//! "This PC" row has no reading in this model and why counting here cannot
//! double-count the host.
//!
//! Three properties of the API shape this module.
//!
//! One: the address block is opaque. `WINDIVERT_ADDRESS` is a packed struct with
//! bitfields whose layout differs between releases, and every field in it except
//! the direction is irrelevant to a pass-through loop. It is captured into a
//! buffer that is larger than any version of the struct and handed back
//! unchanged to `WinDivertSend`, so a layout change cannot silently corrupt a
//! packet.
//!
//! Two: nothing here rewrites a packet, so no checksum is ever recomputed. A
//! blocked packet is one that is never sent back. A paced packet is one that is
//! held and then sent back unchanged.
//!
//! Three: `WinDivertRecv` blocks forever on a quiet link, so stopping means
//! `WinDivertShutdown` from the thread that wants it gone. A stop flag alone
//! would leave the worker asleep on the last packet.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_void, CStr};
use std::time::{Duration, Instant};

/// The layer that sees packets being forwarded between interfaces.
pub const LAYER_NETWORK_FORWARD: i32 = 2;

/// Queue length in packets. Long enough to ride out a burst of a few hundred
/// packets around one TCP handshake, short enough that a paced device does not
/// build a second of backlog in the driver's memory.
pub const QUEUE_LEN: u64 = 8192;
/// How long a packet may sit in the queue before the driver gives up on it.
/// This is the backstop for the loop being slower than the link: better a
/// dropped packet, which TCP retransmits, than a growing queue of stale ones.
pub const QUEUE_TIME_MS: u64 = 2000;
const QUEUE_SIZE_MB: u64 = 16;

const PARAM_QUEUE_LEN: u64 = 0;
const PARAM_QUEUE_TIME: u64 = 1;
const PARAM_QUEUE_SIZE: u64 = 2;

const SHUTDOWN_BOTH: u64 = 3;

/// `sizeof(WINDIVERT_ADDRESS)` is 48 in the shipped releases. The buffer is
/// larger on purpose: the driver writes its own struct size and reads its own
/// struct size, so as long as ours is not smaller, a layout change upstream is
/// invisible here. See the module note.
const ADDR_LEN: usize = 64;

/// An address block from the driver, passed through untouched.
#[derive(Clone, Copy)]
pub struct Addr {
    raw: [u8; ADDR_LEN],
}

impl Addr {
    fn zeroed() -> Addr {
        Addr { raw: [0; ADDR_LEN] }
    }
}

impl Default for Addr {
    fn default() -> Self {
        Addr::zeroed()
    }
}

/// One captured packet and the block that says where it came from.
pub struct Packet {
    pub data: Vec<u8>,
    pub addr: Addr,
}

type FnOpen = unsafe extern "system" fn(*const c_char, i32, i16, u64) -> *mut c_void;
type FnRecv =
    unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32, *mut c_void) -> c_int;
type FnSend =
    unsafe extern "system" fn(*mut c_void, *const c_void, u32, *mut u32, *const c_void) -> c_int;
type FnClose = unsafe extern "system" fn(*mut c_void) -> c_int;
type FnShutdown = unsafe extern "system" fn(*mut c_void, u64) -> c_int;
type FnSetParam = unsafe extern "system" fn(*mut c_void, u64, u64) -> c_int;

/// A loaded `WinDivert.dll` and the four calls the loops need.
pub struct WinDivert {
    _lib: libloading::Library,
    open: FnOpen,
    recv: FnRecv,
    send: FnSend,
    close: FnClose,
    shutdown: FnShutdown,
    set_param: FnSetParam,
}

// The library handle and the function pointers are process-wide facts. The
// handle they operate on is per-open and owned by `Divert`.
unsafe impl Send for WinDivert {}
unsafe impl Sync for WinDivert {}

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
        .open(dir.join("windivert.log"))
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{stamp} {line}")
        });
}

/// Where the driver may be, in the order to try.
///
/// First next to the executable, which is where the bundle puts it and where
/// WinDivert also looks for its `.sys`. Then the source tree's `res/windivert`,
/// so a `cargo tauri dev` run finds it without anyone copying a binary into
/// `target/debug` by hand. The `.sys` travels with the DLL, so loading the pair
/// out of the tree works exactly the same as loading it from the install.
#[cfg(windows)]
fn dll_candidates() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("WinDivert.dll"));
            // .../target/<profile>/app.exe, so the crate root is one to three
            // levels up depending on whether a target triple is in the path.
            let mut p = dir.to_path_buf();
            for _ in 0..4 {
                let Some(parent) = p.parent().map(|d| d.to_path_buf()) else {
                    break;
                };
                p = parent;
                out.push(p.join("res").join("windivert").join("WinDivert.dll"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("WinDivert.dll"));
        out.push(cwd.join("res").join("windivert").join("WinDivert.dll"));
        // `cargo tauri dev` and `tauri build` both run from desktop/src-tauri.
        out.push(
            cwd.join("..")
                .join("..")
                .join("res")
                .join("windivert")
                .join("WinDivert.dll"),
        );
    }
    // Last: whatever the loader's own search order finds.
    out.push(std::path::PathBuf::from("WinDivert.dll"));
    let mut seen: Vec<String> = Vec::new();
    out.retain(|p| {
        let key = p.to_string_lossy().to_lowercase();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    out
}

impl WinDivert {
    /// Load the driver's DLL. Bundled next to the executable, so the first
    /// candidate is the module's own directory, which the loader searches first
    /// for a bare name.
    pub fn load() -> Result<WinDivert, String> {
        #[cfg(not(windows))]
        {
            Err("WinDivert is Windows only".to_string())
        }
        #[cfg(windows)]
        {
            // Safety: loading a DLL runs its initialisers, which is the point.
            let mut lib = None;
            let mut last = String::from("no candidate");
            for path in dll_candidates() {
                match unsafe { libloading::Library::new(&path) } {
                    Ok(l) => {
                        note(&format!("loaded from {}", path.display()));
                        lib = Some(l);
                        break;
                    }
                    Err(e) => {
                        note(&format!("failed for {}: {e}", path.display()));
                        last = format!("{}: {e}", path.display());
                    }
                }
            }
            let lib = lib.ok_or_else(|| format!("WinDivert.dll was not found ({last})"))?;
            unsafe {
                let open = *lib
                    .get::<FnOpen>(b"WinDivertOpen\0")
                    .map_err(|e| format!("WinDivertOpen: {e}"))?;
                let recv = *lib
                    .get::<FnRecv>(b"WinDivertRecv\0")
                    .map_err(|e| format!("WinDivertRecv: {e}"))?;
                let send = *lib
                    .get::<FnSend>(b"WinDivertSend\0")
                    .map_err(|e| format!("WinDivertSend: {e}"))?;
                let close = *lib
                    .get::<FnClose>(b"WinDivertClose\0")
                    .map_err(|e| format!("WinDivertClose: {e}"))?;
                let shutdown = *lib
                    .get::<FnShutdown>(b"WinDivertShutdown\0")
                    .map_err(|e| format!("WinDivertShutdown: {e}"))?;
                let set_param = *lib
                    .get::<FnSetParam>(b"WinDivertSetParam\0")
                    .map_err(|e| format!("WinDivertSetParam: {e}"))?;
                note("windivert loaded");
                Ok(WinDivert {
                    _lib: lib,
                    open,
                    recv,
                    send,
                    close,
                    shutdown,
                    set_param,
                })
            }
        }
    }
}

static DRIVER: std::sync::OnceLock<Result<WinDivert, String>> = std::sync::OnceLock::new();

/// The driver, loaded once. The `Result` is cached too: a machine without the
/// DLL must not retry a failing `LoadLibraryW` on every poll.
pub fn driver() -> Result<&'static WinDivert, String> {
    DRIVER.get_or_init(WinDivert::load).as_ref().map_err(|e| e.clone())
}

/// Whether the driver is usable, without surfacing the reason.
pub fn ready() -> bool {
    driver().is_ok()
}

/// An open handle on the forward layer.
pub struct Divert {
    dll: &'static WinDivert,
    handle: *mut c_void,
}

// The handle is a kernel object; using it from one thread at a time is what the
// loops do, and stopping happens from another thread through `shutdown`.
unsafe impl Send for Divert {}
unsafe impl Sync for Divert {}

impl Divert {
    /// Open the forward layer with a filter.
    ///
    /// `filter` is WinDivert's own language, not ours: see [`forward_filter`].
    pub fn open_forward(filter: &str) -> Result<Divert, String> {
        let dll = driver()?;
        let c = std::ffi::CString::new(filter).map_err(|_| "filter has a nul".to_string())?;
        // Safety: a valid filter string and a null-free CString.
        let handle = unsafe { (dll.open)(c.as_ptr(), LAYER_NETWORK_FORWARD, 0, 0) };
        if handle.is_null() {
            let err = std::io::Error::last_os_error();
            note(&format!("open failed for {filter}: {err}"));
            // The two failures worth naming, because they are the two that happen.
            return Err(match err.raw_os_error() {
                Some(5) => "the packet driver needs this app to run elevated".to_string(),
                Some(2) => "the packet driver is not installed next to the app".to_string(),
                _ => format!("could not open the packet driver: {err}"),
            });
        }
        let d = Divert { dll, handle };
        // Queue tuning is advisory: a driver that refuses it still works. The
        // defaults are 128 MB of queue time, which is an outage in slow motion.
        unsafe {
            (d.dll.set_param)(d.handle, PARAM_QUEUE_LEN, QUEUE_LEN);
            (d.dll.set_param)(d.handle, PARAM_QUEUE_TIME, QUEUE_TIME_MS);
            (d.dll.set_param)(d.handle, PARAM_QUEUE_SIZE, QUEUE_SIZE_MB * 1024 * 1024);
        }
        Ok(d)
    }

    /// Block until one forwarded packet arrives.
    ///
    /// `Ok(None)` means the packet was larger than the buffer, which the driver
    /// reports as a successful receive with a short read. Dropping it is correct:
    /// the caller asked for a size it does not want to handle.
    pub fn recv(&self, buf: &mut [u8]) -> Result<Option<Packet>, String> {
        let mut len: u32 = 0;
        let mut addr = Addr::zeroed();
        let ok = unsafe {
            (self.dll.recv)(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
                &mut len,
                addr.raw.as_mut_ptr() as *mut c_void,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("recv: {err}"));
        }
        let len = (len as usize).min(buf.len());
        if len == 0 {
            return Ok(None);
        }
        Ok(Some(Packet { data: buf[..len].to_vec(), addr }))
    }

    /// Put a packet back on its way. This is both the forward and the un-block:
    /// a packet that is never sent is a packet that was dropped.
    pub fn send(&self, data: &[u8], addr: &Addr) -> Result<(), String> {
        let mut sent: u32 = 0;
        let ok = unsafe {
            (self.dll.send)(
                self.handle,
                data.as_ptr() as *const c_void,
                data.len() as u32,
                &mut sent,
                addr.raw.as_ptr() as *const c_void,
            )
        };
        if ok == 0 {
            return Err(format!("send: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    /// Wake a thread parked in [`Divert::recv`]. Called from the thread that
    /// wants the loop to end, before joining it.
    pub fn shutdown(&self) {
        unsafe {
            (self.dll.shutdown)(self.handle, SHUTDOWN_BOTH);
        }
    }
}

impl Drop for Divert {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.dll.close)(self.handle);
            }
        }
    }
}

/// A filter for the devices a session cares about.
///
/// WinDivert's language has no "in a list" operator, so the addresses are spelled
/// out. `ip` keeps the filter off ARP and everything else that is not IPv4.
pub fn forward_filter(ips: &[String]) -> String {
    if ips.is_empty() {
        // Nothing to match means nothing to see. Opening the layer with a filter
        // that can never be true is honest and cheap: no packet is copied up.
        return "ip and ip.SrcAddr == 0.0.0.0 and ip.DstAddr == 0.0.0.0".to_string();
    }
    let terms: Vec<String> = ips
        .iter()
        .flat_map(|ip| {
            [
                format!("ip.SrcAddr == {ip}"),
                format!("ip.DstAddr == {ip}"),
            ]
        })
        .collect();
    format!("ip and ({})", terms.join(" or "))
}

// ---------------------------------------------------------------- packet parsing

/// The fields of an IPv4 header the loops need.
pub struct Ipv4<'a> {
    pub src: [u8; 4],
    pub dst: [u8; 4],
    pub protocol: u8,
    pub payload: &'a [u8],
}

impl<'a> Ipv4<'a> {
    /// Parse an IPv4 packet. The forward layer hands over the network header, so
    /// there is no Ethernet header to skip.
    pub fn parse(data: &'a [u8]) -> Option<Ipv4<'a>> {
        if data.len() < 20 {
            return None;
        }
        if data[0] >> 4 != 4 {
            return None;
        }
        let ihl = (data[0] & 0x0f) as usize * 4;
        if ihl < 20 || data.len() < ihl {
            return None;
        }
        // A fragment other than the first has no transport header, and it is the
        // transport header the callers read. Skipping it loses a byte or two of
        // accounting and cannot misread a port.
        let frag_off = u16::from_be_bytes([data[6], data[7]]);
        if frag_off & 0x1fff != 0 {
            return None;
        }
        let total = u16::from_be_bytes([data[2], data[3]]) as usize;
        let end = if total >= ihl && total <= data.len() {
            total
        } else {
            data.len()
        };
        Some(Ipv4 {
            src: [data[12], data[13], data[14], data[15]],
            dst: [data[16], data[17], data[18], data[19]],
            protocol: data[9],
            payload: &data[ihl..end],
        })
    }
}

/// Source and destination ports of a transport segment, when it has them.
pub fn ports(protocol: u8, payload: &[u8]) -> Option<(u16, u16)> {
    match protocol {
        6 | 17 => {
            if payload.len() < 4 {
                return None;
            }
            Some((
                u16::from_be_bytes([payload[0], payload[1]]),
                u16::from_be_bytes([payload[2], payload[3]]),
            ))
        }
        _ => None,
    }
}

/// The bytes after the transport header, which is where a DNS or TLS payload
/// starts. TCP's data offset is in the header, so this cannot assume 20.
pub fn payload_after_transport(protocol: u8, payload: &[u8]) -> Option<&[u8]> {
    match protocol {
        17 => {
            if payload.len() < 8 {
                return None;
            }
            Some(&payload[8..])
        }
        6 => {
            if payload.len() < 20 {
                return None;
            }
            let off = ((payload[12] >> 4) as usize) * 4;
            if off < 20 || payload.len() < off {
                return None;
            }
            Some(&payload[off..])
        }
        _ => None,
    }
}

/// `a.b.c.d` for a bare address.
pub fn ip_text(ip: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

fn read_name(buf: &[u8], mut i: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut hops = 0;
    let mut first_end = None;
    loop {
        let len = *buf.get(i)? as usize;
        if len == 0 {
            i += 1;
            break;
        }
        if len & 0xc0 == 0xc0 {
            let ptr = (((len & 0x3f) as usize) << 8) | *buf.get(i + 1)? as usize;
            if first_end.is_none() {
                first_end = Some(i + 2);
            }
            hops += 1;
            if hops > 8 {
                break;
            }
            i = ptr;
            continue;
        }
        let label = buf.get(i + 1..i + 1 + len)?;
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&String::from_utf8_lossy(label));
        i += 1 + len;
    }
    Some((out, first_end.unwrap_or(i)))
}

fn skip_name(buf: &[u8], mut i: usize) -> Option<usize> {
    loop {
        let len = *buf.get(i)? as usize;
        if len == 0 {
            return Some(i + 1);
        }
        if len & 0xc0 == 0xc0 {
            return Some(i + 2);
        }
        i += 1 + len;
    }
}

/// Names in a DNS response, as (name, address) for the address records.
///
/// Both directions of every destination map start here: a response is the one
/// place where a name and the address the device is talking to appear together.
pub fn dns_answers(buf: &[u8]) -> Vec<(String, [u8; 4])> {
    let mut out = Vec::new();
    if buf.len() < 12 {
        return out;
    }
    // Responses only: QR bit set.
    if buf[2] & 0x80 == 0 {
        return out;
    }
    let qd = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let an = u16::from_be_bytes([buf[6], buf[7]]) as usize;
    let mut i = 12;
    for _ in 0..qd {
        let Some(next) = skip_name(buf, i) else { return out };
        i = next + 4;
    }
    for _ in 0..an {
        let Some((name, next)) = read_name(buf, i) else { return out };
        i = next;
        if i + 10 > buf.len() {
            return out;
        }
        let rtype = u16::from_be_bytes([buf[i], buf[i + 1]]);
        let rdlen = u16::from_be_bytes([buf[i + 8], buf[i + 9]]) as usize;
        i += 10;
        if i + rdlen > buf.len() {
            return out;
        }
        if rtype == 1 && rdlen == 4 {
            out.push((name, [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]));
        }
        i += rdlen;
    }
    out
}

/// The server name in a TLS ClientHello, when the segment holds one.
///
/// This is what turns an HTTPS connection into a readable destination. It reads
/// the handshake and stops at the first record; a resumption or a segmented
/// ClientHello simply yields nothing, which costs a name and nothing else.
pub fn tls_sni(buf: &[u8]) -> Option<String> {
    // TLS record: type 22 (handshake), version, length.
    if buf.len() < 5 || buf[0] != 22 {
        return None;
    }
    let rec_len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
    let rec = buf.get(5..5 + rec_len.min(buf.len().saturating_sub(5)))?;
    // Handshake: type 1 (ClientHello), length.
    if rec.len() < 4 || rec[0] != 1 {
        return None;
    }
    let body = rec.get(4..)?;
    // version(2) random(32) session_id(1+n) cipher_suites(2+n) compression(1+n)
    let mut i = 34;
    let sid = *body.get(i)? as usize;
    i += 1 + sid;
    let cs = u16::from_be_bytes([*body.get(i)?, *body.get(i + 1)?]) as usize;
    i += 2 + cs;
    let comp = *body.get(i)? as usize;
    i += 1 + comp;
    let ext_len = u16::from_be_bytes([*body.get(i)?, *body.get(i + 1)?]) as usize;
    i += 2;
    let end = (i + ext_len).min(body.len());
    while i + 4 <= end {
        let kind = u16::from_be_bytes([body[i], body[i + 1]]);
        let len = u16::from_be_bytes([body[i + 2], body[i + 3]]) as usize;
        i += 4;
        if i + len > end {
            return None;
        }
        if kind == 0 {
            // server_name: list length(2) type(1) name length(2) name
            let ext = &body[i..i + len];
            if ext.len() < 5 {
                return None;
            }
            let name_len = u16::from_be_bytes([ext[3], ext[4]]) as usize;
            let name = ext.get(5..5 + name_len)?;
            return Some(String::from_utf8_lossy(name).to_string());
        }
        i += len;
    }
    None
}

/// The `Host:` header of a plain HTTP request.
pub fn http_host(buf: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(buf.get(..buf.len().min(2048))?).ok()?;
    let mut lines = text.split("\r\n");
    let first = lines.next()?;
    if !first.starts_with("GET ")
        && !first.starts_with("POST ")
        && !first.starts_with("HEAD ")
        && !first.starts_with("PUT ")
        && !first.starts_with("CONNECT ")
    {
        return None;
    }
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (key, value) = line.split_once(':')?;
        if key.eq_ignore_ascii_case("host") {
            return Some(value.trim().to_string());
        }
    }
    None
}

// ---------------------------------------------------------------- pacing

/// A token bucket expressed as the moment the next packet may leave.
///
/// `rate_bps` is bytes per second, which is what the UI calls KB/s times 1024.
/// `None` means the queue is already too far behind, so the packet is dropped
/// instead of delayed: a capped link should feel slow, not unresponsive.
pub fn pace(len: usize, rate_bps: u64, slot: &mut Instant, now: Instant) -> Option<Duration> {
    if rate_bps == 0 {
        return Some(Duration::ZERO);
    }
    let at = (*slot).max(now) + Duration::from_secs_f64(len as f64 / rate_bps as f64);
    if at > now + Duration::from_millis(300) {
        return None;
    }
    *slot = at;
    Some(at.saturating_duration_since(now))
}

/// The name a destination is filed under, cleaned of the forms DNS uses that a
/// person does not read.
pub fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if name.is_empty() || name.len() > 253 {
        return None;
    }
    // A name has to have a dot to be worth showing, and must not be an address.
    if !name.contains('.') || name.parse::<std::net::Ipv4Addr>().is_ok() {
        return None;
    }
    if name.ends_with(".local") || name.ends_with(".in-addr.arpa") || name.ends_with(".arpa") {
        return None;
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipv4(src: [u8; 4], dst: [u8; 4], proto: u8, payload: &[u8]) -> Vec<u8> {
        let total = 20 + payload.len();
        let mut p = vec![0u8; 20];
        p[0] = 0x45;
        p[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        p[8] = 64;
        p[9] = proto;
        p[12..16].copy_from_slice(&src);
        p[16..20].copy_from_slice(&dst);
        p.extend_from_slice(payload);
        p
    }

    #[test]
    fn a_forwarded_packet_names_its_two_ends() {
        let frame = ipv4([192, 168, 1, 39], [1, 1, 1, 1], 6, &[0u8; 20]);
        let ip = Ipv4::parse(&frame).expect("parse");
        assert_eq!(ip_text(ip.src), "192.168.1.39");
        assert_eq!(ip_text(ip.dst), "1.1.1.1");
        assert_eq!(ip.protocol, 6);
    }

    #[test]
    fn a_fragment_after_the_first_is_not_read_as_a_header() {
        // A later fragment has no transport header. Reading ports out of its
        // data would attribute bytes to a random service.
        let mut frame = ipv4([192, 168, 1, 39], [1, 1, 1, 1], 6, &[0u8; 40]);
        frame[6] = 0x00;
        frame[7] = 0x10; // offset 16, not the first fragment
        assert!(Ipv4::parse(&frame).is_none());
    }

    #[test]
    fn tcp_payload_starts_where_its_header_ends() {
        // 8 words of header, so the data begins at byte 32 and not at 20. Reading
        // from 20 would hand a TLS record to the parser with 12 bytes of options
        // glued to the front of it.
        let mut tcp = vec![0u8; 36];
        tcp[12] = 8 << 4;
        tcp[32..].copy_from_slice(b"data");
        let body = payload_after_transport(6, &tcp).expect("body");
        assert_eq!(body, b"data");
        // A header shorter than the minimum is not a header.
        let mut short = vec![0u8; 12];
        short[12 - 1] = 0;
        assert!(payload_after_transport(6, &short).is_none());
    }

    #[test]
    fn a_dns_response_yields_its_names() {
        // Header + one question (example.com A) + one answer pointing at 93.184.216.34.
        let mut d = vec![0x12, 0x34, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0, 0, 0, 0];
        for label in ["example", "com"] {
            d.push(label.len() as u8);
            d.extend_from_slice(label.as_bytes());
        }
        d.push(0);
        d.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        d.extend_from_slice(&[0xc0, 0x0c]);
        d.extend_from_slice(&[0x00, 0x01, 0x00, 0x01, 0, 0, 0, 60, 0x00, 0x04]);
        d.extend_from_slice(&[93, 184, 216, 34]);
        let names = dns_answers(&d);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].0, "example.com");
        assert_eq!(names[0].1, [93, 184, 216, 34]);
    }

    #[test]
    fn a_dns_query_is_not_read_as_an_answer() {
        let mut q = vec![0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
        q.push(3);
        q.extend_from_slice(b"www");
        q.push(0);
        q.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        assert!(dns_answers(&q).is_empty());
    }

    fn client_hello(sni: &str) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&[0x03, 0x03]); // version
        body.extend_from_slice(&[0u8; 32]); // random
        body.push(0); // session id length
        body.extend_from_slice(&[0x00, 0x02, 0x13, 0x01]); // one cipher suite
        body.extend_from_slice(&[0x01, 0x00]); // one compression method
        // extension: server_name
        let name = sni.as_bytes();
        let mut ext = Vec::new();
        ext.extend_from_slice(&((name.len() + 3) as u16).to_be_bytes());
        ext.push(0); // host_name type
        ext.extend_from_slice(&(name.len() as u16).to_be_bytes());
        ext.extend_from_slice(name);
        let mut exts = Vec::new();
        exts.extend_from_slice(&[0x00, 0x00]);
        exts.extend_from_slice(&(ext.len() as u16).to_be_bytes());
        exts.extend_from_slice(&ext);
        body.extend_from_slice(&(exts.len() as u16).to_be_bytes());
        body.extend_from_slice(&exts);

        let mut handshake = Vec::new();
        handshake.push(1);
        let len = body.len() as u32;
        handshake.extend_from_slice(&[(len >> 16) as u8, (len >> 8) as u8, len as u8]);
        handshake.extend_from_slice(&body);

        let mut rec = Vec::new();
        rec.push(22);
        rec.extend_from_slice(&[0x03, 0x01]);
        rec.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
        rec.extend_from_slice(&handshake);
        rec
    }

    #[test]
    fn a_client_hello_names_the_site() {
        let rec = client_hello("cdn.example.net");
        assert_eq!(tls_sni(&rec).as_deref(), Some("cdn.example.net"));
    }

    #[test]
    fn a_truncated_client_hello_yields_nothing_instead_of_panicking() {
        let rec = client_hello("cdn.example.net");
        for cut in 1..rec.len() {
            let _ = tls_sni(&rec[..cut]);
        }
    }

    #[test]
    fn an_http_request_names_the_site() {
        let req = b"GET /x HTTP/1.1\r\nHost: example.org\r\nUser-Agent: x\r\n\r\n";
        assert_eq!(http_host(req).as_deref(), Some("example.org"));
        // A body that is not a request must not be read as one.
        assert!(http_host(b"\x16\x03\x01 and then nothing").is_none());
    }

    #[test]
    fn the_cap_holds_a_slow_link_and_drops_what_is_already_too_late() {
        let now = Instant::now();
        let mut slot = now;
        // 1500 bytes at 100 KB/s takes 14.6ms to put on the wire, and that is
        // what the first frame is held for: the bucket paces transmission, it
        // does not let a burst through first. In steady state the loop sleeps
        // that long per frame and the slot keeps up with the clock.
        let per_frame = Duration::from_secs_f64(1500.0 / (100.0 * 1024.0));
        let first = pace(1500, 100 * 1024, &mut slot, now).expect("first");
        assert!(first >= per_frame && first <= per_frame + Duration::from_millis(1));
        // The same frame again without time passing is the case the deadline is
        // for: the release time is now two frames ahead, and at 100 KB/s that is
        // still fine.
        let second = pace(1500, 100 * 1024, &mut slot, now).expect("second");
        assert!(second >= first + per_frame);
        // A queue that is already a third of a second ahead of the clock is
        // dropped instead of delayed, or a capped link turns into lag.
        let mut ahead = now + Duration::from_millis(400);
        assert!(pace(1500, 100 * 1024, &mut ahead, now).is_none());
        // No cap means no wait, and no bucket arithmetic.
        let mut free = now;
        assert_eq!(pace(9000, 0, &mut free, now), Some(Duration::ZERO));
    }

    #[test]
    fn a_destination_name_is_cleaned_or_refused() {
        assert_eq!(clean_name("Example.COM.").as_deref(), Some("example.com"));
        assert!(clean_name("").is_none());
        assert!(clean_name("localhost").is_none(), "a bare word is not a destination");
        assert!(clean_name("192.168.1.1").is_none(), "an address is not a name");
        assert!(clean_name("printer.local").is_none(), "a lan name is not a destination");
        assert!(clean_name("1.0.0.10.in-addr.arpa").is_none());
    }

    #[test]
    fn the_filter_names_both_directions_for_every_device() {
        let f = forward_filter(&["192.168.1.39".to_string()]);
        assert!(f.contains("ip.SrcAddr == 192.168.1.39"));
        assert!(f.contains("ip.DstAddr == 192.168.1.39"));
        assert!(f.starts_with("ip and ("));
        // An empty list must not become a filter that matches everything.
        let none = forward_filter(&[]);
        assert!(none.contains("0.0.0.0"), "an empty device list must match nothing");
    }
}
