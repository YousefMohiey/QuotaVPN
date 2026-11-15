//! The forward pipeline: the part of the LAN features that has to be elevated.
//!
//! `WinDivertOpen` on the forward layer requires Administrator, and this app
//! deliberately runs unelevated, so the loop cannot live in the window process.
//! It lives in the same privileged pass the tunnel already uses, and the two
//! halves talk through files in the engine directory, which is the pattern the
//! kill switch and the engine pid already follow here:
//!
//! * the app writes `lan/policy.json`: which addresses, which are blocked, what
//!   each one is capped at, and a sequence number that changes when any of that
//!   does;
//! * this loop writes `lan/state.json`: what each address has moved, the busiest
//!   destination names, and whether the driver is actually carrying anything;
//! * the app writes `lan/stop` to end the session, exactly like `engine.stop`.
//!
//! Nothing about the mechanism is visible across that line, which is why the
//! app's counters and limits could keep their shapes.
//!
//! A note on the host: the forward layer only ever shows packets being routed
//! between two interfaces, so this machine's own traffic never appears here.
//! That is the model, not an omission: the "This PC" row is filled from the
//! engine's counters, and a device's numbers are the device's alone.

#![allow(non_snake_case)]
// The loop that consumes most of this is Windows-only, and gating the module
// just to keep the compiler quiet would also stop the ledger's tests from
// running on the machine they are written on. The parts that are unused
// off Windows are the file plumbing, and each has its one caller there.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How many destination names to keep. The page shows a handful; the rest are
/// noise from a browser opening side connections, so the list is bounded.
pub const MAX_DOMAINS: usize = 256;
/// How many address-to-name bindings to keep from DNS responses.
pub const MAX_BINDINGS: usize = 4096;
/// Most addresses one policy may carry. A roster is a household, not a campus,
/// and the list becomes a driver filter string.
pub const MAX_IPS: usize = 256;
/// Longest session the loop will honour. The app asks for fifteen minutes; an
/// unbounded value would let a file keep an elevated filtering process alive.
pub const MAX_TTL_SECS: u64 = 12 * 60 * 60;

/// How often the policy file is re-read. Half a second is fast enough that a
/// block feels instant and slow enough that it costs nothing.
const POLICY_POLL: Duration = Duration::from_millis(500);
/// How often the state file is rewritten.
const STATE_EVERY: Duration = Duration::from_secs(1);

pub fn dir() -> PathBuf {
    crate::vpn::engine_dir().join("lan")
}

fn policy_path() -> PathBuf {
    dir().join("policy.json")
}

fn state_path() -> PathBuf {
    dir().join("state.json")
}

fn stop_path() -> PathBuf {
    dir().join("stop")
}

/// What one address is allowed to do.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Cap {
    /// Bytes per second, as the UI's KB/s times 1024. Zero means no cap.
    pub up_bps: u64,
    pub down_bps: u64,
}

/// The app's instructions for one session.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Policy {
    /// Bumped by the app on every change, so the loop can tell "the same
    /// instructions" from "new instructions" without comparing the body.
    pub seq: u64,
    /// The addresses to watch. The driver filter is built from these, so a
    /// device that is not listed is never copied up at all.
    pub ips: Vec<String>,
    /// Addresses whose packets are dropped instead of forwarded.
    pub blocked: Vec<String>,
    /// Per-address caps, keyed by address.
    pub caps: HashMap<String, Cap>,
    /// A session that outlives this is a session nobody is watching any more.
    pub ttl_secs: u64,
}

impl Policy {
    /// The addresses the driver should hand over: everything watched, which is
    /// the watched list regardless of block or cap, because a blocked packet has
    /// to be seen to be dropped.
    pub fn watched(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for ip in &self.ips {
            let ip = ip.trim();
            if !ip.is_empty() && !out.iter().any(|x| x == ip) {
                out.push(ip.to_string());
            }
        }
        out
    }

    pub fn is_blocked(&self, ip: &str) -> bool {
        self.blocked.iter().any(|b| b == ip)
    }

    pub fn cap_for(&self, ip: &str) -> Cap {
        self.caps.get(ip).cloned().unwrap_or_default()
    }

