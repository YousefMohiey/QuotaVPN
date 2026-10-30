//! Names the devices give themselves.
//!
//! NetBIOS and mDNS only answer if the device chooses to, and phones usually do
//! not. DHCP is different: every device that joins a network announces its name
//! to the router as part of asking for an address, in a packet that is broadcast
//! so everyone on the wire can read it. Listening for that is the one name
//! source that works on any router, without an admin login.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Where the names we have heard are kept, so a name learned yesterday still
/// shows today without waiting for the device to rejoin.
fn cache_path() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    base.join("quotavpn").join("lan_names.txt")
}

pub fn cached() -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(text) = std::fs::read_to_string(cache_path()) else {
        return out;
    };
    for line in text.lines() {
        if let Some((mac, name)) = line.split_once('|') {
            if !mac.is_empty() && !name.is_empty() {
                out.insert(mac.to_lowercase(), name.to_string());
            }
        }
    }
    out
}

fn remember(found: &HashMap<String, String>) {
    let mut all = cached();
    let before = all.len();
    for (mac, name) in found {
        all.insert(mac.clone(), name.clone());
    }
    if all.len() == before {
        return;
    }
    let path = cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body: String = all
        .iter()
        .map(|(mac, name)| format!("{mac}|{name}\n"))
        .collect();
    let _ = std::fs::write(path, body);
}

fn mac_of(frame: &[u8]) -> Option<(String, String)> {
    if frame.len() < 14 || frame[12] != 0x08 || frame[13] != 0x00 {
        return None;
    }
    let ihl = (frame[14] & 0x0f) as usize * 4;
    if frame.len() < 14 + ihl + 8 || frame[14 + 9] != 17 {
        return None;
    }
    let udp = 14 + ihl;
    let dst_port = u16::from_be_bytes([frame[udp + 2], frame[udp + 3]]);
    if dst_port != 67 {
        return None;
    }
    let bootp = udp + 8;
    if frame.len() < bootp + 240 {
        return None;
    }
    // Only client messages carry a hostname, and only they are broadcast.
    if frame[bootp] != 1 {
        return None;
    }
    if frame[bootp + 236..bootp + 240] != [0x63, 0x82, 0x53, 0x63] {
        return None;
    }
    let chaddr = &frame[bootp + 28..bootp + 34];
    let mac = chaddr
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let mut i = bootp + 240;
    while i + 1 < frame.len() {
        let code = frame[i];
        if code == 255 {
            break;
        }
        let len = frame[i + 1] as usize;
        if i + 2 + len > frame.len() {
            break;
        }
        if code == 12 && len > 0 {
            let raw = &frame[i + 2..i + 2 + len];
            let name = String::from_utf8_lossy(raw).trim().to_string();
            if !name.is_empty() {
                return Some((mac, name));
            }
        }
        i += 2 + len;
    }
    None
}

/// Read a name out of an mDNS answer: a device that publishes an address
/// record is telling the whole network its own name.
fn mdns_name(frame: &[u8]) -> Option<(String, String)> {
    if frame.len() < 14 || frame[12] != 0x08 || frame[13] != 0x00 {
        return None;
    }
    let ihl = (frame[14] & 0x0f) as usize * 4;
    if frame.len() < 14 + ihl + 8 || frame[14 + 9] != 17 {
        return None;
    }
    let udp = 14 + ihl;
    let src_port = u16::from_be_bytes([frame[udp], frame[udp + 1]]);
    let dst_port = u16::from_be_bytes([frame[udp + 2], frame[udp + 3]]);
    if src_port != 5353 && dst_port != 5353 {
        return None;
    }
    let dns = udp + 8;
    if frame.len() < dns + 12 {
        return None;
    }
    let ancount = u16::from_be_bytes([frame[dns + 6], frame[dns + 7]]) as usize;
    let qdcount = u16::from_be_bytes([frame[dns + 4], frame[dns + 5]]) as usize;
    let mut i = dns + 12;
    // Skip the questions, then walk the answers looking for an A record.
    for _ in 0..qdcount {
        i = skip_name(frame, i)?;
        i += 4;
    }
    for _ in 0..ancount {
        let name = read_name(frame, i)?;
        i = skip_name(frame, i)?;
        if i + 10 > frame.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([frame[i], frame[i + 1]]);
        let rdlen = u16::from_be_bytes([frame[i + 8], frame[i + 9]]) as usize;
        i += 10;
        if i + rdlen > frame.len() {
            return None;
        }
        if rtype == 1 && rdlen == 4 {
            let ip = format!("{}.{}.{}.{}", frame[i], frame[i + 1], frame[i + 2], frame[i + 3]);
            let clean = name.trim_end_matches('.').trim_end_matches(".local").to_string();
            if !clean.is_empty() {
                return Some((ip, clean));
            }
        }
        i += rdlen;
    }
    None
}

