//! The Spaces layer-2 switch.
//!
//! One instance per space. It learns which peer owns a MAC address, sends
//! unicast to that peer only, and floods broadcast, multicast and unknown
//! unicast to everyone else. The adapter side is a `PacketIo`, which is the
//! same seam the layer-3 loop uses, so a TAP adapter and the in-memory test
//! adapter are interchangeable.
//!
//! Rules, in order:
//!
//! 1. Link-only frames never leave the machine: STP, EAPOL and LLDP.
//! 2. A frame whose source MAC is ours is dropped. Windows drops these itself;
//!    a peer that sends one is either looping or lying.
//! 3. A source MAC learned on another peer is a spoof or a loop, so the frame
//!    is dropped.
//! 4. Identical frames inside a short window are dropped. Two paths can deliver
//!    the same frame once the relay exists.
//! 5. A peer over its frame budget is dropped until the bucket refills.
//!
//! Broadcasts do not loop: flooding never returns a frame to its ingress peer,
//! and the source check drops anything that comes back.

use crate::spaces_forward::PacketIo;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// A frame shorter than an Ethernet header is not a frame.
pub const HEADER: usize = 14;
/// The largest frame the switch will carry. The TAP adapter is set to 1380, so
/// 1394 is normal; the slack covers a driver that ignores the MTU setting.
pub const MAX_FRAME: usize = 2048;

const LEARN_TTL: Duration = Duration::from_secs(300);
const DUPLICATE_WINDOW: Duration = Duration::from_millis(250);
const KEEPALIVE: Duration = Duration::from_secs(15);
const FRAMES_PER_SECOND: f32 = 2000.0;
const BURST: f32 = 4000.0;

/// One encrypted path to a member. The switch never looks inside the path.
pub trait PeerLink {
    fn label(&self) -> &str;
    fn send(&mut self, frame: &[u8]) -> Result<(), String>;
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, String>;
}

/// What the switch decided for one frame that came off the adapter.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Drop,
    Unicast(usize),
    Flood(Vec<usize>),
}

#[derive(Clone, Copy)]
struct Learned {
    peer: usize,
    at: Instant,
}

struct Bucket {
    tokens: f32,
    at: Instant,
}

impl Bucket {
    fn new(now: Instant) -> Self {
        Self {
            tokens: BURST,
            at: now,
        }
    }

    fn take(&mut self, now: Instant, frames: f32) -> bool {
        let elapsed = now.saturating_duration_since(self.at).as_secs_f32();
        self.tokens = (self.tokens + elapsed * FRAMES_PER_SECOND).min(BURST);
        self.at = now;
        if self.tokens < frames {
            return false;
        }
        self.tokens -= frames;
        true
    }
}

pub struct Switch {
    local: [u8; 6],
    macs: HashMap<[u8; 6], Learned>,
    seen: HashMap<u64, Instant>,
    buckets: Vec<Bucket>,
}

impl Switch {
    pub fn new(local: [u8; 6], peers: usize) -> Self {
        let now = Instant::now();
        Self {
            local,
            macs: HashMap::new(),
            seen: HashMap::new(),
            buckets: (0..peers).map(|_| Bucket::new(now)).collect(),
        }
    }

    /// The MAC this machine answers on.
    pub fn local_mac(&self) -> [u8; 6] {
        self.local
    }

    /// Number of MACs currently known, for tests and logs.
    pub fn learned(&self) -> usize {
        self.macs.len()
    }

    /// Decide whether a frame from `peer` may reach the adapter.
    pub fn accept_peer_frame(&mut self, peer: usize, frame: &[u8]) -> bool {
        if !self.shape(peer, frame) {
            return false;
        }
        // A peer sending our own MAC is spoofing or looping.
        if frame[6..12] == self.local {
            return false;
        }
        if self.duplicate(frame) {
            return false;
        }
        let src = mac(&frame[6..12]);
        if let Some(entry) = self.macs.get(&src) {
            if entry.peer != peer {
                return false;
            }
        }
        self.macs.insert(
            src,
            Learned {
                peer,
                at: Instant::now(),
            },
        );
        true
    }