    /// Parse without trusting the file: a half-written policy is a policy that
    /// is not there yet, and the loop must keep running on the old one.
    ///
    /// The file sits in the user's profile and this loop runs elevated, so its
    /// contents are only as trustworthy as that directory. Everything it can
    /// affect is bounded here: addresses must parse as IPv4 (they are
    /// interpolated into a driver filter, so a string is also a syntax problem),
    /// the lists are capped, and the session lifetime has a ceiling.
    pub fn parse(text: &str) -> Option<Policy> {
        let mut p: Policy = serde_json::from_str(text).ok()?;
        p.ips.retain(|s| s.parse::<std::net::Ipv4Addr>().is_ok());
        p.blocked.retain(|s| s.parse::<std::net::Ipv4Addr>().is_ok());
        p.caps.retain(|k, _| k.parse::<std::net::Ipv4Addr>().is_ok());
        p.ips.truncate(MAX_IPS);
        p.blocked.truncate(MAX_IPS);
        p.ttl_secs = p.ttl_secs.clamp(60, MAX_TTL_SECS);
        Some(p)
    }
}

/// What one address has moved this session.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DeviceBytes {
    pub ip: String,
    pub up: u64,
    pub down: u64,
    /// How many packets the pipeline saw for this address, in either direction.
    /// The frame counter a diagnosis used to take from a raw capture reads this
    /// instead: it is the same fact, from the layer that is actually carrying.
    pub packets: u64,
}

/// One destination name and the bytes seen for it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DomainBytes {
    pub name: String,
    pub bytes: u64,
}

/// What the app reads back.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub pid: u32,
    /// False once the loop has stopped for good, so the app can tell a dead
    /// session from a quiet one.
    pub running: bool,
    /// Why it is not running, in words a person can act on.
    pub error: String,
    pub seq: u64,
    pub devices: Vec<DeviceBytes>,
    pub domains: Vec<DomainBytes>,
    pub blocked_dropped: u64,
    pub capped_dropped: u64,
    pub forwarded: u64,
}

/// Counts one packet against the address it belongs to.
///
/// Upload is the packet's source, download its destination, which is the whole
/// attribution rule on this layer: a forwarded packet has exactly one of the two
/// ends on the LAN.
pub fn attribute(devices: &mut HashMap<String, DeviceBytes>, src: &str, dst: &str, len: u64) {
    if let Some(d) = devices.get_mut(src) {
        d.up += len;
        d.packets += 1;
    }
    if let Some(d) = devices.get_mut(dst) {
        d.down += len;
        d.packets += 1;
    }
}

/// Bind an address seen in a DNS response to the name in it, with a bound.
pub fn bind(bindings: &mut HashMap<String, String>, ip: &str, name: &str) {
    if bindings.len() >= MAX_BINDINGS && !bindings.contains_key(ip) {
        // Bound, not unbounded: a browser resolving thousands of names in a
        // session must not grow this map for the rest of it.
        return;
    }
    bindings.insert(ip.to_string(), name.to_string());
}

/// Add bytes to a destination, keeping the list bounded by dropping the least
/// busy entry when it is full and the new name does not fit.
pub fn add_domain(domains: &mut Vec<DomainBytes>, name: &str, bytes: u64) {
    if let Some(d) = domains.iter_mut().find(|d| d.name == name) {
        d.bytes += bytes;
        return;
    }
    if domains.len() >= MAX_DOMAINS {
        if let Some((least, _)) = domains
            .iter()
            .enumerate()
            .min_by_key(|(_, d)| d.bytes)
        {
            domains.swap_remove(least);
        }
    }
    domains.push(DomainBytes { name: name.to_string(), bytes });
}

/// Sorted, busiest first, for the page.
pub fn top_domains(domains: &[DomainBytes], n: usize) -> Vec<DomainBytes> {
    let mut out = domains.to_vec();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    out.truncate(n);
    out
}

