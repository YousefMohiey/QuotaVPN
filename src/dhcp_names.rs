//! Names the devices give themselves.
//!
//! NetBIOS and mDNS only answer if the device chooses to, and phones usually do
//! not. DHCP is different: every device that joins a network announces its name
//! to the router as part of asking for an address, in a packet that is broadcast
//! so everyone on the wire can read it.
//!
//! This used to eavesdrop on those broadcasts through Npcap. It now uses plain
//! sockets, and the two legs have very different privilege needs:
//!
//! * the DHCP leg binds UDP 67, which needs an elevated process and a machine
//!   with no DHCP server of its own, so it is opportunistic: when it fails the
//!   other legs still run;
//! * the mDNS leg asks each known device directly, from an ordinary socket,
//!   which needs nothing. A unicast query is a solicited flow, so Windows lets
//!   the answer back through the firewall.
//!
//! Nothing here needs a packet driver, which is the point: a machine without
//! Npcap still resolves names.

use std::collections::HashMap;
use std::net::UdpSocket;
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

/// Most names this cache will hold. A household has a few dozen devices; the
/// number exists because the input is an unauthenticated broadcast from anyone on
/// the wire, and the file is rewritten every window.
const MAX_NAMES: usize = 512;
/// Longest name kept. DHCP option 12 has no length agreement between clients.
const MAX_NAME: usize = 64;

