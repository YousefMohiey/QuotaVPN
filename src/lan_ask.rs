//! Asking a device its name, on the wire.
//!
//! A normal socket has to get past the Windows firewall, and a reply that
//! arrives unsolicited is often dropped. The packet driver does not have that
//! problem: it builds the question, puts it on the wire addressed to the
//! device's own hardware address, and reads the answer as it comes back.

use crate::lan_ctrl::wire;
use crate::wpcap;
use std::time::{Duration, Instant};

fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

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

fn ip4(s: &str) -> Option<[u8; 4]> {
    let p: Vec<u8> = s.split('.').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 4 {
        Some([p[0], p[1], p[2], p[3]])
    } else {
        None
    }
}

/// Ethernet + IPv4 + UDP, with the checksum the network stack would have made.
fn udp_frame(src_mac: [u8; 6], dst_mac: [u8; 6], src_ip: [u8; 4], dst_ip: [u8; 4], sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let udp_len = 8 + payload.len();
    let mut udp = Vec::with_capacity(udp_len);
    udp.extend_from_slice(&sport.to_be_bytes());
    udp.extend_from_slice(&dport.to_be_bytes());
    udp.extend_from_slice(&(udp_len as u16).to_be_bytes());
    udp.extend_from_slice(&[0, 0]);
    udp.extend_from_slice(payload);

    let total = 20 + udp.len();
    let mut ip = Vec::with_capacity(total);
    ip.extend_from_slice(&[0x45, 0x00]);
    ip.extend_from_slice(&(total as u16).to_be_bytes());
    ip.extend_from_slice(&[0x00, 0x01, 0x40, 0x00]);
    ip.extend_from_slice(&[64, 17]);
    ip.extend_from_slice(&[0, 0]);
    ip.extend_from_slice(&src_ip);
    ip.extend_from_slice(&dst_ip);
    let sum = checksum(&ip);
    ip[10..12].copy_from_slice(&sum.to_be_bytes());
    ip.extend_from_slice(&udp);

    let mut frame = Vec::with_capacity(14 + ip.len());
    frame.extend_from_slice(&dst_mac);
    frame.extend_from_slice(&src_mac);
    frame.extend_from_slice(&[0x08, 0x00]);
    frame.extend_from_slice(&ip);
    frame
}

/// A reverse lookup question for one address, asking for a unicast answer.
fn mdns_ptr_query(ip: [u8; 4]) -> Vec<u8> {
    let rev = format!("{}.{}.{}.{}.in-addr.arpa", ip[3], ip[2], ip[1], ip[0]);
    let mut q = Vec::new();
    q.extend_from_slice(&[0, 0]); // id
    q.extend_from_slice(&[0, 0]); // standard query
    q.extend_from_slice(&[0, 1]); // one question
    q.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    for label in rev.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0, 12]); // PTR
    q.extend_from_slice(&[0x80, 0x01]); // IN + unicast reply
    q
}

/// Send a question on the wire and read whatever answers within the wait.
fn ask(ip: &str, mac: &str, payload: Vec<u8>, sport: u16, dport: u16, wait: Duration) -> Option<Vec<u8>> {
    let w = wire()?;
    let our_mac = mac_bytes(&w.mac)?;
    let our_ip = ip4(&w.ip)?;
    let dst_mac = mac_bytes(mac)?;
    let dst_ip = ip4(ip)?;
    let frame = udp_frame(our_mac, dst_mac, our_ip, dst_ip, sport, dport, &payload);

    let cap = wpcap::with(|p| p.open(&w.device))??;
    let deadline = Instant::now() + wait;
    let mut tries = 0;
    let mut sent_at = Instant::now() - Duration::from_secs(1);
    while Instant::now() < deadline {
        if tries < 2 && sent_at.elapsed() > Duration::from_millis(220) {
            let _ = cap.send(&frame);
            tries += 1;
            sent_at = Instant::now();
        }
        let Some(f) = cap.next() else { continue };
        // Only what this device sent back to us.
        if f.len() < 14 || &f[0..6] != our_mac {
            continue;
        }
        if &f[6..12] != dst_mac {
            continue;
        }
        if let Some(name) = crate::dhcp_names::answer_name(&f, dport) {
            return Some(name.into_bytes());
        }
    }
    None
}

/// Ask one device for its own name over mDNS.
pub fn mdns_ask(ip: &str, mac: &str, wait: Duration) -> Option<String> {
    let query = mdns_ptr_query(ip4(ip)?);
    let answer = ask(ip, mac, query, 5353, 5353, wait)?;
    Some(String::from_utf8_lossy(&answer).to_string())
}