/// Write a file by replacing it, so a reader never sees half of one.
pub fn write_atomic(path: &std::path::Path, body: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Read the policy the app last wrote.
pub fn read_policy() -> Option<Policy> {
    Policy::parse(&std::fs::read_to_string(policy_path()).ok()?)
}

/// Write the policy the loop reads, replacing it whole.
pub fn write_policy(policy: &Policy) {
    if let Ok(body) = serde_json::to_string(policy) {
        write_atomic(&policy_path(), &body);
    }
}

/// Publish a state snapshot.
pub fn publish(state: &State) {
    if let Ok(body) = serde_json::to_string_pretty(state) {
        write_atomic(&state_path(), &body);
    }
}

/// Read the last snapshot, for the app side.
pub fn read_state() -> Option<State> {
    serde_json::from_str(&std::fs::read_to_string(state_path()).ok()?).ok()
}

/// Ask the loop to end, from the app side.
pub fn request_stop() {
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(stop_path(), "1");
}

/// Each session's own in-memory bookkeeping, separable from the driver so it can
/// be exercised without one.
pub struct Ledger {
    pub devices: HashMap<String, DeviceBytes>,
    bindings: HashMap<String, String>,
    domains: Vec<DomainBytes>,
    pub blocked_dropped: u64,
    pub capped_dropped: u64,
    pub forwarded: u64,
    /// The moment each device's next packet may leave, for the caps.
    slots_up: HashMap<String, Instant>,
    slots_down: HashMap<String, Instant>,
}

impl Ledger {
    pub fn new(ips: &[String]) -> Ledger {
        let mut devices = HashMap::new();
        for ip in ips {
            devices.insert(
                ip.clone(),
                DeviceBytes { ip: ip.clone(), up: 0, down: 0, packets: 0 },
            );
        }
        Ledger {
            devices,
            bindings: HashMap::new(),
            domains: Vec::new(),
            blocked_dropped: 0,
            capped_dropped: 0,
            forwarded: 0,
            slots_up: HashMap::new(),
            slots_down: HashMap::new(),
        }
    }

    /// Count one packet. Returns false when it must not be forwarded, and the
    /// caller then simply does not hand it back to the driver.
    pub fn admit(
        &mut self,
        policy: &Policy,
        src: &str,
        dst: &str,
        len: usize,
        now: Instant,
    ) -> bool {
        attribute(&mut self.devices, src, dst, len as u64);
        if policy.is_blocked(src) || policy.is_blocked(dst) {
            self.blocked_dropped += 1;
            return false;
        }
        // A cap applies to the direction the device is using: its own uploads
        // are paced by the upload cap, and what arrives for it by the download
        // cap. Both ends of a forwarded packet are checked, because one device's
        // download is the internet's upload.
        for (ip, upward) in [(src, true), (dst, false)] {
            let cap = policy.cap_for(ip);
            let rate = if upward { cap.up_bps } else { cap.down_bps };
            if rate == 0 {
                continue;
            }
            let slot = if upward {
                self.slots_up.entry(ip.to_string()).or_insert(now)
            } else {
                self.slots_down.entry(ip.to_string()).or_insert(now)
            };
            match crate::windivert::pace(len, rate, slot, now) {
                Some(_) => {}
                None => {
                    self.capped_dropped += 1;
                    return false;
                }
            }
        }
        self.forwarded += 1;
        true
    }

    /// Learn the name behind a forwarded packet, from whichever of the three
    /// places carries one.
    pub fn learn(&mut self, protocol: u8, payload: &[u8]) {
        use crate::windivert as wd;
        let Some((sport, dport)) = wd::ports(protocol, payload) else {
            return;
        };
        let Some(body) = wd::payload_after_transport(protocol, payload) else {
            return;
        };
        // DNS: a response carries the name and the address together, which is
        // the binding every later packet to that address is named from.
        if protocol == 17 && (sport == 53 || dport == 53) {
            for (name, ip) in wd::dns_answers(body) {
                if let Some(clean) = wd::clean_name(&name) {
                    bind(&mut self.bindings, &wd::ip_text(ip), &clean);
                }
            }
            return;
        }
        // TLS and plain HTTP are not here: their name belongs to the address in
        // the same packet, which this function is not given. The loop calls
        // `learn_name_for` for those.
    }

    /// Name a destination directly, for the TLS and HTTP cases where the name is
    /// in the packet but the address has to come from the same packet.
    pub fn learn_name_for(&mut self, ip: &str, name: &str, bytes: u64) {
        if let Some(clean) = crate::windivert::clean_name(name) {
            bind(&mut self.bindings, ip, &clean);
            add_domain(&mut self.domains, &clean, bytes);
        }
    }

    /// Count bytes against whatever name that address is already known by.
    pub fn credit_named(&mut self, ip: &str, bytes: u64) {
        if let Some(name) = self.bindings.get(ip).cloned() {
            add_domain(&mut self.domains, &name, bytes);
        }
    }

    pub fn snapshot(&self, seq: u64, running: bool, error: &str) -> State {
        let mut devices: Vec<DeviceBytes> = self.devices.values().cloned().collect();
        devices.sort_by(|a, b| a.ip.cmp(&b.ip));
        State {
            pid: std::process::id(),
            running,
            error: error.to_string(),
            seq,
            devices,
            domains: top_domains(&self.domains, 32),
            blocked_dropped: self.blocked_dropped,
            capped_dropped: self.capped_dropped,
            forwarded: self.forwarded,
        }
    }
}

/// The loop. Elevated only: this is the process that opens the driver.
#[cfg(windows)]
pub fn run() -> Result<(), String> {
    let Some(mut policy) = read_policy() else {
        return Err("no policy: the app did not ask for a session".to_string());
    };
    let _ = std::fs::remove_file(stop_path());
    let mut ledger = Ledger::new(&policy.watched());
    let mut divert = crate::windivert::Divert::open_forward(
        &crate::windivert::forward_filter(&policy.watched()),
    )?;

    let stop = Arc::new(AtomicBool::new(false));
    // The stop file is the app's only lever, so watch it from a thread: the
    // receive below blocks until a packet arrives, and a quiet link is exactly
    // when a stop has to still work.
    {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || loop {
            if stop_path().exists() {
                stop.store(true, Ordering::SeqCst);
                return;
            }
            std::thread::sleep(Duration::from_millis(300));
        });
    }

    let began = Instant::now();
    let mut last_publish = Instant::now() - STATE_EVERY;
    let mut last_policy = Instant::now();
    let mut buf = vec![0u8; 65536];
    let mut error = String::new();

    loop {
        if stop.load(Ordering::SeqCst) || began.elapsed() > Duration::from_secs(policy.ttl_secs.max(60)) {
            error = String::new();
            break;
        }
        if last_policy.elapsed() > POLICY_POLL {
            last_policy = Instant::now();
            if let Some(next) = read_policy() {
                if next.seq != policy.seq {
                    // Caps and blocks are live, but the address list is baked
                    // into the driver filter, so a new list means a new handle.
                    if next.watched() != policy.watched() {
                        divert = match crate::windivert::Divert::open_forward(
                            &crate::windivert::forward_filter(&next.watched()),
                        ) {
                            Ok(d) => d,
                            Err(e) => {
                                error = e;
                                break;
                            }
                        };
                        for ip in next.watched() {
                            ledger.devices.entry(ip.clone()).or_insert(DeviceBytes {
                                ip,
                                up: 0,
                                down: 0,
                                packets: 0,
                            });
                        }
                    }
                    policy = next;
                }
            }
        }
        if last_publish.elapsed() >= STATE_EVERY {
            last_publish = Instant::now();
            publish(&ledger.snapshot(policy.seq, true, ""));
        }

        let packet = match divert.recv(&mut buf) {
            Ok(Some(p)) => p,
            Ok(None) => continue,
            Err(e) => {
                // A shutdown from the drop of the handle is the normal end.
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                error = e;
                break;
            }
        };
        let Some(ip) = crate::windivert::Ipv4::parse(&packet.data) else {
            // Not IPv4: put it back untouched rather than eat it.
            let _ = divert.send(&packet.data, &packet.addr);
            continue;
        };
        let src = crate::windivert::ip_text(ip.src);
        let dst = crate::windivert::ip_text(ip.dst);
        let now = Instant::now();
        // Learn before admitting, so the bytes of this very packet can be
        // credited to the name it just revealed.
        if let Some((sport, dport)) = crate::windivert::ports(ip.protocol, ip.payload) {
            let body = crate::windivert::payload_after_transport(ip.protocol, ip.payload);
            if ip.protocol == 6 && dport == 443 {
                if let Some(sni) = body.and_then(crate::windivert::tls_sni) {
                    ledger.learn_name_for(&dst, &sni, 0);
                }
            } else if ip.protocol == 6 && dport == 80 {
                if let Some(host) = body.and_then(crate::windivert::http_host) {
                    ledger.learn_name_for(&dst, &host, 0);
                }
            } else if ip.protocol == 17 && (sport == 53 || dport == 53) {
                ledger.learn(17, ip.payload);
            }
        }
        if !ledger.admit(&policy, &src, &dst, packet.data.len(), now) {
            continue;
        }
        // Attribute the bytes to a name, if either end is one we have named.
        ledger.credit_named(&dst, packet.data.len() as u64);
        if let Err(e) = divert.send(&packet.data, &packet.addr) {
            error = e;
            break;
        }
    }

    publish(&ledger.snapshot(policy.seq, false, &error));
    let _ = std::fs::remove_file(stop_path());
    let _ = std::fs::remove_file(policy_path());
    Ok(())
}