    /// Decide where a frame from the adapter goes.
    pub fn route(&mut self, frame: &[u8]) -> Route {
        if frame.len() < HEADER || frame.len() > MAX_FRAME {
            return Route::Drop;
        }
        if link_only(frame) {
            return Route::Drop;
        }
        if local_multicast(frame) || service_udp(frame) {
            return Route::Drop;
        }
        let src = mac(&frame[6..12]);
        if src[0] & 1 == 1 {
            // A multicast or broadcast source is not a valid sender.
            return Route::Drop;
        }
        let dst = mac(&frame[0..6]);
        let now = Instant::now();
        self.prune(now);

        if self.duplicate(frame) {
            return Route::Drop;
        }

        let flood = |switch: &Switch| -> Vec<usize> {
            (0..switch.buckets.len())
                .filter(|peer| switch.buckets[*peer].tokens >= 1.0)
                .collect()
        };

        if broadcast(&dst) || dst[0] & 1 == 1 {
            let targets = flood(self);
            return if targets.is_empty() {
                Route::Drop
            } else {
                Route::Flood(targets)
            };
        }

        match self.macs.get(&dst).map(|e| e.peer) {
            Some(peer) if peer != usize::MAX => Route::Unicast(peer),
            _ => {
                let targets = flood(self);
                if targets.is_empty() {
                    Route::Drop
                } else {
                    Route::Flood(targets)
                }
            }
        }
    }

    /// Charge a frame to a peer's budget. Used for both directions.
    pub fn shape(&mut self, peer: usize, frame: &[u8]) -> bool {
        if frame.len() < HEADER || frame.len() > MAX_FRAME || link_only(frame) {
            return false;
        }
        if local_multicast(frame) || service_udp(frame) {
            return false;
        }
        match self.buckets.get_mut(peer) {
            Some(bucket) => bucket.take(Instant::now(), 1.0),
            None => false,
        }
    }

    fn duplicate(&mut self, frame: &[u8]) -> bool {
        let now = Instant::now();
        let key = fingerprint(frame);
        match self.seen.get(&key) {
            Some(at) if now.saturating_duration_since(*at) < DUPLICATE_WINDOW => true,
            _ => {
                self.seen.insert(key, now);
                if self.seen.len() > 4096 {
                    self.seen.retain(|_, at| {
                        now.saturating_duration_since(*at) < DUPLICATE_WINDOW
                    });
                }
                false
            }
        }
    }

    fn prune(&mut self, now: Instant) {
        self.macs
            .retain(|_, entry| now.saturating_duration_since(entry.at) < LEARN_TTL);
    }
}

fn mac(bytes: &[u8]) -> [u8; 6] {
    let mut out = [0u8; 6];
    out.copy_from_slice(&bytes[..6]);
    out
}

fn broadcast(mac: &[u8; 6]) -> bool {
    *mac == [0xFF; 6]
}

/// STP, EAPOL, LLDP and the rest of the 802.1 control block never cross peers.
fn link_only(frame: &[u8]) -> bool {
    if frame.len() < HEADER {
        return true;
    }
    let control = frame[0] == 0x01 && frame[1] == 0x80 && frame[2] == 0xC2;
    let lldp = frame[12] == 0x88 && frame[13] == 0xCC;
    control || lldp
}

/// mDNS, LLMNR, SSDP and the rest of the 224.0.0.0/24 block are link-local
/// service chatter. They never carry a game lobby, and flooding them to every
/// peer at a few MB/s is what pushed ping replies to 1.6 seconds. Minecraft's
/// 224.0.2.60 and the subnet broadcast are kept.
fn local_multicast(frame: &[u8]) -> bool {
    if frame.len() < 34 || frame[12] != 0x08 || frame[13] != 0x00 {
        // IPv6 multicast (33:33:..) is neighbour discovery, MLD and mDNS; a
        // LAN game does not use it.
        if frame[12] == 0x86 && frame[13] == 0xDD && frame[0] == 0x33 && frame[1] == 0x33 {
            return true;
        }
        return false;
    }
    let dst = [frame[30], frame[31], frame[32], frame[33]];
    (dst[0] == 224 && dst[1] == 0 && dst[2] == 0) || dst == [239, 255, 255, 250]
}

/// NetBIOS, DHCP, SSDP, mDNS and LLMNR are machine chatter, not game traffic.
/// Flooding them at a few hundred frames a second is what put three seconds of
/// queueing in front of the ping replies.
fn service_udp(frame: &[u8]) -> bool {
    if frame.len() < 38 || frame[12] != 0x08 || frame[13] != 0x00 || frame[23] != 17 {
        return false;
    }
    let ihl = ((frame[14] & 0x0F) as usize) * 4;
    let udp = 14 + ihl;
    if frame.len() < udp + 4 {
        return false;
    }
    let source = u16::from_be_bytes([frame[udp], frame[udp + 1]]);
    let destination = u16::from_be_bytes([frame[udp + 2], frame[udp + 3]]);
    matches!(
        destination,
        67 | 68 | 137 | 138 | 1900 | 5353 | 5355
    ) || matches!(source, 67 | 68)
}

