//! Device names, asked directly.
//!
//! Reverse DNS is empty on most home routers, so a device that answers ping
//! still shows up as an address. These two queries ask the device itself:
//! NetBIOS for Windows machines, and mDNS for phones, printers and Macs.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

/// The NetBIOS wildcard name, encoded the way the protocol wants it: 16 bytes
/// padded with zeros, each byte split into two nibbles offset by 'A'.
fn encode_wildcard() -> Vec<u8> {
    let mut name = [0u8; 16];
    name[0] = b'*';
    let mut out = Vec::with_capacity(34);
    out.push(32u8);
    for b in name {
        out.push(b'A' + (b >> 4));
        out.push(b'A' + (b & 0x0f));
    }
    out.push(0);
    out
}

/// Ask one address for its NetBIOS computer name.
pub fn netbios(ip: Ipv4Addr, wait: Duration) -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(wait)).ok()?;
    let mut pkt = vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    pkt.extend_from_slice(&encode_wildcard());
    pkt.extend_from_slice(&[0x00, 0x21, 0x00, 0x01]);
    sock.send_to(&pkt, SocketAddr::new(IpAddr::V4(ip), 137)).ok()?;

    let mut buf = [0u8; 1024];
    let (n, _) = sock.recv_from(&mut buf).ok()?;
    let data = &buf[..n];
    // Header is 12 bytes, then the echoed question: 34 bytes of name plus 4.
    let mut i = 12 + 34 + 4;
    if i + 12 > data.len() {
        return None;
    }
    // Answer header: name pointer, type, class, ttl, rdlength.
    i += 12;
    if i >= data.len() {
        return None;
    }
    let count = data[i] as usize;
    i += 1;
    for _ in 0..count {
        if i + 18 > data.len() {
            return None;
        }
        let entry = &data[i..i + 18];
        let flags = u16::from_be_bytes([entry[16], entry[17]]);
        let is_group = flags & 0x8000 != 0;
        if !is_group {
            let raw = &entry[..15];
            let name: String = raw
                .iter()
                .take_while(|b| **b != 0 && **b != b' ')
                .map(|b| *b as char)
                .collect();
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
        i += 18;
    }
    None
}

/// Decode a DNS name that may use compression pointers.
fn read_name(data: &[u8], mut i: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut jumped = false;
    let mut end = i;
    let mut hops = 0;
    loop {
        if i >= data.len() || hops > 16 {
            return None;
        }
        let len = data[i] as usize;
        if len == 0 {
            if !jumped {
                end = i + 1;
            }
            break;
        }
        if len & 0xc0 == 0xc0 {
            if i + 1 >= data.len() {
                return None;
            }
            let ptr = (((len & 0x3f) as usize) << 8) | data[i + 1] as usize;
            if !jumped {
                end = i + 2;
            }
            jumped = true;
            i = ptr;
            hops += 1;
            continue;
        }
        if i + 1 + len > data.len() {
            return None;
        }
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&String::from_utf8_lossy(&data[i + 1..i + 1 + len]));
        i += 1 + len;
    }
    Some((out, end))
}

/// Ask the multicast resolver what a device calls itself.
pub fn mdns(ip: Ipv4Addr, wait: Duration) -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(wait)).ok()?;
    let rev = format!(
        "{}.{}.{}.{}.in-addr.arpa",
        ip.octets()[3],
        ip.octets()[2],
        ip.octets()[1],
        ip.octets()[0]
    );
    let mut pkt: Vec<u8> = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    for label in rev.split('.') {
        pkt.push(label.len() as u8);
        pkt.extend_from_slice(label.as_bytes());
    }
    pkt.push(0);
    pkt.extend_from_slice(&[0x00, 0x0c, 0x00, 0x01]); // PTR IN
    sock.send_to(&pkt, SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353))
        .ok()?;

    let mut buf = [0u8; 2048];
    let (n, _) = sock.recv_from(&mut buf).ok()?;
    let data = &buf[..n];
    if n < 12 {
        return None;
    }
    let qd = u16::from_be_bytes([data[4], data[5]]) as usize;
    let an = u16::from_be_bytes([data[6], data[7]]) as usize;
    let mut i = 12;
    for _ in 0..qd {
        let (_name, next) = read_name(data, i)?;
        i = next + 4;
    }
    for _ in 0..an {
        let (_name, next) = read_name(data, i)?;
        i = next;
        if i + 10 > data.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([data[i], data[i + 1]]);
        let rdlen = u16::from_be_bytes([data[i + 8], data[i + 9]]) as usize;
        i += 10;
        if i + rdlen > data.len() {
            return None;
        }
        if rtype == 12 {
            if let Some((target, _)) = read_name(data, i) {
                let clean = target.trim_end_matches('.').to_string();
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
        }
        i += rdlen;
    }
    None
}


/// Listen to the multicast resolver for a moment and keep every address record
/// it announces: phones, TVs, printers and Macs say "name.local is 192.168.1.x"
/// without being asked, which is the only name source that works on a network
/// where reverse DNS and NetBIOS both stay silent.
pub fn harvest(wait: Duration) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    use std::net::UdpSocket;

    let mut out: HashMap<String, String> = HashMap::new();
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
        return out;
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(400)));

    // QU bit set: responders answer to the source port instead of the group,
    // so no multicast membership and no port 5353 binding is needed.
    let mut pkt: Vec<u8> = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    for label in "_services._dns-sd._udp.local".split('.') {
        pkt.push(label.len() as u8);
        pkt.extend_from_slice(label.as_bytes());
    }
    pkt.push(0);
    pkt.extend_from_slice(&[0x00, 0x0c, 0x80, 0x01]);
    let _ = sock.send_to(&pkt, SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353));

    let deadline = std::time::Instant::now() + wait;
    let mut buf = [0u8; 4096];
    while std::time::Instant::now() < deadline {
        let Ok((n, _)) = sock.recv_from(&mut buf) else {
            continue;
        };
        let data = &buf[..n];
        if n < 12 {
            continue;
        }
        let qd = u16::from_be_bytes([data[4], data[5]]) as usize;
        let an = u16::from_be_bytes([data[6], data[7]]) as usize;
        let mut i = 12;
        let mut ok = true;
        for _ in 0..qd {
            match read_name(data, i) {
                Some((_, next)) => i = next + 4,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        for _ in 0..an {
            let Some((name, next)) = read_name(data, i) else { break };
            i = next;
            if i + 10 > data.len() {
                break;
            }
            let rtype = u16::from_be_bytes([data[i], data[i + 1]]);
            let rdlen = u16::from_be_bytes([data[i + 8], data[i + 9]]) as usize;
            i += 10;
            if i + rdlen > data.len() {
                break;
            }
            if rtype == 1 && rdlen == 4 {
                let ip = format!("{}.{}.{}.{}", data[i], data[i + 1], data[i + 2], data[i + 3]);
                let clean = name.trim_end_matches('.').trim_end_matches(".local").to_string();
                if !clean.is_empty() {
                    out.entry(ip).or_insert(clean);
                }
            }
            i += rdlen;
        }
    }
    out
}
