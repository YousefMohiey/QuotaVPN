//! Route one LAN device through the tunnel without installing anything on it.
//!
//! The device keeps its own settings; what changes is what it believes about the
//! router. We send it a forged ARP reply saying the gateway address now lives at our
//! MAC, it sends its traffic to us, and since we forward and its next hop is the
//! engine's tun, its traffic comes out of the tunnel.
//!
//! Three things make this safe to ship or not ship at all:
//!
//! - an intention record is written BEFORE the first frame, so a crash cannot leave a
//!   device pointing at a machine that stopped forwarding;
//! - the restore frame names the real router MAC, and is sent on stop and on start-up
//!   whenever an intention exists;
//! - every step is read back. A forwarding state that cannot be verified is a failure
//!   reported to the caller, never a silent success.
//!
//! Only Windows uses this. The phone's tunnel is a real VpnService and needs no tricks.

use std::net::Ipv4Addr;
use std::path::PathBuf;

/// Ethernet frame carrying an ARP reply. Fixed size: 14 byte header, 28 byte body.
pub const FRAME_LEN: usize = 42;

const ETHERTYPE_ARP: u16 = 0x0806;
const ARP_REPLY: u16 = 2;
const HWTYPE_ETHERNET: u16 = 1;
const PROTO_IPV4: u16 = 0x0800;

fn mac_bytes(mac: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut out = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        out[i] = u8::from_str_radix(p, 16).ok()?;
    }
    Some(out)
}

/// An ARP reply that tells `target_ip` that `claimed_ip` is at `sender_mac`.
///
/// This is the whole mechanism. Called with our MAC it takes the device over; called
/// with the router's MAC it hands it back. Same shape, opposite direction, which is
/// why the restore path is testable against the claim path.
pub fn arp_reply(
    sender_mac: &str,
    claimed_ip: Ipv4Addr,
    target_mac: &str,
    target_ip: Ipv4Addr,
) -> Option<[u8; FRAME_LEN]> {
    let smac = mac_bytes(sender_mac)?;
    let tmac = mac_bytes(target_mac)?;
    let mut f = [0u8; FRAME_LEN];

    // Ethernet: the target is the unicast destination, we are the source.
    f[0..6].copy_from_slice(&tmac);
    f[6..12].copy_from_slice(&smac);
    f[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());

    // ARP body.
    f[14..16].copy_from_slice(&HWTYPE_ETHERNET.to_be_bytes());
    f[16..18].copy_from_slice(&PROTO_IPV4.to_be_bytes());
    f[18] = 6; // hardware address length
    f[19] = 4; // protocol address length
    f[20..22].copy_from_slice(&ARP_REPLY.to_be_bytes());
    f[22..28].copy_from_slice(&smac); // sender hardware
    f[28..32].copy_from_slice(&claimed_ip.octets()); // sender protocol
    f[32..38].copy_from_slice(&tmac); // target hardware
    f[38..42].copy_from_slice(&target_ip.octets()); // target protocol
    Some(f)
}

/// The frame that hands a device back to the router.
pub fn arp_release(
    router_mac: &str,
    gateway_ip: Ipv4Addr,
    target_mac: &str,
    target_ip: Ipv4Addr,
) -> Option<[u8; FRAME_LEN]> {
    arp_reply(router_mac, gateway_ip, target_mac, target_ip)
}

// ---------------------------------------------------------------- intention record

fn intent_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("quotavpn").join("lan_route_intent.txt")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub target_ip: String,
    pub target_mac: String,
    pub gateway_ip: String,
    pub interface: String,
    /// The router's real hardware address, captured before the first forged frame.
    /// It must never be re-read at release time: our own claim can rewrite the local
    /// neighbour table, and then "restore" would hand the device back to us.
    pub router_mac: String,
}

impl Intent {
    pub fn encode(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.target_ip, self.target_mac, self.gateway_ip, self.interface, self.router_mac
        )
    }

    pub fn decode(line: &str) -> Option<Intent> {
        let p: Vec<&str> = line.trim().split('|').collect();
        if p.len() != 5 || p.iter().any(|s| s.is_empty()) {
            return None;
        }
        Some(Intent {
            target_ip: p[0].to_string(),
            target_mac: p[1].to_string(),
            gateway_ip: p[2].to_string(),
            interface: p[3].to_string(),
            router_mac: p[4].to_string(),
        })
    }
}

/// Written before the first forged frame. Its existence is the promise that a
/// restore is owed, and that is the only reason a crash is survivable here.
pub fn remember_intent(intent: &Intent) -> Result<(), String> {
    let path = intent_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, intent.encode()).map_err(|e| e.to_string())
}

pub fn read_intent() -> Option<Intent> {
    let text = std::fs::read_to_string(intent_path()).ok()?;
    Intent::decode(&text)
}

pub fn forget_intent() {
    let _ = std::fs::remove_file(intent_path());
}

// ---------------------------------------------------------------- forwarding control

/// Read back whether this interface forwards. Parsing `netsh` is ugly but it is the
/// same source of truth the user sees, and the whole feature depends on it being on.
pub fn forwarding_enabled(interface: &str) -> Result<bool, String> {
    let out = crate::lan::hidden("netsh")
        .args(["interface", "ipv4", "show", "interface", interface])
        .output()
        .map_err(|e| e.to_string())?;
    // Read the ONE line that answers the question. Scanning the whole output for
    // "disabled" is wrong: the same block lists Router Discovery and Directed MAC Wake
    // up, both disabled, so the answer came back false no matter what the state was.
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if let Some(rest) = lower.strip_prefix("forwarding") {
            let value = rest.split(':').nth(1).unwrap_or("").trim();
            return Ok(value == "enabled");
        }
    }
    Err(format!("netsh printed no Forwarding line for {interface}"))
}

