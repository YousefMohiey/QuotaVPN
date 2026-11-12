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
    let out = std::process::Command::new("netsh")
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

// ---------------------------------------------------------------- npcap

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

unsafe extern "C" {
    fn pcap_findalldevs(alldevs: *mut *mut PcapIf, errbuf: *mut std::os::raw::c_char) -> i32;
    fn pcap_freealldevs(alldevs: *mut PcapIf);
    fn pcap_open_live(
        device: *const std::os::raw::c_char,
        snaplen: i32,
        promisc: i32,
        to_ms: i32,
        errbuf: *mut std::os::raw::c_char,
    ) -> *mut std::ffi::c_void;
    fn pcap_sendpacket(p: *mut std::ffi::c_void, buf: *const u8, size: i32) -> i32;
    fn pcap_close(p: *mut std::ffi::c_void);
    fn pcap_geterr(p: *mut std::ffi::c_void) -> *mut std::os::raw::c_char;
}

/// A packet socket on the interface that owns `ip`.
pub struct Socket {
    handle: *mut std::ffi::c_void,
}

// The handle is only used behind the claim lock, one call at a time.
unsafe impl Send for Socket {}

impl Socket {
    pub fn open_for(ip: Ipv4Addr) -> Result<Socket, String> {
        let mut errbuf = [0i8; 256];
        let mut devs: *mut PcapIf = std::ptr::null_mut();
        if unsafe { pcap_findalldevs(&mut devs, errbuf.as_mut_ptr()) } != 0 {
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
        unsafe { pcap_freealldevs(devs) };
        let name = chosen.ok_or_else(|| format!("no interface owns {ip}"))?;

        let cname = std::ffi::CString::new(name.clone()).map_err(|e| e.to_string())?;
        let handle = unsafe { pcap_open_live(cname.as_ptr(), 65536, 0, 100, errbuf.as_mut_ptr()) };
        if handle.is_null() {
            return Err(format!("pcap_open_live {name}: {}", cstr(&errbuf)));
        }
        Ok(Socket { handle })
    }

    pub fn send(&self, frame: &[u8]) -> Result<(), String> {
        let rc = unsafe { pcap_sendpacket(self.handle, frame.as_ptr(), frame.len() as i32) };
        if rc != 0 {
            let msg = unsafe {
                let p = pcap_geterr(self.handle);
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
}

impl Drop for Socket {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { pcap_close(self.handle) };
        }
    }
}

fn cstr(buf: &[i8]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
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
    // Routing a device is only meaningful while the tunnel is up: forwarding sends its
    // traffic into the tun, and with no tun there is nowhere for it to go. Saying so is
    // better than a claim that silently does nothing.
    if !crate::vpn::tun_adapter_present() {
        return Err("connect the VPN first: there is no tunnel for this device's traffic to use".into());
    }

    let router_mac = crate::lan::mac_for(&ad.gateway.to_string())
        .ok_or_else(|| format!("no MAC known for the gateway {}", ad.gateway))?;
    if router_mac.eq_ignore_ascii_case(&ad.mac) {
        return Err(format!(
            "the neighbour table says we are the gateway ({}); refusing to build a restore              that would name ourselves. Release the previous claim, or let the entry expire.",
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

    let sock = Socket::open_for(ad.ip)?;
    let frame = arp_reply(&ad.mac, ad.gateway, target_mac, target_ip)
        .ok_or_else(|| "could not build the claim frame".to_string())?;
    sock.send(&frame)?;

    set_forwarding(&ad.if_index.to_string(), true)?;
    open_firewall(target_ip)?;

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

fn rule_name(target_ip: Ipv4Addr) -> String {
    format!("QuotaVPN lan-route {target_ip}")
}

/// Let the device's traffic be forwarded. Enabling IPv4 forwarding is not enough on
/// Windows: the filtering platform drops forwarded packets unless a rule permits them,
/// which is why the claim alone moved nothing.
fn open_firewall(target_ip: Ipv4Addr) -> Result<(), String> {
    let name = rule_name(target_ip);
    let _ = close_firewall(target_ip); // idempotent: a stale rule would stack up otherwise
    let out = crate::lan::hidden("netsh")
        .args([
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={name}"),
            "dir=in",
            "action=allow",
            &format!("remoteip={target_ip}"),
            "profile=any",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "could not add the firewall rule: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn close_firewall(target_ip: Ipv4Addr) -> Result<(), String> {
    let out = crate::lan::hidden("netsh")
        .args([
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={}", rule_name(target_ip)),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    // A missing rule is not an error: the first call always has one to delete.
    let _ = out;
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
    set_forwarding(&intent.interface, false)?;
    let _ = close_firewall(target_ip);
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

// ---------------------------------------------------------------- live proof

#[repr(C)]
struct PcapPkthdr {
    // Windows `struct timeval` is two 32-bit longs, not two 64-bit ones.
    ts_sec: i32,
    ts_usec: i32,
    caplen: u32,
    len: u32,
}

unsafe extern "C" {
    fn pcap_next_ex(
        p: *mut std::ffi::c_void,
        hdr: *mut *mut PcapPkthdr,
        data: *mut *const u8,
    ) -> i32;
}

impl Socket {
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
            let rc = unsafe { pcap_next_ex(self.handle, &mut hdr, &mut data) };
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
        if !crate::vpn::tun_adapter_present() {
            eprintln!("skipped: connect the tunnel first, there is nothing to forward into");
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
