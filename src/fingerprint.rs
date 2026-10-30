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

type Blocks = (HashMap<u32, String>, HashMap<u64, String>);

fn table() -> &'static Blocks {
    static TABLE: OnceLock<Blocks> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut by3: HashMap<u32, String> = HashMap::new();
        let mut by6: HashMap<u64, String> = HashMap::new();
        for line in MANUF.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let Some(prefix) = parts.next() else { continue };
            let short = parts.next().unwrap_or("");
            // The long name is the readable one; the short name is truncated
            // to ten characters and reads badly in a list.
            let long = parts.next().unwrap_or("");
            let name = if long.trim().is_empty() { short } else { long };
            if name.trim().is_empty() {
                continue;
            }
            let octets: Vec<&str> = prefix.split(':').collect();
            let mut value: u64 = 0;
            let mut ok = true;
            for o in &octets {
                match u64::from_str_radix(o, 16) {
                    Ok(v) => value = (value << 8) | v,
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            match octets.len() {
                3 => {
                    by3.insert(value as u32, tidy(name));
                }
                6 => {
                    by6.insert(value, tidy(name));
                }
                _ => {}
            }
        }
        (by3, by6)
    })
}

/// The manufacturer, from the hardware address, when the address is real.
pub fn vendor(mac: &str) -> Option<String> {
    let clean = mac.replace('-', ":").to_uppercase();
    let octets: Vec<&str> = clean.split(':').collect();
    if octets.len() != 6 {
        return None;
    }
    let mut key: u64 = 0;
    for o in &octets {
        key = (key << 8) | u64::from_str_radix(o, 16).ok()?;
    }
    let (by3, by6) = table();
    // A subdivided block is more specific than the three-byte one.
    if let Some(name) = by6.get(&key) {
        return Some(name.clone());
    }
    by3.get(&((key >> 24) as u32)).cloned()
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
