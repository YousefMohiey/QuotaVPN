//! Hand out DHCP leases so a chosen device uses this PC as its gateway.
//!
//! Phones refuse forged ARP, so the only honest way to steer one is to be the
//! server that tells it where the gateway is. DHCP has no authentication: a
//! client takes the first OFFER it hears, and it must obey the gateway in the
//! lease. That makes this the one mechanism that works on every phone without
//! touching the phone.
//!
//! One switch on the router is required: its own DHCP server has to be off, or
//! it races us. Everything else is per device here: a MAC in `routed` gets this
//! PC as its gateway, every other MAC is told the real router, so the rest of
//! the house is untouched.
//!
//! The wire format is built by hand: 240 fixed bytes, a magic cookie and TLV
//! options. That is small enough to own outright and avoids a dependency on a
//! crate whose API would have to be learned twice.

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const MAGIC: [u8; 4] = [99, 130, 83, 99];
const BOOTP_LEN: usize = 240;

const OPT_SUBNET: u8 = 1;
const OPT_ROUTER: u8 = 3;
const OPT_DNS: u8 = 6;
const OPT_REQUESTED: u8 = 50;
const OPT_LEASE: u8 = 51;
const OPT_MSG: u8 = 53;
const OPT_SERVER_ID: u8 = 54;
const OPT_BROADCAST: u8 = 28;
const OPT_END: u8 = 255;

const DISCOVER: u8 = 1;
const OFFER: u8 = 2;
const REQUEST: u8 = 3;
const ACK: u8 = 5;
const NAK: u8 = 6;

#[derive(Clone, Debug)]
pub struct Settings {
    /// This machine, the address the lease points at.
    pub server: Ipv4Addr,
    /// The router, given to every device that is not being routed.
    pub gateway: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub dns: Ipv4Addr,
    /// Inclusive range of last octets handed to unrouted devices.
    pub pool: (u8, u8),
}

impl Settings {
    pub fn detected() -> Result<Settings, String> {
        let ad = crate::lan_route::adapter()?;
        // Mask and DNS come from the adapter's own view of the network.
        let mask = std::net::Ipv4Addr::new(255, 255, 255, 0);
        Ok(Settings {
            server: ad.ip,
            gateway: ad.gateway,
            mask,
            dns: ad.gateway,
            pool: (200, 249),
        })
    }
}