pub fn set_forwarding(interface: &str, on: bool) -> Result<(), String> {
    let state = if on { "enabled" } else { "disabled" };
    let out = crate::lan::hidden("netsh")
        // One argument: netsh rejects "forwarding enabled" as two, and answers
        // "The parameter is incorrect", which reads like a permissions problem and is not.
        .args([
            "interface",
            "ipv4",
            "set",
            "interface",
            interface,
            &format!("forwarding={state}"),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let said = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return Err(if err.is_empty() && said.is_empty() {
            "netsh failed with no message, which is what happens without elevation".to_string()
        } else if err.is_empty() {
            said
        } else {
            err
        });
    }
    if forwarding_enabled(interface)? != on {
        return Err(format!("forwarding did not become {state} on {interface}"));
    }
    Ok(())
}

// ---------------------------------------------------------------- the machine

/// What this machine looks like on the LAN, discovered rather than assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    pub ip: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub if_index: u32,
    pub mac: String,
}

/// Ask Windows, in JSON, because `ipconfig` and `netsh` are localised and this has to
/// work on a machine whose output language nobody checked.
impl Adapter {
    /// Only the local address, for the capture side of the live test.
    pub fn discover_ip() -> Result<Ipv4Addr, String> {
        Ok(adapter()?.ip)
    }
}

pub fn adapter() -> Result<Adapter, String> {
    let script = "$c = Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -and $_.NetAdapter.Status -eq 'Up' } | Select-Object -First 1;                   if (-not $c) { '' } else { [pscustomobject]@{                     ip = [string]$c.IPv4Address.IPAddress;                     gateway = [string]$c.IPv4DefaultGateway.NextHop;                     ifindex = [int]$c.NetAdapter.ifIndex;                     mac = [string]$c.NetAdapter.MacAddress } | ConvertTo-Json -Compress }";
    // Hidden: this is a GUI app, and a console flashing on every adapter lookup is
    // the difference between a feature and a bug report.
    let out = crate::lan::hidden("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    if text.is_empty() {
        return Err("no adapter with an IPv4 gateway is up".into());
    }
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("adapter json: {e}"))?;
    let ip: Ipv4Addr = v["ip"].as_str().ok_or("no ip")?.parse().map_err(|_| "bad ip")?;
    let gateway: Ipv4Addr = v["gateway"].as_str().ok_or("no gateway")?.parse().map_err(|_| "bad gateway")?;
    let if_index = v["ifindex"].as_u64().ok_or("no ifindex")? as u32;
    let mac = v["mac"].as_str().ok_or("no mac")?.to_ascii_lowercase().replace('-', ":");
    Ok(Adapter { ip, gateway, if_index, mac })
}

// ---------------------------------------------------------------- npcap, loaded at runtime

// Npcap is loaded when it is needed, never linked.
//
// The first version linked `wpcap` into the binary, which made the DLL a startup
/// dependency: on any PC without Npcap installed the app died with
/// "The code execution cannot proceed because wpcap.dll was not found" before it could draw a
/// window. A packet driver is needed by one optional feature, so its absence has to cost that
/// feature and nothing else.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut std::ffi::c_void;
    fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
}

#[repr(C)]
struct PcapIf {
    next: *mut PcapIf,
    name: *mut std::os::raw::c_char,
    description: *mut std::os::raw::c_char,
    addresses: *mut PcapAddr,
    flags: u32,
}

#[repr(C)]
struct PcapAddr {
    next: *mut PcapAddr,
    addr: *mut Sockaddr,
    netmask: *mut Sockaddr,
    broadaddr: *mut Sockaddr,
    dstaddr: *mut Sockaddr,
}

#[repr(C)]
struct Sockaddr {
    family: u16,
    data: [u8; 14],
}

#[repr(C)]
struct PcapPkthdr {
    // Windows `struct timeval` is two 32-bit longs, not two 64-bit ones.
    ts_sec: i32,
    ts_usec: i32,
    caplen: u32,
    len: u32,
}

type FindAllDevs = unsafe extern "C" fn(*mut *mut PcapIf, *mut std::os::raw::c_char) -> i32;
type FreeAllDevs = unsafe extern "C" fn(*mut PcapIf);
type OpenLive = unsafe extern "C" fn(
    *const std::os::raw::c_char,
    i32,
    i32,
    i32,
    *mut std::os::raw::c_char,
) -> *mut std::ffi::c_void;
type SendPacket = unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, i32) -> i32;
type CloseFn = unsafe extern "C" fn(*mut std::ffi::c_void);
type GetErr = unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::os::raw::c_char;
type NextEx = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *mut *mut PcapPkthdr,
    *mut *const u8,
) -> i32;

struct Npcap {
    findalldevs: FindAllDevs,
    freealldevs: FreeAllDevs,
    open_live: OpenLive,
    sendpacket: SendPacket,
    close: CloseFn,
    geterr: GetErr,
    next_ex: NextEx,
}

// Only a success is cached. Caching the failure would mean an install that happens while the
// app is running is not noticed until the next restart, which is exactly when the user has just
// clicked "install it".
static NPCAP: std::sync::OnceLock<Npcap> = std::sync::OnceLock::new();

fn npcap() -> Option<&'static Npcap> {
    if let Some(loaded) = NPCAP.get() {
        return Some(loaded);
    }
    match load_npcap() {
        Some(found) => {
            let _ = NPCAP.set(found);
            NPCAP.get()
        }
        None => None,
    }
}

/// Is the driver installed on this machine? Reads the disk, so it notices an install that
/// happened a moment ago.
pub fn npcap_installed() -> bool {
    npcap().is_some()
}