/// FNV-1a over the frame. Only needs to be good enough to spot the same frame
/// arriving twice within a quarter of a second.
fn fingerprint(frame: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in frame {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

/// Pump one adapter and every peer until `stop` is set.
pub fn run(
    io: &mut dyn PacketIo,
    peers: &mut [Box<dyn PeerLink>],
    local: [u8; 6],
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut switch = Switch::new(local, peers.len());
    let mut keepalive = Instant::now();
    let debug = std::env::var("QC_SWITCH_DEBUG").map(|v| v == "1").unwrap_or(false);
    while !stop.load(Ordering::Relaxed) {
        let mut busy = false;

        for _ in 0..32 {
            match io.try_read()? {
                Some(frame) => {
                    busy = true;
                    if frame.is_empty() {
                        continue;
                    }
                    let route = switch.route(&frame);
                    if debug {
                        crate::vpn::app_log(&format!(
                            "spaces switch: adapter len {} -> {:?}",
                            frame.len(),
                            route
                        ));
                    }
                    match route {
                        Route::Drop => {}
                        Route::Unicast(peer) => {
                            if switch.shape(peer, &frame) {
                                let _ = peers[peer].send(&frame);
                            }
                        }
                        Route::Flood(targets) => {
                            for peer in targets {
                                if switch.shape(peer, &frame) {
                                    let _ = peers[peer].send(&frame);
                                }
                            }
                        }
                    }
                }
                None => break,
            }
        }

        for (index, peer) in peers.iter_mut().enumerate() {
            for _ in 0..32 {
                match peer.try_recv()? {
                    Some(frame) => {
                        busy = true;
                        if frame.is_empty() {
                            continue;
                        }
                        let accepted = switch.accept_peer_frame(index, &frame);
                        if debug {
                            crate::vpn::app_log(&format!(
                                "spaces switch: peer {} len {} accepted {}",
                                index,
                                frame.len(),
                                accepted
                            ));
                        }
                        if accepted {
                            io.write(&frame)?;
                        }
                    }
                    None => break,
                }
            }
        }

        if keepalive.elapsed() >= KEEPALIVE {
            for peer in peers.iter_mut() {
                let _ = peer.send(&[]);
            }
            keepalive = Instant::now();
        }

        if !busy {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const A: [u8; 6] = [0x02, 0x51, 0x56, 0x00, 0x00, 0x01];
    const B: [u8; 6] = [0x02, 0x51, 0x56, 0x00, 0x00, 0x02];
    const C: [u8; 6] = [0x02, 0x51, 0x56, 0x00, 0x00, 0x03];

    fn frame(dst: [u8; 6], src: [u8; 6], tag: u8) -> Vec<u8> {
        let mut out = vec![0u8; 18];
        out[0..6].copy_from_slice(&dst);
        out[6..12].copy_from_slice(&src);
        out[12] = 0x08;
        out[13] = 0x00;
        out[14] = tag;
        out
    }

    #[test]
    fn unicast_goes_to_the_peer_that_owns_the_mac() {
        let mut switch = Switch::new(A, 3);
        assert!(switch.accept_peer_frame(1, &frame(A, B, 1)));
        assert_eq!(switch.route(&frame(B, A, 2)), Route::Unicast(1));
    }

    #[test]
    fn unknown_unicast_floods_to_every_other_peer() {
        let mut switch = Switch::new(A, 3);
        assert_eq!(switch.route(&frame(B, A, 1)), Route::Flood(vec![0, 1, 2]));
    }

    #[test]
    fn a_frame_that_comes_back_from_a_peer_is_dropped() {
        let mut switch = Switch::new(A, 3);
        let f = frame([0xFF; 6], A, 9);
        assert_eq!(switch.route(&f), Route::Flood(vec![0, 1, 2]));
        // A peer echoing the frame back must not reach the adapter.
        assert!(!switch.accept_peer_frame(1, &f));
    }

    #[test]
    fn a_peer_cannot_claim_another_peers_mac() {
        let mut switch = Switch::new(A, 3);
        assert!(switch.accept_peer_frame(0, &frame(A, B, 1)));
        assert!(!switch.accept_peer_frame(1, &frame(A, B, 2)));
    }

    #[test]
    fn the_same_frame_from_two_paths_is_delivered_once() {
        let mut switch = Switch::new(A, 3);
        let f = frame(A, B, 7);
        assert!(switch.accept_peer_frame(0, &f.clone()));
        assert!(!switch.accept_peer_frame(1, &f));
    }

    #[test]
    fn link_control_frames_never_cross() {
        let mut switch = Switch::new(A, 3);
        let mut lldp = frame([0x01, 0x80, 0xC2, 0x00, 0x00, 0x0E], B, 1);
        lldp[12] = 0x88;
        lldp[13] = 0xCC;
        assert_eq!(switch.route(&lldp), Route::Drop);
        let stp = frame([0x01, 0x80, 0xC2, 0x00, 0x00, 0x00], B, 2);
        assert!(!switch.accept_peer_frame(0, &stp));
    }

    #[test]
    fn the_switch_accepts_its_own_adapters_frames() {
        let mut switch = Switch::new(A, 1);
        assert!(switch.shape(0, &frame([0xFF; 6], A, 1)));
    }

    #[test]
    fn a_peer_frame_from_our_own_mac_is_dropped() {
        let mut switch = Switch::new(A, 1);
        assert!(!switch.accept_peer_frame(0, &frame(B, A, 1)));
    }

    #[test]
    fn a_frame_from_the_adapter_carries_our_mac_and_routes() {
        let mut switch = Switch::new(A, 1);
        assert_eq!(switch.route(&frame([0xFF; 6], A, 1)), Route::Flood(vec![0]));
    }

    fn ip_multicast(dst_ip: [u8; 4], tag: u8) -> Vec<u8> {
        let mut out = vec![0u8; 38];
        out[0..6].copy_from_slice(&[0x01, 0x00, 0x5E, 0x00, 0x00, 0xFB]);
        out[6..12].copy_from_slice(&B);
        out[12] = 0x08;
        out[13] = 0x00;
        out[14] = 0x45;
        out[30..34].copy_from_slice(&dst_ip);
        out[34] = tag;
        out
    }

    #[test]
    fn service_udp_noise_is_not_flooded() {
        let mut switch = Switch::new(A, 1);
        let mut ssdp = vec![0u8; 42];
        ssdp[0..6].copy_from_slice(&[0xFF; 6]);
        ssdp[6..12].copy_from_slice(&B);
        ssdp[12] = 0x08;
        ssdp[13] = 0x00;
        ssdp[14] = 0x45;
        ssdp[23] = 17;
        ssdp[34..36].copy_from_slice(&5000u16.to_be_bytes());
        ssdp[36..38].copy_from_slice(&1900u16.to_be_bytes());
        assert_eq!(switch.route(&ssdp), Route::Drop);

        let mut game = ssdp.clone();
        game[36..38].copy_from_slice(&7777u16.to_be_bytes());
        assert_eq!(switch.route(&game), Route::Flood(vec![0]));
    }

    #[test]
    fn link_local_service_multicast_stays_home() {
        let mut switch = Switch::new(A, 1);
        assert_eq!(switch.route(&ip_multicast([224, 0, 0, 251], 1)), Route::Drop);
        assert_eq!(
            switch.route(&ip_multicast([239, 255, 255, 250], 2)),
            Route::Drop
        );
        assert_eq!(
            switch.route(&ip_multicast([224, 0, 2, 60], 3)),
            Route::Flood(vec![0])
        );

        let mut v6 = vec![0u8; 48];
        v6[0] = 0x33;
        v6[1] = 0x33;
        v6[6..12].copy_from_slice(&B);
        v6[12] = 0x86;
        v6[13] = 0xDD;
        assert_eq!(switch.route(&v6), Route::Drop);
    }

    #[test]
    fn the_budget_stops_a_flood() {
        let mut switch = Switch::new(A, 1);
        let mut accepted = 0;
        for i in 0..BURST as usize + 100 {
            if switch.shape(0, &frame(A, B, (i % 250) as u8)) {
                accepted += 1;
            }
        }
        // A little refill happens while the loop runs, so allow a small slack
        // over the burst size and nothing more.
        assert!(accepted <= BURST as usize + 50);
        assert!(accepted > 3000);
    }
}
