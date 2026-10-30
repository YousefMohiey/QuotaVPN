//! Who is on this network.
//!
//! A home router hides its client list behind an admin login, so the honest way
//! to answer "who is on my wifi" from a normal PC is the one every network tool
//! uses: wake the subnet with a ping sweep, then read the neighbour table the
//! machine already keeps. No driver, no administrator, no packet capture: the
//! listing has to work before the app is allowed to ask for anything heavier.

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

/// One device seen on the local network.
#[derive(Clone, serde::Serialize)]
pub struct Device {
    pub ip: String,
    pub mac: String,
    pub name: String,
}

#[cfg(windows)]
fn hidden(prog: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    let mut c = Command::new(prog);
    c.creation_flags(NO_WINDOW);
    c
}

/// The address this machine uses to reach the internet, which is the one whose
/// subnet we care about.
fn local_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

/// Prefix length for the interface that owns `ip`, read from the system config.
/// Home networks are nearly always /24, so that is the fallback.
#[cfg(windows)]
fn prefix_for(ip: Ipv4Addr) -> u8 {
    let out = match hidden("ipconfig").output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return 24,
    };
    let target = ip.to_string();
    let mut current: Option<String> = None;
    for raw in out.lines() {
        let line = raw.trim();
        if line.to_lowercase().contains("ipv4") {
            if let Some(v) = line.split(':').nth(1) {
                current = Some(v.trim().to_string());
            }
        }
        if line.to_lowercase().contains("subnet mask") || line.contains("قناع") {
            if let (Some(cur), Some(mask)) = (current.as_ref(), line.split(':').nth(1)) {
                if *cur == target {
                    let m = mask.trim();
                    if let Ok(octets) = m
                        .split('.')
                        .map(|p| p.parse::<u8>())
                        .collect::<Result<Vec<_>, _>>()
                    {
                        if octets.len() == 4 {
                            let bits: u32 = octets
                                .iter()
                                .fold(0u32, |acc, o| (acc << 8) | (*o as u32));
                            return bits.leading_ones() as u8;
                        }
                    }
                }
            }
        }
    }
    24
}

#[cfg(not(windows))]
fn prefix_for(_ip: Ipv4Addr) -> u8 {
    24
}

/// The neighbour table: ip to mac, as the machine sees it right now.
#[cfg(windows)]
fn arp_table() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let out = match hidden("arp").arg("-a").output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return map,
    };
    for raw in out.lines() {
        let line = raw.trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let ip = parts[0];
        let mac = parts[1].replace('-', ":");
        let looks_ip = ip.split('.').count() == 4
            && ip.split('.').all(|p| p.parse::<u8>().is_ok());
        let looks_mac = mac.split(':').count() == 6;
        if looks_ip && looks_mac {
            map.insert(ip.to_string(), mac.to_lowercase());
        }
    }
    map
}

#[cfg(not(windows))]
fn arp_table() -> HashMap<String, String> {
    HashMap::new()
}

/// Ask one address to answer, and take its name from the reply when the system
/// resolves one. Windows prints "Pinging NAME [ip] with 32 bytes" when it knows
/// the name, and "Pinging ip with 32 bytes" when it does not.
#[cfg(windows)]
fn wake(ip: String) -> (String, bool, String) {
    let out = match hidden("ping").args(["-n", "1", "-w", "260", &ip]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return (ip, false, String::new()),
    };
    let alive = out.contains("TTL=") || out.contains("TTL =");
    (ip, alive, String::new())
}

#[cfg(not(windows))]
fn wake(ip: String) -> (String, bool, String) {
    (ip, false, String::new())
}

/// Every device on this subnet, sorted by address.
pub fn scan() -> Vec<Device> {
    let Some(local) = local_ipv4() else {
        return Vec::new();
    };
    let prefix = prefix_for(local);
    // Only a home-sized subnet is swept: a /16 would take minutes and is not
    // what this screen is for.
    let hosts: u32 = if prefix >= 24 { 254 } else { 254 };
    let base = u32::from(local) & !((1u32 << (32 - prefix.min(30))) - 1);
    let me = local.to_string();

    let found: Arc<Mutex<Vec<(String, bool, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    let chunk = 24usize;
    let mut batch = Vec::new();
    for i in 1..=hosts {
        let ip = Ipv4Addr::from(base + i).to_string();
        if ip == me {
            continue;
        }
        batch.push(ip);
        if batch.len() == chunk {
            let queue = std::mem::take(&mut batch);
            let sink = Arc::clone(&found);
            handles.push(thread::spawn(move || {
                for ip in queue {
                    let r = wake(ip);
                    if let Ok(mut g) = sink.lock() {
                        g.push(r);
                    }
                }
            }));
        }
    }
    if !batch.is_empty() {
        let sink = Arc::clone(&found);
        handles.push(thread::spawn(move || {
            for ip in batch {
                let r = wake(ip);
                if let Ok(mut g) = sink.lock() {
                    g.push(r);
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    // Names for the hosts that answered, in parallel, and never for the silent
    // ones: a reverse lookup against a missing DNS server can hang for seconds.
    let alive: Vec<String> = found
        .lock()
        .map(|g| g.iter().filter(|(_, a, _)| *a).map(|(ip, _, _)| ip.clone()).collect())
        .unwrap_or_default();
    let names: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut name_handles = Vec::new();
    for ip in alive {
        let sink = Arc::clone(&names);
        name_handles.push(thread::spawn(move || {
            if let Ok(o) = hidden("ping").args(["-a", "-n", "1", "-w", "600", &ip]).output() {
                let text = String::from_utf8_lossy(&o.stdout).to_string();
                for raw in text.lines() {
                    let line = raw.trim();
                    if let Some(rest) = line.strip_prefix("Pinging ") {
                        let head = rest.split(" with ").next().unwrap_or("");
                        let head = head.split(" [").next().unwrap_or("").trim();
                        if !head.is_empty() && head != ip && head.split('.').count() != 4 {
                            if let Ok(mut g) = sink.lock() {
                                g.insert(ip.clone(), head.to_string());
                            }
                        }
                        break;
                    }
                }
            }
        }));
    }
    for h in name_handles {
        let _ = h.join();
    }
    let name_map = names.lock().map(|g| g.clone()).unwrap_or_default();

    let table = arp_table();
    let mut out: Vec<Device> = Vec::new();
    let results = found.lock().map(|g| g.clone()).unwrap_or_default();
    for (ip, alive, name) in results {
        let mac = table.get(&ip).cloned().unwrap_or_default();
        if mac.is_empty() {
            continue;
        }
        // The neighbour table also holds multicast groups, the broadcast
        // address and incomplete entries. None of those is a device.
        let first: u32 = ip.split('.').next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let last: u32 = ip.split('.').last().and_then(|p| p.parse().ok()).unwrap_or(0);
        if first == 0 || first >= 224 || ip == "255.255.255.255" || last == 255 || last == 0 {
            continue;
        }
        if !alive && mac.starts_with("00:00:00") {
            continue;
        }
        let resolved = name_map.get(&ip).cloned().unwrap_or(name);
        out.push(Device {
            ip,
            mac,
            name: if resolved.is_empty() { String::from("-") } else { resolved },
        });
    }
    // The router itself is the gateway, and it answers as an ordinary host.
    out.sort_by_key(|d| {
        d.ip.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .fold(0u32, |acc, n| (acc << 8) | n)
    });
    out.dedup_by(|a, b| a.ip == b.ip);
    out
}