/// Where the vendor publishes the installer. Npcap does not permit a product to ship its
/// installer inside another one without an OEM licence, so this downloads it from the vendor
/// and runs it: the user's machine fetches it, we do not redistribute it.
/// Known good releases, newest first. The vendor removes old files, so a hardcoded single URL
/// eventually turns into "download failed" instead of "that version is gone".
const NPCAP_VERSIONS: [&str; 3] = ["1.90", "1.89", "1.80"];

fn npcap_url(version: &str) -> String {
    format!("https://npcap.com/dist/npcap-{version}.exe")
}

/// Is this file signed by the vendor, and does the signature verify?
///
/// The app is elevated, so whatever this installs runs as administrator. A downloaded binary is
/// the one place in this program where trusting the network would be a privilege escalation.
fn verify_vendor_signature(path: &std::path::Path) -> Result<(), String> {
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath '{}';          [pscustomobject]@{{ status = [string]$s.Status; subject = [string]$s.SignerCertificate.Subject }}          | ConvertTo-Json -Compress",
        path.display()
    );
    let out = crate::lan::hidden("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|_| "could not read the installer's signature".to_string())?;
    let status = value["status"].as_str().unwrap_or("").to_ascii_lowercase();
    let subject = value["subject"].as_str().unwrap_or("").to_ascii_lowercase();
    if status != "valid" {
        return Err(format!(
            "the downloaded installer's signature is not valid ({status}), so it was not run"
        ));
    }
    // The real signer, read off npcap-1.90.exe on 2026-11-12:
    //   CN=Nmap Software LLC, O=Nmap Software LLC, Beaverton, Oregon, US
    // Matched on the organisation rather than on a bare "nmap" so a similarly named
    // certificate cannot pass.
    if !subject.contains("nmap software") {
        return Err(format!(
            "the downloaded installer is signed by somebody else ({subject}), so it was not run"
        ));
    }
    Ok(())
}

/// Download and install Npcap silently, then report whether it actually works.
///
/// This is the only way to get a feature that sends a raw ARP frame: an Ethernet frame cannot
/// be sent from user mode on Windows without a packet driver. Writing our own driver instead
/// would mean an EV certificate and Microsoft attestation for a single feature.
pub fn install_npcap() -> Result<String, String> {
    if npcap().is_some() {
        return Ok("Npcap is already installed".into());
    }
    let dest = std::env::temp_dir().join("npcap-installer.exe");
    let _ = std::fs::remove_file(&dest);

    let mut fetched = None;
    for version in NPCAP_VERSIONS {
        let url = npcap_url(version);
        let script = format!(
            "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri '{url}' -OutFile '{}' -UseBasicParsing",
            dest.display()
        );
        let out = crate::lan::hidden("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() && dest.exists() {
            fetched = Some(version);
            break;
        }
        let _ = std::fs::remove_file(&dest);
    }
    let Some(version) = fetched else {
        return Err(format!(
            "could not download the Npcap installer from npcap.com (tried {}). The vendor may \
             have moved it; install it by hand from npcap.com and press retry.",
            NPCAP_VERSIONS.join(", ")
        ));
    };

    // The app runs elevated, so the installer it executes would run elevated too. Never execute
    // a downloaded binary without checking who signed it.
    if let Err(e) = verify_vendor_signature(&dest) {
        let _ = std::fs::remove_file(&dest);
        return Err(e);
    }

    // /S is the vendor's silent switch. The app already runs elevated, which the driver needs.
    let run = crate::lan::hidden(&dest.to_string_lossy())
        .arg("/S")
        .arg("/winpcap_mode=yes")
        .status()
        .map_err(|e| format!("could not run the Npcap installer: {e}"))?;
    let _ = std::fs::remove_file(&dest);
    if !run.success() {
        return Err("the Npcap installer did not finish successfully".into());
    }

    // Trust the disk, not the exit code: the installer can finish and leave no driver.
    if npcap().is_some() {
        Ok(format!("Npcap {version} installed, device routing is ready"))
    } else {
        Err("the installer finished but wpcap.dll is still missing; a reboot may be needed".into())
    }
}

/// Look for the driver where Npcap actually installs it, then on PATH.
///
/// Npcap puts `wpcap.dll` in `System32\Npcap`, which is not on PATH, so relying on the
/// loader's search alone would fail on machines where it is installed.
fn load_npcap() -> Option<Npcap> {
    const CANDIDATES: [&str; 2] = [
        r"C:\Windows\System32\Npcap\wpcap.dll",
        r"wpcap.dll",
    ];
    for candidate in CANDIDATES {
        let wide: Vec<u16> = candidate.encode_utf16().chain(std::iter::once(0)).collect();
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            continue;
        }
        // SAFETY: the names are NUL-terminated and the symbols are the documented C API.
        unsafe {
            let sym = |name: &[u8]| GetProcAddress(module, name.as_ptr());
            let findalldevs = sym(b"pcap_findalldevs\0");
            let freealldevs = sym(b"pcap_freealldevs\0");
            let open_live = sym(b"pcap_open_live\0");
            let sendpacket = sym(b"pcap_sendpacket\0");
            let close = sym(b"pcap_close\0");
            let geterr = sym(b"pcap_geterr\0");
            let next_ex = sym(b"pcap_next_ex\0");
            if [
                findalldevs, freealldevs, open_live, sendpacket, close, geterr, next_ex,
            ]
            .iter()
            .any(|p| p.is_null())
            {
                continue;
            }
            return Some(Npcap {
                findalldevs: std::mem::transmute::<*mut std::ffi::c_void, FindAllDevs>(findalldevs),
                freealldevs: std::mem::transmute::<*mut std::ffi::c_void, FreeAllDevs>(freealldevs),
                open_live: std::mem::transmute::<*mut std::ffi::c_void, OpenLive>(open_live),
                sendpacket: std::mem::transmute::<*mut std::ffi::c_void, SendPacket>(sendpacket),
                close: std::mem::transmute::<*mut std::ffi::c_void, CloseFn>(close),
                geterr: std::mem::transmute::<*mut std::ffi::c_void, GetErr>(geterr),
                next_ex: std::mem::transmute::<*mut std::ffi::c_void, NextEx>(next_ex),
            });
        }
    }
    None
}