fn skip_name(frame: &[u8], mut i: usize) -> Option<usize> {
    loop {
        let len = *frame.get(i)? as usize;
        if len == 0 {
            return Some(i + 1);
        }
        if len & 0xc0 == 0xc0 {
            return Some(i + 2);
        }
        i += 1 + len;
    }
}

fn read_name(frame: &[u8], start: usize) -> Option<String> {
    let mut out = String::new();
    let mut i = start;
    let mut hops = 0;
    loop {
        let len = *frame.get(i)? as usize;
        if len == 0 {
            break;
        }
        if len & 0xc0 == 0xc0 {
            let ptr = (((len & 0x3f) as usize) << 8) | *frame.get(i + 1)? as usize;
            hops += 1;
            if hops > 8 {
                break;
            }
            i = ptr;
            continue;
        }
        let label = frame.get(i + 1..i + 1 + len)?;
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&String::from_utf8_lossy(label));
        i += 1 + len;
    }
    Some(out)
}

/// Listen for the duration given and return whatever names were announced.
pub fn listen(secs: u64) -> HashMap<String, String> {
    let mut found = HashMap::new();
    let Some(wire) = crate::lan_ctrl::wire() else {
        return found;
    };
    let Some(Some(cap)) = crate::wpcap::with(|w| w.open(&wire.device)) else {
        return found;
    };
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut by_ip: HashMap<String, String> = HashMap::new();
    while Instant::now() < deadline {
        let Some(frame) = cap.next() else { continue };
        if let Some((mac, name)) = mac_of(&frame) {
            found.insert(mac, name);
        }
        if let Some((ip, name)) = mdns_name(&frame) {
            by_ip.insert(ip, name);
        }
    }
    remember(&found);
    if !by_ip.is_empty() {
        crate::lan::remember_probed(&by_ip);
    }
    found
}

/// Keep listening in the background. A device only announces itself when it
/// joins, so the honest way to know names is to be listening when it does.
pub fn start_background() {
    std::thread::spawn(|| loop {
        let found = listen(120);
        let _ = found;
        std::thread::sleep(Duration::from_secs(5));
    });
}

/// Read the name out of a DNS answer: a PTR target if there is one, otherwise
/// the owner of an address record. Used for questions this app asked itself.
pub fn answer_name(frame: &[u8], _port: u16) -> Option<String> {
    if frame.len() < 14 || frame[12] != 0x08 || frame[13] != 0x00 {
        return None;
    }
    let ihl = (frame[14] & 0x0f) as usize * 4;
    if frame.len() < 14 + ihl + 8 || frame[14 + 9] != 17 {
        return None;
    }
    let dns = 14 + ihl + 8;
    if frame.len() < dns + 12 {
        return None;
    }
    let qdcount = u16::from_be_bytes([frame[dns + 4], frame[dns + 5]]) as usize;
    let ancount = u16::from_be_bytes([frame[dns + 6], frame[dns + 7]]) as usize;
    let mut i = dns + 12;
    for _ in 0..qdcount {
        i = skip_name(frame, i)?;
        i += 4;
    }
    let mut fallback = String::new();
    for _ in 0..ancount {
        let owner = read_name(frame, i)?;
        i = skip_name(frame, i)?;
        if i + 10 > frame.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([frame[i], frame[i + 1]]);
        let rdlen = u16::from_be_bytes([frame[i + 8], frame[i + 9]]) as usize;
        i += 10;
        if i + rdlen > frame.len() {
            return None;
        }
        if rtype == 12 {
            if let Some(target) = read_name(frame, i) {
                let clean = target.trim_end_matches('.').trim_end_matches(".local").to_string();
                if !clean.is_empty() && clean.split('.').count() <= 3 {
                    return Some(clean);
                }
            }
        }
        if rtype == 1 && fallback.is_empty() {
            fallback = owner.trim_end_matches('.').trim_end_matches(".local").to_string();
        }
        i += rdlen;
    }
    if fallback.is_empty() {
        None
    } else {
        Some(fallback)
    }
}