/// Trim a device-announced name to something a roster can show and a file can
/// hold: printable, bounded, and never empty.
fn tidy_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control() && *c != '|' && *c != '\n' && *c != '\r')
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn remember(found: &HashMap<String, String>) {
    let mut all = cached();
    let before = all.len();
    for (mac, name) in found {
        if let Some(clean) = tidy_name(name) {
            if all.len() >= MAX_NAMES && !all.contains_key(mac) {
                // Full and this is a new device: drop it rather than grow.
                continue;
            }
            all.insert(mac.clone(), clean);
        }
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

// ---------------------------------------------------------------- wire formats

/// The name out of one DHCP message, plus the hardware address it belongs to.
///
/// The input is the DHCP payload itself, starting at the BOOTP `op` byte: a
/// socket hands over the payload with no headers in front of it.
pub fn dhcp_payload(payload: &[u8]) -> Option<(String, String)> {
    if payload.len() < 240 {
        return None;
    }
    // Only client messages carry a hostname, and only they are broadcast.
    if payload[0] != 1 {
        return None;
    }
    if payload[236..240] != [0x63, 0x82, 0x53, 0x63] {
        return None;
    }
    let chaddr = &payload[28..34];
    let mac = chaddr
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let mut i = 240;
    while i + 1 < payload.len() {
        let code = payload[i];
        if code == 255 {
            break;
        }
        let len = payload[i + 1] as usize;
        if i + 2 + len > payload.len() {
            break;
        }
        if code == 12 && len > 0 {
            let raw = &payload[i + 2..i + 2 + len];
            let name = String::from_utf8_lossy(raw).trim().to_string();
            if !name.is_empty() {
                return Some((mac, name));
            }
        }
        i += 2 + len;
    }
    None
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

fn read_name(buf: &[u8], start: usize) -> Option<String> {
    let mut out = String::new();
    let mut i = start;
    let mut hops = 0;
    loop {
        let len = *buf.get(i)? as usize;
        if len == 0 {
            break;
        }
        if len & 0xc0 == 0xc0 {
            let ptr = (((len & 0x3f) as usize) << 8) | *buf.get(i + 1)? as usize;
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
    Some(out)
}

/// The name out of one DNS message: a PTR target if there is one, otherwise the
/// owner of an address record. Used for questions this app asked itself.
///
/// The input is the DNS message, with no transport headers in front of it,
/// which is what a UDP socket returns.
pub fn dns_payload_name(dns: &[u8]) -> Option<String> {
    if dns.len() < 12 {
        return None;
    }
    let qdcount = u16::from_be_bytes([dns[4], dns[5]]) as usize;
    let ancount = u16::from_be_bytes([dns[6], dns[7]]) as usize;
    let mut i = 12;
    for _ in 0..qdcount {
        i = skip_name(dns, i)?;
        i += 4;
    }
    let mut fallback = String::new();
    for _ in 0..ancount {
        let owner = read_name(dns, i)?;
        i = skip_name(dns, i)?;
        if i + 10 > dns.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([dns[i], dns[i + 1]]);
        let rdlen = u16::from_be_bytes([dns[i + 8], dns[i + 9]]) as usize;
        i += 10;
        if i + rdlen > dns.len() {
            return None;
        }
        if rtype == 12 {
            if let Some(target) = read_name(dns, i) {
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

/// The name out of a DNS message that arrived as an Ethernet frame.
///
/// Kept as the frame-shaped entry point because that is the shape the callers
/// here already pass around; it now strips the headers and hands the message to
/// [`dns_payload_name`], so there is one parser and not two.
pub fn answer_name(frame: &[u8], _port: u16) -> Option<String> {
    if frame.len() < 14 || frame[12] != 0x08 || frame[13] != 0x00 {
        return None;
    }
    let ihl = (frame[14] & 0x0f) as usize * 4;
    if frame.len() < 14 + ihl + 8 || frame[14 + 9] != 17 {
        return None;
    }
    let dns = 14 + ihl + 8;
    dns_payload_name(frame.get(dns..)?)
}

// ---------------------------------------------------------------- listening

/// How long the DHCP leg waits before the mDNS leg gets its turn.
const DHCP_WINDOW: Duration = Duration::from_secs(20);

/// Listen for the duration given and return whatever names were announced.
pub fn listen(secs: u64) -> HashMap<String, String> {
    let began = Instant::now();
    let deadline = began + Duration::from_secs(secs);
    let mut found: HashMap<String, String> = HashMap::new();

    // The privileged leg first: it is the only source that names a phone that
    // answers nothing, and it is the one that will not always be available.
    let dhcp_until = deadline.min(began + DHCP_WINDOW);
    if let Ok(sock) = dhcp_socket() {
        let _ = sock.set_read_timeout(Some(Duration::from_millis(400)));
        let mut buf = [0u8; 1500];
        while Instant::now() < dhcp_until {
            match sock.recv_from(&mut buf) {
                Ok((n, _)) => {
                    if let Some((mac, name)) = dhcp_payload(&buf[..n.min(buf.len())]) {
                        if let Some(clean) = tidy_name(&name) {
                            found.insert(mac, clean);
                        }
                    }
                }
                // A timeout is the normal case on a quiet network; one is
                // enough to stop waiting, the rest of the window is for a
                // device that is actually joining.
                Err(_) => break,
            }
        }
    }

    // The unprivileged leg: ask every device this machine already knows about
    // for its own name. This is what makes names work with no driver and no
    // elevation.
    let mut by_ip: HashMap<String, String> = HashMap::new();
    for device in crate::lan::known() {
        if Instant::now() >= deadline {
            break;
        }
        if device.mac.is_empty() || device.self_pc {
            continue;
        }
        let wait = Duration::from_millis(400);
        if let Some(name) = crate::lan_ask::mdns_ask(&device.ip, &device.mac, wait) {
            by_ip.insert(device.ip.clone(), name);
        }
    }
    if !by_ip.is_empty() {
        crate::lan::remember_probed(&by_ip);
    }

    remember(&found);
    found
}

/// Bind the DHCP server port, when this process is allowed to.
///
/// Binding 67 needs an elevated process and needs nothing else on the machine to
/// be listening there. Both are outside this function's control, so a failure is
/// reported as "no passive leg" and never as an error: names still resolve from
/// the mDNS leg.
fn dhcp_socket() -> Result<UdpSocket, ()> {
    let sock = UdpSocket::bind("0.0.0.0:67").map_err(|_| ())?;
    sock.set_broadcast(true).map_err(|_| ())?;
    Ok(sock)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dhcp_with_name(name: &str) -> Vec<u8> {
        let mut p = vec![0u8; 240];
        p[0] = 1; // BOOTREQUEST
        p[28..34].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0x01]);
        p[236..240].copy_from_slice(&[0x63, 0x82, 0x53, 0x63]);
        p.push(12); // hostname option
        p.push(name.len() as u8);
        p.extend_from_slice(name.as_bytes());
        p.push(255);
        p
    }

    #[test]
    fn a_dhcp_request_names_the_device_that_sent_it() {
        let (mac, name) = dhcp_payload(&dhcp_with_name("pixel-8")).expect("name");
        assert_eq!(mac, "aa:bb:cc:dd:ee:01");
        assert_eq!(name, "pixel-8");
    }

    #[test]
    fn a_dhcp_reply_is_not_read_as_a_device_name() {
        // op = 2 is the server's answer, which carries no client hostname.
        let mut p = dhcp_with_name("pixel-8");
        p[0] = 2;
        assert!(dhcp_payload(&p).is_none());
    }

    #[test]
    fn a_truncated_dhcp_message_is_refused_rather_than_indexed_past() {
        let full = dhcp_with_name("pixel-8");
        for cut in [0usize, 1, 239] {
            assert!(dhcp_payload(&full[..cut.min(full.len())]).is_none(), "cut {cut}");
        }
    }

    fn ptr_answer() -> Vec<u8> {
        // Header: response, one question, one answer.
        let mut d = vec![0x00, 0x00, 0x84, 0x00, 0x00, 0x01, 0x00, 0x01, 0, 0, 0, 0];
        // Question: 39.1.168.192.in-addr.arpa PTR
        for label in ["39", "1", "168", "192", "in-addr", "arpa"] {
            d.push(label.len() as u8);
            d.extend_from_slice(label.as_bytes());
        }
        d.push(0);
        d.extend_from_slice(&[0x00, 0x0c, 0x80, 0x01]);
        // Answer: same owner, PTR, target "pixel-8.local"
        d.extend_from_slice(&[0xc0, 0x0c]);
        d.extend_from_slice(&[0x00, 0x0c, 0x80, 0x01, 0, 0, 0, 120]);
        let mut target = Vec::new();
        for label in ["pixel-8", "local"] {
            target.push(label.len() as u8);
            target.extend_from_slice(label.as_bytes());
        }
        target.push(0);
        d.extend_from_slice(&(target.len() as u16).to_be_bytes());
        d.extend_from_slice(&target);
        d
    }

    #[test]
    fn a_reverse_lookup_answer_yields_the_host_name() {
        assert_eq!(dns_payload_name(&ptr_answer()).as_deref(), Some("pixel-8"));
    }

    #[test]
    fn a_truncated_dns_message_yields_nothing_instead_of_panicking() {
        let full = ptr_answer();
        for cut in 0..full.len() {
            let _ = dns_payload_name(&full[..cut]);
        }
    }

    #[test]
    fn the_frame_entry_point_reads_the_same_answer() {
        // Ethernet + IPv4 + UDP in front of the message: the shape the older
        // callers pass, and it must reach the same parser.
        let dns = ptr_answer();
        let mut udp = vec![0x14, 0xe9, 0x14, 0xe9];
        udp.extend_from_slice(&((8 + dns.len()) as u16).to_be_bytes());
        udp.extend_from_slice(&[0, 0]);
        udp.extend_from_slice(&dns);
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&((20 + udp.len()) as u16).to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0x40, 0x00, 64, 17, 0, 0]);
        ip.extend_from_slice(&[192, 168, 1, 5, 192, 168, 1, 39]);
        ip.extend_from_slice(&udp);
        let mut frame = vec![0u8; 12];
        frame.extend_from_slice(&[0x08, 0x00]);
        frame.extend_from_slice(&ip);
        assert_eq!(answer_name(&frame, 5353).as_deref(), Some("pixel-8"));
    }
}
