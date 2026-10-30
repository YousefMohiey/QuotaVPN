//! What a device is, when it will not say its name.
//!
//! Phones hide their hardware address and ignore every name protocol, so an
//! honest list cannot always carry a name. It can still carry the truth: who
//! made the thing, and what kind of thing it is. A person reading "Vivo, phone"
//! knows which row is their phone; a row of dashes tells them nothing.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;

/// The Wireshark manufacturer table, shipped with the app. It is the same list
/// every network tool uses, and it turns three bytes of an address into a name.
const MANUF: &str = include_str!("../assets/manuf.txt");

fn table() -> &'static HashMap<u32, String> {
    static TABLE: OnceLock<HashMap<u32, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        for line in MANUF.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let Some(prefix) = parts.next() else { continue };
            let Some(name) = parts.next() else { continue };
            // Only whole three-byte blocks: a subdivided range would need the
            // full six bytes to be right, and guessing would be worse than "-".
            let octets: Vec<&str> = prefix.split(':').collect();
            if octets.len() != 3 {
                continue;
            }
            let mut key: u32 = 0;
            let mut ok = true;
            for o in octets {
                match u32::from_str_radix(o, 16) {
                    Ok(v) => key = (key << 8) | v,
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                map.insert(key, tidy(name));
            }
        }
        map
    })
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
    table().get(&key).cloned()
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