/// Is the packet driver available on this machine? Cheap, and it never panics.
pub fn npcap_ready() -> bool {
    npcap().is_some()
}

/// A packet socket on the interface that owns `ip`.
pub struct Socket {
    handle: *mut std::ffi::c_void,
    api: &'static Npcap,
}

// The handle is only used behind the claim lock, one call at a time.
unsafe impl Send for Socket {}

impl Socket {
    pub fn open_for(ip: Ipv4Addr) -> Result<Socket, String> {
        let api = npcap().ok_or_else(|| {
            "Npcap is not installed, so this PC cannot route a device. Install Npcap from \
             npcap.com and try again; nothing else about QuotaVPN is affected."
                .to_string()
        })?;
        let mut errbuf = [0i8; 256];
        let mut devs: *mut PcapIf = std::ptr::null_mut();
        if unsafe { (api.findalldevs)(&mut devs, errbuf.as_mut_ptr()) } != 0 {
            return Err(format!("pcap_findalldevs: {}", cstr(&errbuf)));
        }
        let mut chosen: Option<String> = None;
        let mut cur = devs;
        while !cur.is_null() {
            let d = unsafe { &*cur };
            let name = unsafe { std::ffi::CStr::from_ptr(d.name) }.to_string_lossy().into_owned();
            let mut a = d.addresses;
            while !a.is_null() {
                let ad = unsafe { &*a };
                if !ad.addr.is_null() {
                    let sa = unsafe { &*ad.addr };
                    if sa.family == 2 {
                        let octets = [sa.data[2], sa.data[3], sa.data[4], sa.data[5]];
                        if Ipv4Addr::from(octets) == ip {
                            chosen = Some(name.clone());
                            break;
                        }
                    }
                }
                a = ad.next;
            }
            if chosen.is_some() {
                break;
            }
            cur = d.next;
        }
        unsafe { (api.freealldevs)(devs) };
        let name = chosen.ok_or_else(|| format!("no interface owns {ip}"))?;

        let cname = std::ffi::CString::new(name.clone()).map_err(|e| e.to_string())?;
        let handle = unsafe { (api.open_live)(cname.as_ptr(), 65536, 0, 100, errbuf.as_mut_ptr()) };
        if handle.is_null() {
            return Err(format!("pcap_open_live {name}: {}", cstr(&errbuf)));
        }
        Ok(Socket { handle, api })
    }

    pub fn send(&self, frame: &[u8]) -> Result<(), String> {
        let rc = unsafe { (self.api.sendpacket)(self.handle, frame.as_ptr(), frame.len() as i32) };
        if rc != 0 {
            let msg = unsafe {
                let p = (self.api.geterr)(self.handle);
                if p.is_null() {
                    "unknown".to_string()
                } else {
                    std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
                }
            };
            return Err(format!("pcap_sendpacket: {msg}"));
        }
        Ok(())
    }

    /// Count frames arriving from `sender_mac` within `secs`.
    ///
    /// This is the only honest way to answer "did the claim work": if the device now
    /// believes we are its gateway, its frames start arriving addressed to our MAC.
    /// Only the 14-byte Ethernet header is read, so a misread header length cannot
    /// walk off the buffer into a claim about data we never saw.
    pub fn count_frames_from(&self, sender_mac: &str, secs: u64) -> usize {
        let want = mac_bytes(sender_mac);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut seen = 0;
        while std::time::Instant::now() < deadline {
            let mut hdr: *mut PcapPkthdr = std::ptr::null_mut();
            let mut data: *const u8 = std::ptr::null();
            let rc = unsafe { (self.api.next_ex)(self.handle, &mut hdr, &mut data) };
            if rc <= 0 || hdr.is_null() || data.is_null() {
                continue;
            }
            let caplen = unsafe { (*hdr).caplen };
            if caplen < 14 {
                continue;
            }
            let src = unsafe { std::slice::from_raw_parts(data.add(6), 6) };
            if Some([src[0], src[1], src[2], src[3], src[4], src[5]]) == want {
                seen += 1;
            }
        }
        seen
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.api.close)(self.handle) };
        }
    }
}

