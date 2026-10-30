//! What a device is, when it will not say its name.
//!
//! Phones hide their hardware address and ignore every name protocol, so an
//! honest list cannot always carry a name. It can still carry the truth: who
//! made the thing, and what kind of thing it is. A person reading "Vivo, phone"
//! knows which row is their phone; a row of dashes tells them nothing.

use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

/// The IEEE manufacturer register, shipped with the app as a compact table:
/// a sorted list of the three-byte blocks and one blob of names. No network
/// call, no download, and it works with the app offline.
const OUI: &[u8] = include_bytes!("../assets/oui.bin");

fn read_u32(at: usize) -> u32 {
    u32::from_le_bytes([OUI[at], OUI[at + 1], OUI[at + 2], OUI[at + 3]])
}

/// Binary search over the sorted block list.
fn lookup_block(block: u32) -> Option<&'static str> {
    let count = read_u32(0) as usize;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let base = 4 + mid * 8;
        let key = read_u32(base);
        if key == block {
            let offset = read_u32(base + 4) as usize;
            let start = 4 + count * 8 + offset;
            let len = OUI[start..].iter().position(|b| *b == 0)?;
            return std::str::from_utf8(&OUI[start..start + len]).ok();
        }
        if key < block {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    None
}

/// The manufacturer, from the hardware address, when the address is real.
pub fn vendor(mac: &str) -> Option<String> {
    let clean = mac.replace('-', ":").to_uppercase();
    let octets: Vec<&str> = clean.split(':').collect();
    if octets.len() != 6 {
        return None;
    }
    let mut key: u32 = 0;
    for o in &octets[..3] {
        key = (key << 8) | u32::from_str_radix(o, 16).ok()?;
    }
    lookup_block(key).map(tidy)
}

/// OUI entries are written in capitals and often end with a legal suffix.
fn tidy(name: &str) -> String {
    let mut out = String::new();
    for word in name.split_whitespace() {
        let lower = word.to_lowercase();
        if matches!(
            lower.as_str(),
            "co.,ltd." | "co.,ltd" | "co.," | "ltd." | "ltd" | "inc." | "inc" | "corporation" | "corp." | "corp"
                | "technologies" | "technology" | "limited" | "gmbh" | "s.a." | "s.l." | "b.v." | "pty" | "llc"
        ) {
            continue;
        }
        if out.is_empty() {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_uppercase().next().unwrap_or(first));
                out.push_str(&chars.as_str().to_lowercase());
            }
        } else {
            out.push(' ');
            out.push_str(&word.to_lowercase());
        }
    }
    if out.is_empty() {
        name.to_string()
    } else {
        out
    }
}

/// True when the address was made up by the device, which is what phones do.
pub fn private_address(mac: &str) -> bool {
    mac.split(':')
        .next()
        .and_then(|b| u8::from_str_radix(b, 16).ok())
        .map(|b| b & 0x02 != 0)
        .unwrap_or(false)
}

fn open(ip: &str, port: u16) -> bool {
    let Ok(addr) = ip.parse::<IpAddr>() else { return false };
    let target = SocketAddr::new(addr, port);
    TcpStream::connect_timeout(&target, Duration::from_millis(220)).is_ok()
}

/// What kind of device answers on these ports. Order matters: the first match
/// wins, so the most specific signature is asked first.
pub fn kind(ip: &str) -> Option<&'static str> {
    // iPhone and iPad answer here and nowhere else useful.
    if open(ip, 62078) {
        return Some("phone");
    }
    if open(ip, 445) || open(ip, 139) {
        return Some("windows");
    }
    if open(ip, 9100) || open(ip, 631) || open(ip, 515) {
        return Some("printer");
    }
    if open(ip, 554) || open(ip, 8554) {
        return Some("camera");
    }
    if open(ip, 8009) {
        return Some("tv");
    }
    if open(ip, 22) {
        return Some("linux");
    }
    None
}

/// A label from the two things a scan already knows: the hardware address and
/// the time-to-live of the reply. No extra packets, no waiting.
pub fn describe_ttl(mac: &str, ttl: u32) -> String {
    let kind = match ttl {
        0 => None,
        1..=64 => Some("unix"),
        65..=127 => Some("device"),
        _ => Some("windows"),
    };
    describe(mac, "", "", kind)
}

/// The line a row shows when there is no name: who made it, and what it is.
pub fn describe(mac: &str, ip: &str, name: &str, kind: Option<&str>) -> String {
    if !name.is_empty() && name != "-" && name != "private" {
        return name.to_string();
    }
    let maker = vendor(mac);
    let what = kind.map(|k| match k {
        "phone" => "phone",
        "windows" => "windows pc",
        "printer" => "printer",
        "camera" => "camera",
        "tv" => "tv",
        "linux" => "linux device",
        "unix" => "device",
        "windows" => "windows pc",
        _ => "device",
    });
    match (maker, what) {
        (Some(m), Some(w)) => format!("{m} · {w}"),
        (Some(m), None) => m,
        (None, Some(w)) if private_address(mac) => format!("private · {w}"),
        (None, Some(w)) => w.to_string(),
        (None, None) if private_address(mac) => String::from("private"),
        (None, None) => String::from("-"),
    }
}