/// The app side: read the last published snapshot, filling the counters the page
/// already reads.
pub fn counters() -> (Vec<DeviceBytes>, Vec<DomainBytes>, u64, u64, bool) {
    match read_state() {
        Some(s) => (s.devices, s.domains, s.blocked_dropped, s.capped_dropped, s.running),
        None => (Vec::new(), Vec::new(), 0, 0, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(ips: &[&str], blocked: &[&str], caps: &[(&str, u64, u64)]) -> Policy {
        Policy {
            seq: 1,
            ips: ips.iter().map(|s| s.to_string()).collect(),
            blocked: blocked.iter().map(|s| s.to_string()).collect(),
            caps: caps
                .iter()
                .map(|(ip, up, down)| {
                    (
                        ip.to_string(),
                        Cap { up_bps: *up, down_bps: *down },
                    )
                })
                .collect(),
            ttl_secs: 900,
        }
    }

    #[test]
    fn the_watched_list_is_deduplicated_and_keeps_order() {
        let p = policy(&["192.168.1.39", " 192.168.1.39 ", "192.168.1.40", ""], &[], &[]);
        assert_eq!(p.watched(), vec!["192.168.1.39", "192.168.1.40"]);
    }

    #[test]
    fn upload_is_the_source_and_download_is_the_destination() {
        let mut led = Ledger::new(&["192.168.1.39".to_string()]);
        let p = policy(&["192.168.1.39"], &[], &[]);
        let now = Instant::now();
        assert!(led.admit(&p, "192.168.1.39", "1.1.1.1", 100, now));
        assert!(led.admit(&p, "93.184.216.34", "192.168.1.39", 300, now));
        let d = &led.devices["192.168.1.39"];
        assert_eq!(d.up, 100);
        assert_eq!(d.down, 300);
        assert_eq!(led.forwarded, 2);
    }

    #[test]
    fn a_blocked_device_has_its_packets_refused_in_both_directions() {
        let mut led = Ledger::new(&["192.168.1.39".to_string()]);
        let p = policy(&["192.168.1.39"], &["192.168.1.39"], &[]);
        let now = Instant::now();
        assert!(!led.admit(&p, "192.168.1.39", "1.1.1.1", 100, now));
        assert!(!led.admit(&p, "1.1.1.1", "192.168.1.39", 100, now));
        assert_eq!(led.blocked_dropped, 2);
        assert_eq!(led.forwarded, 0);
        // Its bytes are still counted: the block is not a blind spot.
        assert_eq!(led.devices["192.168.1.39"].up, 100);
    }

    #[test]
    fn a_cap_paces_rather_than_blocks() {
        let mut led = Ledger::new(&["192.168.1.39".to_string()]);
        let p = policy(&["192.168.1.39"], &[], &[("192.168.1.39", 100 * 1024, 0)]);
        let now = Instant::now();
        // A normal packet passes.
        assert!(led.admit(&p, "192.168.1.39", "1.1.1.1", 1500, now));
        // Fifty more in the same instant cannot all pass: at 100 KB/s a 1500 byte
        // frame costs 14.6ms, and the bucket tolerates 300ms of backlog, so about
        // 20 frames fit before the release time is too far ahead. That tolerance
        // is deliberate (a TCP handshake of a few frames still gets through), and
        // it is the difference between a cap and a stutter.
        let mut refused = 0;
        for _ in 0..50 {
            if !led.admit(&p, "192.168.1.39", "1.1.1.1", 1500, now) {
                refused += 1;
            }
        }
        assert!(refused > 0, "a 100 KB/s cap must not forward 50 frames at once");
        let tolerated = led.forwarded;
        assert!(
            (18..=22).contains(&tolerated),
            "300ms of backlog at 14.6ms a frame is about 20 frames, got {tolerated}"
        );
        assert_eq!(tolerated + refused, 51, "every frame is either forwarded or counted as dropped");
        // Once the clock has moved past the backlog, the link carries again.
        assert!(led.admit(&p, "192.168.1.39", "1.1.1.1", 1500, now + Duration::from_millis(400)));
    }

    #[test]
    fn a_dns_answer_names_the_address_it_resolves() {
        let mut led = Ledger::new(&["192.168.1.39".to_string()]);
        // Build a response for example.com -> 93.184.216.34, reusing the shape
        // the windivert tests already prove the parser reads.
        let mut body = vec![0x12, 0x34, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0, 0, 0, 0];
        for label in ["example", "com"] {
            body.push(label.len() as u8);
            body.extend_from_slice(label.as_bytes());
        }
        body.push(0);
        body.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        body.extend_from_slice(&[0xc0, 0x0c]);
        body.extend_from_slice(&[0x00, 0x01, 0x00, 0x01, 0, 0, 0, 60, 0x00, 0x04]);
        body.extend_from_slice(&[93, 184, 216, 34]);
        // UDP header with port 53, then the body.
        let mut udp = vec![0x00, 0x35, 0x00, 0x35, 0x00, 0x00, 0x00, 0x00];
        udp.extend_from_slice(&body);
        led.learn(17, &udp);
        led.credit_named("93.184.216.34", 4200);
        let top = top_domains(&led.domains, 5);
        assert_eq!(top[0].name, "example.com");
        assert_eq!(top[0].bytes, 4200);
    }

    #[test]
    fn the_destination_list_stays_bounded() {
        let mut domains = Vec::new();
        for i in 0..(MAX_DOMAINS + 20) {
            add_domain(&mut domains, &format!("host{i}.example.com"), 1 + i as u64);
        }
        assert_eq!(domains.len(), MAX_DOMAINS);
        // The busiest survived, the cheapest went.
        assert!(domains.iter().any(|d| d.name == "host275.example.com"));
        assert!(!domains.iter().any(|d| d.name == "host0.example.com"));
    }

    #[test]
    fn a_full_binding_map_refuses_new_names_and_keeps_old_ones() {
        let mut bindings = HashMap::new();
        for i in 0..MAX_BINDINGS {
            bind(&mut bindings, &format!("10.0.{}.{}", i / 256, i % 256), "x.example.com");
        }
        bind(&mut bindings, "10.9.9.9", "late.example.com");
        assert_eq!(bindings.len(), MAX_BINDINGS);
        assert!(!bindings.contains_key("10.9.9.9"));
        // An address already known can still be re-pointed.
        bind(&mut bindings, "10.0.0.0", "new.example.com");
        assert_eq!(bindings["10.0.0.0"], "new.example.com");
    }

    #[test]
    fn a_hostile_policy_file_is_bounded() {
        // The file is in the user's profile and the loop is elevated, so its
        // contents arrive untrusted: a filter string, a list length, and a
        // session lifetime all have to be constrained before they are obeyed.
        let text = r#"{"seq":1,"ips":["192.168.1.39","not-an-address","10.0.0.1 and true"],
            "blocked":["1.2.3.4","x"],"caps":{"9.9.9.9":{"up_bps":1,"down_bps":1},"y":{"up_bps":1,"down_bps":1}},
            "ttl_secs":99999999}"#;
        let p = Policy::parse(text).expect("a whole policy parses");
        assert_eq!(p.ips, vec!["192.168.1.39"], "only addresses survive");
        assert_eq!(p.blocked, vec!["1.2.3.4"]);
        assert_eq!(p.caps.len(), 1);
        assert!(p.ttl_secs <= MAX_TTL_SECS);
    }

    #[test]
    fn a_half_written_policy_is_policy_absent() {
        assert!(Policy::parse("{\"seq\":1,").is_none());
        assert!(Policy::parse("").is_none());
        let ok = r#"{"seq":7,"ips":["192.168.1.39"],"blocked":[],"caps":{},"ttl_secs":900}"#;
        let p = Policy::parse(ok).expect("a whole policy parses");
        assert_eq!(p.seq, 7);
        assert_eq!(p.watched(), vec!["192.168.1.39"]);
    }

    #[test]
    fn per_device_snapshot_is_ordered_and_carries_the_session_facts() {
        let mut led = Ledger::new(&["192.168.1.40".to_string(), "192.168.1.39".to_string()]);
        let p = policy(&["192.168.1.39", "192.168.1.40"], &[], &[]);
        led.admit(&p, "192.168.1.39", "1.1.1.1", 10, Instant::now());
        let s = led.snapshot(3, true, "");
        assert_eq!(s.seq, 3);
        assert!(s.running);
        assert_eq!(s.devices[0].ip, "192.168.1.39", "the snapshot is ordered by address");
        assert_eq!(s.devices[1].ip, "192.168.1.40");
        assert_eq!(s.forwarded, 1);
    }
}