fn cstr(buf: &[i8]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

// ---------------------------------------------------------------- keeping ourselves online

/// Pin our own gateway entry to the real router for the duration of a claim.
///
/// The forged reply goes to the device, but this machine receives it too and updates its own
/// neighbour cache: the entry for the gateway becomes **our** MAC, our own traffic is sent to
/// ourselves, and the PC loses the network until the entry expires. A static entry outranks
/// the cache, so it holds until it is removed.
fn pin_gateway(interface: &str, gateway: Ipv4Addr, router_mac: &str) -> Result<(), String> {
    let out = crate::lan::hidden("netsh")
        .args([
            "interface",
            "ipv4",
            "set",
            "neighbors",
            interface,
            &gateway.to_string(),
            router_mac,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "could not pin the gateway: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Drop the pin so Windows resolves the router again on its own.
fn unpin_gateway(interface: &str, gateway: Ipv4Addr) -> Result<(), String> {
    let out = crate::lan::hidden("netsh")
        .args([
            "interface",
            "ipv4",
            "delete",
            "neighbors",
            interface,
            &gateway.to_string(),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        // A missing entry is fine: only the removal of the pin matters, and time also
        // repairs it, so this is reported but not fatal.
        let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !said.is_empty() {
            return Err(format!("could not unpin the gateway: {said}"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- claim and release

static MAINTAINING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Take one device over. Returns once the first frame is on the wire and forwarding is
/// confirmed, so a true return means "it is working now", not "it was attempted".
pub fn claim(target_ip: Ipv4Addr, target_mac: &str) -> Result<Adapter, String> {
    let ad = adapter()?;

    // Captured before the first frame. If the table already says we are the gateway,
    // something else ran this feature and did not restore, and guessing is not an
    // option: a restore that names our own MAC restores nothing.
    // A live engine, not merely a tun adapter: `tun_adapter_present` is a name match, so an
    // adapter left behind by a killed run satisfies it, which is exactly why
    // `reset_tun_adapter` exists. `task_engine_pid` verifies a real sing-box.
    if crate::vpn::task_engine_pid().is_none() {
        return Err("connect the VPN first: the engine is not running".into());
    }
    // Alive is not the same as carrying traffic: a sing-box that started and did not
    // bring a tun up would take the claim and have nowhere to put the packets.
    if !crate::vpn::tun_adapter_present() {
        return Err("connect the VPN first: the tunnel interface is not up".into());
    }

    let router_mac = crate::lan::mac_for(&ad.gateway.to_string())
        .ok_or_else(|| format!("no MAC known for the gateway {}", ad.gateway))?;
    if router_mac.eq_ignore_ascii_case(&ad.mac) {
        return Err(format!(
            "the neighbour table says we are the gateway ({}); refusing to build a restore that would name ourselves. Release the previous claim, or let the entry expire.",
            ad.gateway
        ));
    }
    let intent = Intent {
        target_ip: target_ip.to_string(),
        target_mac: target_mac.to_ascii_lowercase(),
        gateway_ip: ad.gateway.to_string(),
        interface: ad.if_index.to_string(),
        router_mac: router_mac.clone(),
    };
    // The promise comes first: if this process dies after this line and before the
    // frames, start-up still knows a device may be pointing at us.
    remember_intent(&intent)?;

    // Order matters and it is not cosmetic: the permit goes in first, because a device
    // pointed at a machine with forwarding on and no rule is a device with no internet.
    // Anything that fails from here unwinds through `release`, which sends the restore
    // frame, turns forwarding off, removes the rule and forgets the record.
    let armed = (|| -> Result<(), String> {
        open_firewall(target_ip)?;
        set_forwarding(&ad.if_index.to_string(), true)?;
        // Before the frame, because that is the moment our own cache is at risk.
        pin_gateway(&ad.if_index.to_string(), ad.gateway, &router_mac)?;
        let sock = Socket::open_for(ad.ip)?;
        let frame = arp_reply(&ad.mac, ad.gateway, target_mac, target_ip)
            .ok_or_else(|| "could not build the claim frame".to_string())?;
        sock.send(&frame)?;
        Ok(())
    })();
    if let Err(e) = armed {
        let _ = release();
        return Err(e);
    }

    MAINTAINING.store(true, std::sync::atomic::Ordering::SeqCst);
    let mac = ad.mac.clone();
    let gw = ad.gateway;
    // owned for the thread: the caller's &str does not outlive this call
    let target_mac_owned = target_mac.to_ascii_lowercase();
    let target_ip_owned = target_ip;
    let our_ip = ad.ip;
    std::thread::spawn(move || {
        // ARP entries expire; a single claim looks like a feature that stops working.
        while MAINTAINING.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if !MAINTAINING.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            if let Ok(s) = Socket::open_for(our_ip) {
                if let Some(f) = arp_reply(&mac, gw, &target_mac_owned, target_ip_owned) {
                    let _ = s.send(&f);
                }
            }
        }
    });
    Ok(ad)
}

fn outbound_rule(target_ip: Ipv4Addr) -> String {
    format!("QuotaVPN lan-route {target_ip}")
}

fn return_rule(target_ip: Ipv4Addr) -> String {
    format!("QuotaVPN lan-route return {target_ip}")
}

#[derive(Default, Debug)]
struct RuleFacts {
    direction: String,
    local: String,
    remote: String,
    action: String,
}

/// Read one rule back as facts, not as a substring.
///
/// The old check accepted the address appearing anywhere in the output, and the rule name
/// itself contains it, so `RemoteIP: Any` passed. Parse the fields that matter.
fn rule_facts(name: &str) -> Result<Option<RuleFacts>, String> {
    let out = crate::lan::hidden("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={name}")])
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    if text.to_lowercase().contains("no rules match") {
        return Ok(None);
    }
    let mut facts = RuleFacts::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let value = v.trim().to_string();
        match k.trim().to_ascii_lowercase().as_str() {
            "direction" => facts.direction = value,
            "localip" => facts.local = value,
            "remoteip" => facts.remote = value,
            "action" => facts.action = value,
            _ => {}
        }
    }
    if facts.direction.is_empty() && facts.action.is_empty() {
        return Ok(None);
    }
    Ok(Some(facts))
}

/// Is this field exactly the address we asked for?
///
/// Deliberately not permissive. Accepting "any" made the return rule's verification vacuous:
/// a rule scoped to nothing passed as a rule scoped to the device, which is the same hole the
/// substring check had, moved into a helper. Only the address, or the address with a /32, is
/// a match. Where "any" is legitimate it is checked by the caller as its own condition.
fn matches_ip(value: &str, ip: Ipv4Addr) -> bool {
    value.split(',').any(|part| {
        let part = part.trim();
        part == ip.to_string() || part == format!("{ip}/32")
    })
}

/// Let both legs through.
///
/// Two rules, because one cannot do both jobs. `dir=in remoteip=<device>` matches only
/// packets whose source is the device: the leg going out. Replies from the internet come
/// back on the tunnel with a public source, which that shape can never match, so the
/// return leg is scoped by destination instead: `dir=in localip=<device> remoteip=any`.
fn open_firewall(target_ip: Ipv4Addr) -> Result<(), String> {
    let _ = close_firewall(target_ip); // idempotent: stale rules would otherwise stack

    let out_rule = outbound_rule(target_ip);
    let out = crate::lan::hidden("netsh")
        .args([
            "advfirewall", "firewall", "add", "rule",
            &format!("name={out_rule}"), "dir=in", "action=allow",
            &format!("remoteip={target_ip}"), "profile=any",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("could not add the outbound rule: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }

    let ret_rule = return_rule(target_ip);
    let out = crate::lan::hidden("netsh")
        .args([
            "advfirewall", "firewall", "add", "rule",
            &format!("name={ret_rule}"), "dir=in", "action=allow",
            &format!("localip={target_ip}"), "remoteip=any", "profile=any",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let _ = close_firewall(target_ip);
        return Err(format!("could not add the return rule: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }

    // Both legs verified, by field and not by substring.
    match rule_facts(&out_rule)? {
        Some(f) if f.direction.eq_ignore_ascii_case("in")
            && f.action.eq_ignore_ascii_case("allow")
            && matches_ip(&f.remote, target_ip) => {}
        other => {
            let _ = close_firewall(target_ip);
            return Err(format!("the outbound rule is not what was asked for: {other:?}"));
        }
    }
    match rule_facts(&ret_rule)? {
        Some(f) if f.direction.eq_ignore_ascii_case("in")
            && f.action.eq_ignore_ascii_case("allow")
            && matches_ip(&f.local, target_ip) => {}
        other => {
            let _ = close_firewall(target_ip);
            return Err(format!("the return rule is not what was asked for: {other:?}"));
        }
    }
    Ok(())
}

fn close_firewall(target_ip: Ipv4Addr) -> Result<(), String> {
    for name in [outbound_rule(target_ip), return_rule(target_ip)] {
        let _ = crate::lan::hidden("netsh")
            .args(["advfirewall", "firewall", "delete", "rule", &format!("name={name}")])
            .output()
            .map_err(|e| e.to_string())?;
    }
    // Absence has to be proven: these are inbound allows on every profile.
    for name in [outbound_rule(target_ip), return_rule(target_ip)] {
        if rule_facts(&name)?.is_some() {
            return Err(format!("the firewall rule {name} survived its deletion"));
        }
    }
    Ok(())
}

/// Hand everything back. Safe to call when nothing is claimed.
pub fn release() -> Result<(), String> {
    MAINTAINING.store(false, std::sync::atomic::Ordering::SeqCst);
    let Some(intent) = read_intent() else {
        return Ok(());
    };
    let ad = adapter()?;
    let target_ip: Ipv4Addr = intent.target_ip.parse().map_err(|_| "bad target ip")?;
    let gateway: Ipv4Addr = intent.gateway_ip.parse().map_err(|_| "bad gateway")?;

    // The MAC recorded when the claim was made, not whatever the table says now.
    let router_mac = intent.router_mac.clone();
    if router_mac.eq_ignore_ascii_case(&ad.mac) {
        return Err(format!(
            "the recorded router MAC is our own ({}), so the restore would be a no-op",
            router_mac
        ));
    }

    let sock = Socket::open_for(ad.ip)?;
    let frame = arp_release(&router_mac, gateway, &intent.target_mac, target_ip)
        .ok_or_else(|| "could not build the release frame".to_string())?;
    // three times: one frame can be lost and the device would stay pointed at us
    for _ in 0..3 {
        sock.send(&frame)?;
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    // After the restore frame, before the record is dropped: our own cache has to be
    // released too, or the PC stays pointed at itself.
    unpin_gateway(&intent.interface, gateway)?;
    set_forwarding(&intent.interface, false)?;
    // If the rule will not go, the record stays so start-up tries again. An inbound
    // allow that outlives its release is worse than a failed release.
    close_firewall(target_ip)?;
    forget_intent();
    Ok(())
}

/// Called at start-up. A record on disk means a previous run owed a restore.
pub fn recover() -> Option<String> {
    let intent = read_intent()?;
    Some(match release() {
        Ok(()) => format!("restored {} after an unclean stop", intent.target_ip),
        Err(e) => format!("could not restore {}: {e}", intent.target_ip),
    })
}

// ---------------------------------------------------------------- diagnosis

/// Everything that decides whether a claim can work, as lines a person can read.
///
/// Written because "it does not work" has five different causes in this path and guessing
/// between them cost days. Run this with a device selected and it also counts the frames that
/// device actually sends us, which is the one fact no amount of reading can supply.
pub fn diagnose(target: Option<(Ipv4Addr, String)>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut say = |line: String| out.push(line);

    say(format!("npcap loaded: {}", npcap().is_some()));
    match adapter() {
        Ok(ad) => {
            say(format!("our address: {} | gateway: {} | ifindex {} | mac {}", ad.ip, ad.gateway, ad.if_index, ad.mac));
            match crate::lan::mac_for(&ad.gateway.to_string()) {
                Some(m) => say(format!(
                    "gateway mac in the table: {m}{}",
                    if m.eq_ignore_ascii_case(&ad.mac) { "  <-- that is US, the pin is missing or the entry is poisoned" } else { "" }
                )),
                None => say("gateway mac in the table: none".to_string()),
            }
            match forwarding_enabled(&ad.if_index.to_string()) {
                Ok(v) => say(format!("forwarding on the LAN interface: {v}")),
                Err(e) => say(format!("forwarding: could not read ({e})")),
            }
        }
        Err(e) => say(format!("adapter lookup failed: {e}")),
    }

    say(format!("engine running: {}", crate::vpn::task_engine_pid().is_some()));
    say(format!("tun adapter present: {}", crate::vpn::tun_adapter_present()));
    match read_intent() {
        Some(i) => say(format!("intent on disk: target {} | router mac {}", i.target_ip, i.router_mac)),
        None => say("intent on disk: none".to_string()),
    }

    if let Some((target_ip, target_mac)) = target {
        match rule_facts(&outbound_rule(target_ip)) {
            Ok(Some(f)) => say(format!("rule outbound: {:?}", f)),
            Ok(None) => say("rule outbound: MISSING".to_string()),
            Err(e) => say(format!("rule outbound: could not read ({e})")),
        }
        match rule_facts(&return_rule(target_ip)) {
            Ok(Some(f)) => say(format!("rule return: {:?}", f)),
            Ok(None) => say("rule return: MISSING".to_string()),
            Err(e) => say(format!("rule return: could not read ({e})")),
        }
        match Socket::open_for(Ipv4Addr::new(0, 0, 0, 0)) {
            Ok(_) => say("packet socket: opened".to_string()),
            Err(_) => {}
        }
        if let Ok(ad) = adapter() {
            if let Ok(sock) = Socket::open_for(ad.ip) {
                let n = sock.count_frames_from(&target_mac, 6);
                say(format!("frames from {target_mac} in 6s: {n}{}", if n == 0 { "  <-- the device is not sending to us, so the claim never took" } else { "" }));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- live proof

/// A claim that undoes itself, so a panic or an early return cannot leave a device
/// pointed at a machine that stopped forwarding.
pub struct Claim {
    pub adapter: Adapter,
    pub target_ip: Ipv4Addr,
}

impl Drop for Claim {
    fn drop(&mut self) {
        let _ = release();
    }
}

/// Claim and return a guard. The guard releases on drop.
pub fn route_device(target_ip: Ipv4Addr, target_mac: &str) -> Result<Claim, String> {
    let adapter = claim(target_ip, target_mac)?;
    Ok(Claim { adapter, target_ip })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn the_claim_frame_names_us_as_the_gateway() {
        let f = arp_reply(
            "f0:11:22:33:44:55",
            ip("192.168.1.1"),
            "aa:bb:cc:dd:ee:ff",
            ip("192.168.1.28"),
        )
        .expect("frame");

        // ethernet
        assert_eq!(&f[0..6], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(&f[6..12], &[0xf0, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(u16::from_be_bytes([f[12], f[13]]), ETHERTYPE_ARP);
        // arp
        assert_eq!(u16::from_be_bytes([f[14], f[15]]), 1);
        assert_eq!(u16::from_be_bytes([f[16], f[17]]), 0x0800);
        assert_eq!((f[18], f[19]), (6, 4));
        assert_eq!(u16::from_be_bytes([f[20], f[21]]), 2, "must be a reply");
        assert_eq!(&f[22..28], &[0xf0, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(&f[28..32], &[192, 168, 1, 1], "claims the gateway address");
        assert_eq!(&f[32..38], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(&f[38..42], &[192, 168, 1, 28]);
    }

    #[test]
    fn the_release_frame_names_the_router_and_is_otherwise_identical() {
        let claim = arp_reply("f0:11:22:33:44:55", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).unwrap();
        let release = arp_release("f0:a9:51:c1:eb:da", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).unwrap();
        // The difference must be exactly the sender hardware address, which appears
        // twice: once in the Ethernet header and once in the ARP body. Nothing else
        // may differ, or the restore would not be the same request reversed.
        assert_ne!(&claim[6..12], &release[6..12], "ethernet source must differ");
        assert_ne!(&claim[22..28], &release[22..28], "arp sender hardware must differ");
        assert_eq!(&claim[22..28], &claim[6..12], "the two copies must agree");
        assert_eq!(&claim[0..6], &release[0..6], "destination unchanged");
        assert_eq!(&claim[12..22], &release[12..22], "ethertype and arp header unchanged");
        assert_eq!(&claim[28..], &release[28..], "claimed and target addresses unchanged");
        assert_eq!(&release[22..28], &[0xf0, 0xa9, 0x51, 0xc1, 0xeb, 0xda]);
    }

    #[test]
    fn a_bad_mac_produces_nothing_rather_than_a_malformed_frame() {
        assert!(arp_reply("not-a-mac", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).is_none());
        assert!(arp_reply("f0:11:22:33:44", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).is_none());
        assert!(arp_reply("f0:11:22:33:44:55", ip("192.168.1.1"), "aa:bb", ip("192.168.1.28")).is_none());
    }

    /// Step by step, so a failure names itself instead of arriving as an empty string.
    #[test]
    #[ignore]
    fn lan_route_diagnose() {
        println!("adapter: {:?}", adapter());
        match adapter() {
            Ok(ad) => {
                println!("  our mac {:?} gateway {:?} index {}", ad.mac, ad.gateway, ad.if_index);
                println!("  router mac for the gateway: {:?}", crate::lan::mac_for(&ad.gateway.to_string()));
                match Socket::open_for(ad.ip) {
                    Ok(sock) => {
                        println!("  pcap socket opened on {}", ad.ip);
                        match arp_reply(&ad.mac, ad.gateway, "82:d0:c2:a6:fe:c8", "192.168.1.55".parse().unwrap()) {
                            Some(f) => println!("  send test frame: {:?}", sock.send(&f).map(|_| "sent")),
                            None => println!("  could not build a frame"),
                        }
                        println!("  capture 3s from the device: {} frames",
                                 sock.count_frames_from("82:d0:c2:a6:fe:c8", 3));
                    }
                    Err(e) => println!("  pcap socket FAILED: {e:?}"),
                }
                println!("  set_forwarding(true): {:?}", set_forwarding(&ad.if_index.to_string(), true));
                println!("  observed: {:?}", forwarding_enabled(&ad.if_index.to_string()));
                println!("  set_forwarding(false): {:?}", set_forwarding(&ad.if_index.to_string(), false));
                println!("  observed: {:?}", forwarding_enabled(&ad.if_index.to_string()));
            }
            Err(e) => println!("  adapter FAILED: {e:?}"),
        }
    }

    /// The start-up repair: a record on disk means a previous run owed a restore.
    /// Ignored by default; run it deliberately to unwind a failed claim.
    #[test]
    #[ignore]
    fn lan_route_recover() {
        println!("intent before: {:?}", read_intent());
        match recover() {
            Some(msg) => println!("recover: {msg}"),
            None => println!("recover: nothing owed"),
        }
        println!("intent after: {:?}", read_intent());
        if let Ok(ad) = adapter() {
            println!("forwarding now: {:?}", forwarding_enabled(&ad.if_index.to_string()));
        }
    }

    /// The real thing, and ignored by default because it takes over a real device on a
    /// real LAN. Run it deliberately:
    ///
    ///   set LAN_ROUTE_TEST_TARGET=192.168.1.55 and LAN_ROUTE_TEST_MAC=82-d0-c2-a6-fe-c8
    ///   cargo test --lib -- --ignored lan_route_live
    #[test]
    #[ignore]
    fn lan_route_live() {
        let (Ok(target), Ok(mac)) = (
            std::env::var("LAN_ROUTE_TEST_TARGET"),
            std::env::var("LAN_ROUTE_TEST_MAC"),
        ) else {
            eprintln!("skipped: set LAN_ROUTE_TEST_TARGET and LAN_ROUTE_TEST_MAC");
            return;
        };
        if crate::vpn::task_engine_pid().is_none() {
            eprintln!("skipped: connect the tunnel first, the engine is not running");
            return;
        }
        let target: Ipv4Addr = target.parse().expect("target ip");
        let mac = mac.replace('-', ":").to_ascii_lowercase();
        let gateway: Ipv4Addr = std::env::var("LAN_ROUTE_TEST_GATEWAY")
            .unwrap_or_else(|_| "192.168.1.1".into())
            .parse()
            .expect("gateway");

        let socket = Socket::open_for(Adapter::discover_ip().expect("adapter")).expect("socket");
        let before = socket.count_frames_from(&mac, 3);
        println!("frames from the device in 3s BEFORE the claim: {before}");

        let claim = match route_device(target, &mac) {
            Ok(c) => c,
            Err(e) => {
                println!("CLAIM FAILED: {e}");
                panic!("claim failed: {e}");
            }
        };
        let if_idx = claim.adapter.if_index.to_string();
        println!("claimed {} via {}", target, claim.adapter.gateway);
        assert!(
            forwarding_enabled(&if_idx).expect("read back"),
            "forwarding must be on while a device is claimed"
        );

        let after = socket.count_frames_from(&mac, 8);
        println!("frames from the device in 8s AFTER the claim: {after}");

        drop(claim);
        // the guard released on drop; prove it did
        assert!(
            !forwarding_enabled(&if_idx).unwrap_or(false),
            "forwarding must be off after release"
        );
        assert!(read_intent().is_none(), "the intention record must be cleared");
        let restored = arp_release(
            &crate::lan::mac_for(&gateway.to_string()).expect("router mac"),
            gateway,
            &mac,
            target,
        );
        assert!(restored.is_some(), "the restore frame must build");
        println!("released; forwarding off, record cleared");
    }

    // The rule read-back decides whether a permit is real. It is pure, so it is tested here
    // rather than only in the field, and the case that matters is the permissive one: a rule
    // scoped to nothing must never pass as a rule scoped to the device.
    #[test]
    fn the_address_check_is_exact() {
        let ip: Ipv4Addr = "192.168.1.55".parse().unwrap();
        assert!(matches_ip("192.168.1.55", ip));
        assert!(matches_ip("192.168.1.55/32", ip));
        assert!(matches_ip("10.0.0.1, 192.168.1.55/32", ip), "a list containing it is scoped to it");

        assert!(!matches_ip("Any", ip), "a rule scoped to nothing is not scoped to the device");
        assert!(!matches_ip("any", ip));
        assert!(!matches_ip("Any, 10.0.0.1", ip), "the any check must be per part, not whole value");
        assert!(!matches_ip("10.0.0.1", ip));
        assert!(!matches_ip("192.168.1.5", ip), "a neighbour is not the device");
        assert!(!matches_ip("192.168.1.55/24", ip), "a subnet is not the device");
    }

    #[test]
    fn an_intention_round_trips_through_the_record() {
        let i = Intent {
            target_ip: "192.168.1.28".into(),
            target_mac: "aa:bb:cc:dd:ee:ff".into(),
            gateway_ip: "192.168.1.1".into(),
            interface: "Ethernet".into(),
            router_mac: "f0:a9:51:c1:eb:da".into(),
        };
        assert_eq!(Intent::decode(&i.encode()), Some(i));
    }

    #[test]
    fn a_damaged_record_is_not_mistaken_for_an_intention() {
        // Half a line would otherwise produce a device with no gateway to restore.
        for bad in ["", "1.2.3.4", "1.2.3.4|aa:bb", "a|b|c|", "|||", "a|b|c|d"] {
            assert_eq!(Intent::decode(bad), None, "accepted {bad:?}");
        }
    }
}
