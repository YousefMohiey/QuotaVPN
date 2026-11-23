//! Carrying one device's traffic without the router's help.
//!
//! This is the part that has to work on a router nobody signed in to. The
//! device is told this PC is the gateway, the router is told this PC is the
//! device, and the frames that arrive are re-addressed and sent back out at
//! layer 2. The PC's own IP stack, routes, firewall and VPN never see them:
//! there is no IP forwarding to switch on, no kernel filter to keep fed, and no
//! way for a busy device to pull this machine's own connection down with it.
//!
//! Three safety rules are the whole difference between this and an outage:
//!
//! * nothing is claimed until the capture is open and a frame has actually been
//!   sent, so a dead driver can never leave a device pointing at a PC that is
//!   not carrying it;
//! * the device is asked for its hardware address on the wire first, so a stale
//!   neighbour entry cannot blackhole a phone whose address changed;
//! * every exit, including a panic, sends the true mappings back, and the
//!   journal is there for the one exit nobody can handle: the process being
//!   killed.
//!
//! IPv4 only. A device using IPv6 talks to the router directly and is not
//! slowed or blocked by this session; the screens say so where it matters.

use crate::wpcap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often the two caches are refreshed with the claim.
const POISON_EVERY: Duration = Duration::from_millis(800);
/// How long one capture read may block, so a stop is never stuck behind a quiet
/// link.
const CAPTURE_TIMEOUT_MS: i32 = 20;
/// How far ahead a cap may queue before frames are dropped instead of delayed.
/// A third of a second of lag makes a capped link feel broken; this keeps the
/// burst small and lets TCP do the adapting.
const PACE_HORIZON: Duration = Duration::from_millis(150);
/// Consecutive failed sends that mean the adapter is gone, not busy.
const MAX_SEND_FAILURES: u32 = 40;

/// One device under this PC's control.
pub struct Session {
    pub target_ip: String,
    pub target_mac: String,
    pub blocked: Arc<AtomicBool>,
    pub down_kbps: Arc<AtomicU32>,
    pub up_kbps: Arc<AtomicU32>,
    pub bytes_up: Arc<AtomicU64>,
    pub bytes_down: Arc<AtomicU64>,
    pub forwarded: Arc<AtomicU64>,
    pub dropped: Arc<AtomicU64>,
    /// ARP replies the device sent to the gateway-shaped request. Proof that
    /// the claim reached its stack, which a plain reply does not give.
    pub client_answers: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Session {
    /// False once the relay has stopped for any reason. The screen uses this
    /// instead of a file: there is no detached pass to outlive the session.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    fn stop_inner(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    /// Stop and wait for the ARP tables to be handed back.
    pub fn stop(mut self) {
        self.stop_inner();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A session that is replaced or dropped without an explicit stop must
        // not leave a live thread poisoning a device nobody owns.
        self.stop_inner();
    }
}

/// Start carrying one device. Nothing on the wire changes until the capture is
/// open, the device has answered for its address, and the first claim went out.
pub fn start_with(
    target_ip: &str,
    blocked: bool,
    known_mac: Option<String>,
) -> Result<Session, String> {
    let w = crate::lan_ctrl::wire().ok_or("No network adapter leads to the router.")?;
    if target_ip == w.gateway_ip {
        return Err("The router cannot be controlled.".into());
    }
    if target_ip == w.ip {
        return Err("This PC cannot be controlled.".into());
    }
    if w.device.is_empty() {
        return Err("Npcap is not installed, or the adapter was not found.".into());
    }
    if w.gateway_ip.is_empty() || w.gateway_mac.is_empty() {
        return Err("The router's address could not be read.".into());
    }
    let target_mac = crate::lan_ctrl::resolve_mac(target_ip, known_mac)
        .ok_or("The device's hardware address is unknown.")?;
    if target_mac == w.mac {
        return Err("The device is using this PC's hardware address.".into());
    }
    if target_mac == w.gateway_mac {
        return Err("The device is using the router's hardware address.".into());
    }

    let (Some(_expected), Some(our), Some(gw), Some(tip), Some(gip), Some(our_ip)) = (
        crate::lan_ctrl::mac_bytes(&target_mac),
        crate::lan_ctrl::mac_bytes(&w.mac),
        crate::lan_ctrl::mac_bytes(&w.gateway_mac),
        crate::lan_ctrl::ip4(target_ip),
        crate::lan_ctrl::ip4(&w.gateway_ip),
        crate::lan_ctrl::ip4(&w.ip),
    ) else {
        return Err("An address on the wire could not be read.".into());
    };

    let stop = Arc::new(AtomicBool::new(false));
    let alive = Arc::new(AtomicBool::new(false));
    let blocked_flag = Arc::new(AtomicBool::new(blocked));
    let down = Arc::new(AtomicU32::new(0));
    let up = Arc::new(AtomicU32::new(0));
    let bytes_up = Arc::new(AtomicU64::new(0));
    let bytes_down = Arc::new(AtomicU64::new(0));
    let forwarded = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let client_answers = Arc::new(AtomicU64::new(0));

    // Written before the first frame goes out, so a killed process is undone on
    // the next launch. The fields are the same ones `repair` already restores.
    crate::lan_ctrl::write_journal(&format!(
        "{}|{}|{}|{}",
        target_ip, target_mac, w.gateway_ip, w.gateway_mac
    ));

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<[u8; 6], String>>();
    let handle = {
        let device = w.device.clone();
        let stop = Arc::clone(&stop);
        let alive = Arc::clone(&alive);
        let blocked = Arc::clone(&blocked_flag);
        let up_kbps = Arc::clone(&up);
        let down_kbps = Arc::clone(&down);
        let bytes_up = Arc::clone(&bytes_up);
        let bytes_down = Arc::clone(&bytes_down);
        let forwarded = Arc::clone(&forwarded);
        let dropped = Arc::clone(&dropped);
        let client_answers = Arc::clone(&client_answers);
        let target = target_ip.to_string();
        std::thread::spawn(move || {
            relay_loop(
                device,
                our,
                our_ip,
                gw,
                tip,
                gip,
                &target,
                stop,
                alive,
                blocked,
                up_kbps,
                down_kbps,
                bytes_up,
                bytes_down,
                forwarded,
                dropped,
                client_answers,
                ready_tx,
            )
        })
    };

    // The worker answers only after the capture is open, the device replied to
    // its probe, and the first claim went out. Until then nothing has changed.
    let learned = match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(mac)) => mac,
        Ok(Err(e)) => {
            stop.store(true, Ordering::SeqCst);
            let _ = handle.join();
            crate::lan_ctrl::clear_journal();
            return Err(e);
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            let _ = handle.join();
            crate::lan_ctrl::clear_journal();
            return Err("The packet capture did not start.".into());
        }
    };

    let learned_text = learned
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");

    Ok(Session {
        target_ip: target_ip.to_string(),
        target_mac: learned_text,
        blocked: blocked_flag,
        down_kbps: down,
        up_kbps: up,
        bytes_up,
        bytes_down,
        forwarded,
        dropped,
        client_answers,
        alive,
        stop,
        handle: Some(handle),
    })
}