/// Bind udp/67 with SO_REUSEADDR.
///
/// Windows' own DHCP and ICS stacks hold 67 whenever the hotspot or sharing has
/// ever been on, and a plain bind then fails with 10048 even with nothing
/// visible in netstat. Sharing the address is the documented answer for a DHCP
/// server, so the socket is created by hand, the option is set before bind, and
/// the handle is handed back to std.
fn bind_reusable(port: u16) -> Result<UdpSocket, String> {
    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn socket(af: i32, ty: i32, proto: i32) -> usize;
        fn setsockopt(s: usize, level: i32, name: i32, val: *const u8, len: i32) -> i32;
        fn bind(s: usize, name: *const u8, len: i32) -> i32;
        fn closesocket(s: usize) -> i32;
    }
    const AF_INET: i32 = 2;
    const SOCK_DGRAM: i32 = 2;
    const IPPROTO_UDP: i32 = 17;
    const SOL_SOCKET: i32 = 0xffff;
    const SO_REUSEADDR: i32 = 0x0004;
    const SO_BROADCAST: i32 = 0x0020;
    const INVALID: usize = usize::MAX;

    // Make sure winsock is initialised before the raw call.
    let _ = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0));

    let mut last = String::from("no attempt");
    for _ in 0..5 {
        unsafe {
            let s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
            if s == INVALID {
                last = "socket() returned an invalid handle".to_string();
                std::thread::sleep(std::time::Duration::from_millis(250));
                continue;
            }
            let one: i32 = 1;
            setsockopt(s, SOL_SOCKET, SO_REUSEADDR, &one as *const i32 as *const u8, 4);
            setsockopt(s, SOL_SOCKET, SO_BROADCAST, &one as *const i32 as *const u8, 4);
            // sockaddr_in: family, port (big endian), address, padding
            let mut sa = [0u8; 16];
            sa[0..2].copy_from_slice(&(AF_INET as u16).to_ne_bytes());
            sa[2..4].copy_from_slice(&port.to_be_bytes());
            if bind(s, sa.as_ptr(), 16) == 0 {
                use std::os::windows::io::FromRawSocket;
                return Ok(UdpSocket::from_raw_socket(s as std::os::windows::raw::SOCKET));
            }
            let why = std::io::Error::last_os_error();
            closesocket(s);
            last = format!("bind udp/{port}: {why}");
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
    Err(format!("{last} (another DHCP server is running?)"))
}

fn mac_of(pkt: &[u8]) -> Option<[u8; 6]> {
    if pkt.len() < 28 || pkt[0] != 1 {
        return None;
    }
    let mut m = [0u8; 6];
    m.copy_from_slice(&pkt[28..34]);
    Some(m)
}

fn xid_of(pkt: &[u8]) -> Option<[u8; 4]> {
    if pkt.len() < 8 {
        return None;
    }
    let mut x = [0u8; 4];
    x.copy_from_slice(&pkt[4..8]);
    Some(x)
}

fn options(pkt: &[u8]) -> Option<HashMap<u8, Vec<u8>>> {
    if pkt.len() < BOOTP_LEN || pkt[236..240] != MAGIC {
        return None;
    }
    let mut out = HashMap::new();
    let mut i = BOOTP_LEN;
    while i < pkt.len() {
        let code = pkt[i];
        if code == OPT_END || code == 0 {
            break;
        }
        if i + 1 >= pkt.len() {
            break;
        }
        let len = pkt[i + 1] as usize;
        if i + 2 + len > pkt.len() {
            break;
        }
        out.insert(code, pkt[i + 2..i + 2 + len].to_vec());
        i += 2 + len;
    }
    Some(out)
}

fn msg_type(opts: &HashMap<u8, Vec<u8>>) -> Option<u8> {
    opts.get(&OPT_MSG).and_then(|v| v.first().copied())
}

fn push_opt(buf: &mut Vec<u8>, code: u8, data: &[u8]) {
    buf.push(code);
    buf.push(data.len() as u8);
    buf.extend_from_slice(data);
}

#[allow(clippy::too_many_arguments)]
fn reply(
    xid: [u8; 4],
    mac: [u8; 6],
    kind: u8,
    offered: Ipv4Addr,
    s: &Settings,
    gateway: Ipv4Addr,
    lease_secs: u32,
) -> Vec<u8> {
    let mut b = vec![0u8; BOOTP_LEN];
    b[0] = 2; // BOOTREPLY
    b[1] = 1; // ethernet
    b[2] = 6; // hlen
    b[4..8].copy_from_slice(&xid);
    b[10..12].copy_from_slice(&0x8000u16.to_be_bytes()); // broadcast
    b[16..20].copy_from_slice(&offered.octets()); // yiaddr
    b[20..24].copy_from_slice(&s.server.octets()); // siaddr
    b[28..34].copy_from_slice(&mac);
    b[236..240].copy_from_slice(&MAGIC);

    push_opt(&mut b, OPT_MSG, &[kind]);
    push_opt(&mut b, OPT_SERVER_ID, &s.server.octets());
    push_opt(&mut b, OPT_LEASE, &lease_secs.to_be_bytes());
    push_opt(&mut b, OPT_SUBNET, &s.mask.octets());
    push_opt(&mut b, OPT_ROUTER, &gateway.octets());
    push_opt(&mut b, OPT_DNS, &s.dns.octets());
    push_opt(&mut b, OPT_BROADCAST, &Ipv4Addr::BROADCAST.octets());
    b.push(OPT_END);
    while b.len() < 300 {
        b.push(0);
    }
    b
}

fn last_octet(ip: Ipv4Addr) -> u8 {
    ip.octets()[3]
}

/// Serve until `stop` is raised. Never returns an error for a bad packet: a
/// malformed frame from the wire is not a reason to stop answering the network.
pub fn serve(
    routed: HashMap<[u8; 6], Ipv4Addr>,
    stop: Arc<AtomicBool>,
    log: impl Fn(&str) + Send + Sync + 'static,
) -> Result<(), String> {
    let s = Settings::detected()?;
    // A plain bind first: if it succeeds nobody else holds the port, and a
    // second copy of this server fails loudly instead of fighting over leases.
    let sock = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 67)) {
        Ok(s) => s,
        Err(e) => {
            log(&format!("dhcp: udp/67 busy ({e}), sharing with the system stack"));
            bind_reusable(67)?
        }
    };
    sock.set_broadcast(true).map_err(|e| e.to_string())?;
    log(&format!(
        "dhcp: serving as {} gateway {} ({} routed device(s))",
        s.server,
        s.gateway,
        routed.len()
    ));

    let mut buf = [0u8; 1500];
    while !stop.load(Ordering::Relaxed) {
        let (n, _from) = match sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let pkt = &buf[..n];
        let (Some(mac), Some(xid), Some(opts)) = (mac_of(pkt), xid_of(pkt), options(pkt)) else {
            continue;
        };
        let Some(kind) = msg_type(&opts) else { continue };
        if kind != DISCOVER && kind != REQUEST {
            continue;
        }

        // A routed device is told to use this PC; everyone else keeps the router.
        let (offered, gateway) = match routed.get(&mac) {
            Some(ip) => (*ip, s.server),
            None => {
                let span = (s.pool.1.saturating_sub(s.pool.0) as usize) + 1;
                let pick = (mac[5] as usize) % span;
                let mut o = s.server.octets();
                o[3] = s.pool.0 + pick as u8;
                (Ipv4Addr::from(o), s.gateway)
            }
        };

        if kind == REQUEST {
            // Honour a request for anything we offered, otherwise refuse: a NAK
            // makes the client start over and hear our OFFER next time.
            // A request for an address we did not offer is not ours to refuse.
            // NAKing it is what made a phone give up with "no IP allocation" when
            // two servers were briefly on the wire: each refused the other's lease.
            if let Some(want) = opts.get(&OPT_REQUESTED) {
                if want.len() == 4 && want[..] != offered.octets()[..] {
                    continue;
                }
            }
            let ack = reply(xid, mac, ACK, offered, &s, gateway, 3600);
            let _ = sock.send_to(&ack, (Ipv4Addr::BROADCAST, 68));
            log(&format!(
                "dhcp: ACK {offered} -> {:02x?} via {gateway}",
                mac
            ));
        } else {
            let offer = reply(xid, mac, OFFER, offered, &s, gateway, 3600);
            let _ = sock.send_to(&offer, (Ipv4Addr::BROADCAST, 68));
            log(&format!(
                "dhcp: OFFER {offered} -> {:02x?} via {gateway}",
                mac
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            server: Ipv4Addr::new(192, 168, 1, 52),
            gateway: Ipv4Addr::new(192, 168, 1, 1),
            mask: Ipv4Addr::new(255, 255, 255, 0),
            dns: Ipv4Addr::new(192, 168, 1, 1),
            pool: (200, 249),
        }
    }

    #[test]
    fn a_routed_device_is_offered_this_pc_as_gateway() {
        let s = settings();
        let ip = Ipv4Addr::new(192, 168, 1, 28);
        let pkt = reply([1, 2, 3, 4], [0x42, 0xd5, 0x2a, 0x0c, 0xbc, 0x0a], OFFER, ip, &s, s.server, 3600);
        let opts = options(&pkt).unwrap();
        assert_eq!(opts[&OPT_ROUTER], s.server.octets().to_vec());
        assert_eq!(opts[&OPT_MSG], vec![OFFER]);
    }

    #[test]
    fn everyone_else_keeps_the_real_router() {
        let s = settings();
        let ip = Ipv4Addr::new(192, 168, 1, 200);
        let pkt = reply([9, 9, 9, 9], [0, 1, 2, 3, 4, 5], ACK, ip, &s, s.gateway, 3600);
        let opts = options(&pkt).unwrap();
        assert_eq!(opts[&OPT_ROUTER], s.gateway.octets().to_vec());
    }
}