/// Send the true mappings back on every way out. The struct exists so a panic
/// in the middle of the loop still restores the two caches.
struct Restore<'a> {
    cap: &'a wpcap::Capture,
    tm: [u8; 6],
    gw: [u8; 6],
    tip: [u8; 4],
    gip: [u8; 4],
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        let to_target = crate::lan_ctrl::arp_reply(self.tm, self.gw, self.gip, self.tm, self.tip);
        let to_gateway = crate::lan_ctrl::arp_reply(self.gw, self.tm, self.tip, self.gw, self.gip);
        for _ in 0..5 {
            let _ = self.cap.send(&to_target);
            let _ = self.cap.send(&to_gateway);
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn relay_loop(
    device: String,
    our: [u8; 6],
    our_ip: [u8; 4],
    gw: [u8; 6],
    tip: [u8; 4],
    gip: [u8; 4],
    target_text: &str,
    stop: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    blocked: Arc<AtomicBool>,
    up_kbps: Arc<AtomicU32>,
    down_kbps: Arc<AtomicU32>,
    bytes_up: Arc<AtomicU64>,
    bytes_down: Arc<AtomicU64>,
    forwarded: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    client_answers: Arc<AtomicU64>,
    ready: std::sync::mpsc::Sender<Result<[u8; 6], String>>,
) {
    // Non-promiscuous on purpose: everything this session carries is addressed
    // to this machine, and promiscuous mode is what breaks some adapters.
    let Some(Some(cap)) = wpcap::with(|w| w.open_mode(&device, false, CAPTURE_TIMEOUT_MS)) else {
        let _ = ready.send(Err(
            "Npcap could not open the adapter. Reinstall Npcap and try again.".into(),
        ));
        return;
    };

    // The address in the app may be a lease old, and its hardware address may
    // have changed with it. Ask the whole segment who holds the address: the
    // device that does answers, and its reply is the address to use. A unicast
    // probe to a stale address would never reach a device that moved.
    let probe = arp_request([0xff; 6], our, our_ip, tip);
    let probe_until = Instant::now() + Duration::from_secs(2);
    let mut learned: Option<[u8; 6]> = None;
    while Instant::now() < probe_until && !stop.load(Ordering::SeqCst) {
        let _ = cap.send(&probe);
        let listen_until = Instant::now() + Duration::from_millis(220);
        while Instant::now() < listen_until {
            match cap.read() {
                wpcap::Read::Frame(frame) => {
                    if let Some(mac) = arp_reply_from(&frame, tip) {
                        if mac == our || mac == gw {
                            continue;
                        }
                        learned = Some(mac);
                        break;
                    }
                }
                wpcap::Read::Timeout => continue,
                wpcap::Read::Error(e) => {
                    let _ = ready.send(Err(format!("Npcap stopped reading: {e}")));
                    return;
                }
            }
        }
        if learned.is_some() {
            break;
        }
    }
    let Some(tm) = learned else {
        let _ = ready.send(Err(
            "The device did not answer for its address. Refresh the list and try again.".into(),
        ));
        return;
    };

    let poison_target = crate::lan_ctrl::arp_reply(tm, our, gip, tm, tip);
    // A reply is what a router accepts, but some client stacks ignore an
    // unsolicited reply and still answer a request. Asking the device for its
    // own address, with the gateway's address as the sender, makes it process
    // a normal request and reply to us. The reply is the proof the claim
    // reached its stack.
    let poison_target_request = arp_request(tm, our, gip, tip);
    let poison_gateway = crate::lan_ctrl::arp_reply(gw, our, tip, gw, gip);

    // Nothing has been claimed yet. Send the claim only if a frame really
    // leaves, so a driver that accepted the open but cannot transmit is a
    // refusal instead of a blackhole.
    let mut sent_any = false;
    for _ in 0..3 {
        sent_any |= cap.send(&poison_target);
        sent_any |= cap.send(&poison_target_request);
        sent_any |= cap.send(&poison_gateway);
        std::thread::sleep(Duration::from_millis(60));
    }
    if !sent_any {
        let _ = ready.send(Err(
            "Npcap opened the adapter but could not transmit. Reinstall Npcap.".into(),
        ));
        return;
    }
    if stop.load(Ordering::SeqCst) {
        // The caller gave up while the handshake was running: undo immediately.
        let _ = cap.send(&crate::lan_ctrl::arp_reply(tm, gw, gip, tm, tip));
        let _ = cap.send(&crate::lan_ctrl::arp_reply(gw, tm, tip, gw, gip));
        return;
    }
    alive.store(true, Ordering::SeqCst);
    let _ = ready.send(Ok(tm));
    crate::vpn::app_log(&format!(
        "lan relay: {} is {} at {}",
        target_text,
        tm.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":"),
        device
    ));

    let restore = Restore { cap: &cap, tm, gw, tip, gip };
    // Ethernet pads short frames; a bare TCP ACK is 54 bytes and some drivers
    // refuse anything under 60. The IP length field still ends where it should.
    let mut padding = [0u8; 60];

    let mut last_poison = Instant::now();
    let mut slot_up = Instant::now();
    let mut slot_down = Instant::now();
    let mut failures = 0u32;
    let mut foreign_gateway_logged = false;

    while !stop.load(Ordering::SeqCst) {
        if last_poison.elapsed() >= POISON_EVERY {
            last_poison = Instant::now();
            let _ = cap.send(&poison_target);
            let _ = cap.send(&poison_target_request);
            let _ = cap.send(&poison_gateway);
        }

        let frame = match cap.read() {
            wpcap::Read::Frame(f) => f,
            wpcap::Read::Timeout => continue,
            wpcap::Read::Error(e) => {
                crate::vpn::app_log(&format!("lan relay: adapter read failed: {e}"));
                break;
            }
        };
        if frame.len() < 14 {
            continue;
        }
        let ether = u16::from_be_bytes([frame[12], frame[13]]);
        let (mut l3, mut proto) = (14usize, ether);
        if ether == 0x8100 && frame.len() >= 18 {
            proto = u16::from_be_bytes([frame[16], frame[17]]);
            l3 = 18;
        }

        if proto == 0x0806 {
            // The caches are refreshed on a timer, but a device that has just
            // asked for the gateway gets an immediate answer so it cannot spend
            // the interval talking to the real one.
            if frame.len() >= l3 + 28 {
                let op = u16::from_be_bytes([frame[l3 + 6], frame[l3 + 7]]);
                if op == 1 {
                    let spa = [frame[l3 + 14], frame[l3 + 15], frame[l3 + 16], frame[l3 + 17]];
                    let tpa = [frame[l3 + 24], frame[l3 + 25], frame[l3 + 26], frame[l3 + 27]];
                    if spa == tip && tpa == gip {
                        let _ = cap.send(&poison_target);
                        let _ = cap.send(&poison_target_request);
                    } else if spa == gip && tpa == tip {
                        let _ = cap.send(&poison_gateway);
                    } else if spa == tip && !foreign_gateway_logged {
                        // A device whose gateway is not the address this
                        // session assumed is a fact worth writing down: no
                        // amount of claiming the wrong address will carry it.
                        foreign_gateway_logged = true;
                        crate::vpn::app_log(&format!(
                            "lan relay: {} asked for gateway {}.{}.{}.{}",
                            target_text, tpa[0], tpa[1], tpa[2], tpa[3]
                        ));
                    }
                } else if op == 2 {
                    // The device answered the gateway-shaped request. That is
                    // the proof the claim reached its stack, and it is the one
                    // reading a plain reply can never give.
                    if let Some(mac) = arp_reply_from(&frame, tip) {
                        if mac == tm {
                            client_answers.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            continue;
        }

        if proto != 0x0800 || frame.len() < l3 + 20 || frame[l3] >> 4 != 4 {
            continue;
        }
        let ihl = (frame[l3] & 0x0f) as usize * 4;
        if ihl < 20 || frame.len() < l3 + ihl {
            continue;
        }
        let src = [
            frame[l3 + 12],
            frame[l3 + 13],
            frame[l3 + 14],
            frame[l3 + 15],
        ];
        let dst = [
            frame[l3 + 16],
            frame[l3 + 17],
            frame[l3 + 18],
            frame[l3 + 19],
        ];
        let src_mac: [u8; 6] = frame[6..12].try_into().unwrap_or([0; 6]);
        let dst_mac: [u8; 6] = frame[0..6].try_into().unwrap_or([0; 6]);

        if src_mac == tm && dst_mac == our && src == tip {
            // Upload. The frame is re-sent with this PC's hardware address as
            // its source: the router already believes the device lives here,
            // and the access point must not learn the phone's address on the
            // wired port and start sending its traffic the wrong way.
            let len = frame.len() as u64;
            bytes_up.fetch_add(len, Ordering::Relaxed);
            if blocked.load(Ordering::Relaxed)
                || !pace(frame.len(), up_kbps.load(Ordering::Relaxed) as u64 * 1024, &mut slot_up)
            {
                dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let mut out = frame;
            out[0..6].copy_from_slice(&gw);
            out[6..12].copy_from_slice(&our);
            if transmit(&cap, &mut out, &mut padding, &mut failures) {
                forwarded.fetch_add(1, Ordering::Relaxed);
            }
        } else if dst_mac == our && src_mac == gw && dst == tip {
            // Download. It arrived here because the router's neighbour entry
            // for the device points at this PC.
            let len = frame.len() as u64;
            bytes_down.fetch_add(len, Ordering::Relaxed);
            if blocked.load(Ordering::Relaxed)
                || !pace(
                    frame.len(),
                    down_kbps.load(Ordering::Relaxed) as u64 * 1024,
                    &mut slot_down,
                )
            {
                dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let mut out = frame;
            out[0..6].copy_from_slice(&tm);
            out[6..12].copy_from_slice(&gw);
            if transmit(&cap, &mut out, &mut padding, &mut failures) {
                forwarded.fetch_add(1, Ordering::Relaxed);
            }
        }

        if failures >= MAX_SEND_FAILURES {
            crate::vpn::app_log("lan relay: the adapter stopped accepting frames");
            break;
        }
    }

    drop(restore);
    alive.store(false, Ordering::SeqCst);
    crate::lan_ctrl::clear_journal();
    crate::vpn::app_log(&format!(
        "lan relay stopped: forwarded {} dropped {} client_answers {}",
        forwarded.load(Ordering::Relaxed),
        dropped.load(Ordering::Relaxed),
        client_answers.load(Ordering::Relaxed)
    ));
}

fn transmit(
    cap: &wpcap::Capture,
    frame: &mut [u8],
    padding: &mut [u8; 60],
    failures: &mut u32,
) -> bool {
    let body: &[u8] = if frame.len() < 60 {
        padding[..frame.len()].copy_from_slice(frame);
        for b in padding.iter_mut().skip(frame.len()) {
            *b = 0;
        }
        &padding[..60]
    } else {
        frame
    };
    if cap.send(body) {
        *failures = 0;
        true
    } else {
        *failures += 1;
        false
    }
}

/// A unicast probe: "who has `target_ip`?" sent to the hardware address the
/// roster gave. Only that device sees it, and its reply proves the address.
fn arp_request(dst_mac: [u8; 6], src_mac: [u8; 6], sender_ip: [u8; 4], target_ip: [u8; 4]) -> Vec<u8> {
    let mut f = Vec::with_capacity(42);
    f.extend_from_slice(&dst_mac);
    f.extend_from_slice(&src_mac);
    f.extend_from_slice(&[0x08, 0x06]);
    f.extend_from_slice(&[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01]);
    f.extend_from_slice(&src_mac);
    f.extend_from_slice(&sender_ip);
    f.extend_from_slice(&[0u8; 6]);
    f.extend_from_slice(&target_ip);
    f
}

/// The hardware address in an ARP reply that says `ip` is at that address.
fn arp_reply_from(frame: &[u8], ip: [u8; 4]) -> Option<[u8; 6]> {
    if frame.len() < 14 {
        return None;
    }
    let ether = u16::from_be_bytes([frame[12], frame[13]]);
    let l3 = if ether == 0x8100 {
        if frame.len() < 18 {
            return None;
        }
        18
    } else {
        14
    };
    if frame.len() < l3 + 28 {
        return None;
    }
    if u16::from_be_bytes([frame[l3 + 6], frame[l3 + 7]]) != 2 {
        return None;
    }
    let sender_ip = [frame[l3 + 14], frame[l3 + 15], frame[l3 + 16], frame[l3 + 17]];
    if sender_ip != ip {
        return None;
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&frame[l3 + 8..l3 + 14]);
    Some(mac)
}

/// A cap expressed as a token bucket. Returns false when the queue is already
/// too far ahead, which is where a frame is dropped. There is no sleeping: one
/// direction at its cap must not stall the other.
fn pace(len: usize, rate_bps: u64, slot: &mut Instant) -> bool {
    if rate_bps == 0 {
        return true;
    }
    let now = Instant::now();
    let at = (*slot).max(now) + Duration::from_secs_f64(len as f64 / rate_bps as f64);
    if at > now + PACE_HORIZON {
        return false;
    }
    *slot = at;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUR: [u8; 6] = [0x04, 0x7c, 0x16, 0x7d, 0xe7, 0x02];
    const GW: [u8; 6] = [0xf0, 0xa9, 0x51, 0xc1, 0xeb, 0xda];
    const TM: [u8; 6] = [0x42, 0xd5, 0x2a, 0x0c, 0xbc, 0x0a];

    #[test]
    fn a_probe_is_a_unicast_request() {
        let f = arp_request(TM, OUR, [192, 168, 1, 52], [192, 168, 1, 28]);
        assert_eq!(&f[0..6], &TM);
        assert_eq!(&f[6..12], &OUR);
        assert_eq!(&f[12..14], &[0x08, 0x06]);
        assert_eq!(u16::from_be_bytes([f[20], f[21]]), 1, "a request, not a reply");
        assert_eq!(&f[28..32], &[192, 168, 1, 52]);
        assert_eq!(&f[38..42], &[192, 168, 1, 28]);
    }

    #[test]
    fn a_reply_is_learned_from_its_sender_fields() {
        let f = crate::lan_ctrl::arp_reply(
            OUR,
            TM,
            [192, 168, 1, 28],
            OUR,
            [192, 168, 1, 52],
        );
        assert_eq!(arp_reply_from(&f, [192, 168, 1, 28]), Some(TM));
        assert_eq!(arp_reply_from(&f, [192, 168, 1, 99]), None);
    }

    #[test]
    fn a_request_is_not_mistaken_for_a_reply() {
        let f = arp_request(TM, OUR, [192, 168, 1, 52], [192, 168, 1, 28]);
        assert_eq!(arp_reply_from(&f, [192, 168, 1, 28]), None);
    }

    #[test]
    fn a_cap_drops_a_burst_instead_of_stalling() {
        let now = Instant::now();
        let mut slot = now;
        let mut passed = 0;
        for _ in 0..200 {
            if pace(1400, 64 * 1024, &mut slot) {
                passed += 1;
            }
        }
        // 150 ms at 64 KB/s is about 9.6 KB, or seven 1400 byte frames.
        assert!((5..=10).contains(&passed), "got {passed}");
    }

    #[test]
    fn zero_means_no_cap() {
        let mut slot = Instant::now();
        for _ in 0..1000 {
            assert!(pace(1500, 0, &mut slot));
        }
    }
}
