//! Route one LAN device through the tunnel without installing anything on it.
//!
//! The device keeps its own settings; what changes is what it believes about the
//! router. We send it a forged ARP reply saying the gateway address now lives at our
//! MAC, it sends its traffic to us, and since we forward and its next hop is the
//! engine's tun, its traffic comes out of the tunnel.
//!
//! Three things make this safe to ship or not ship at all:
//!
//! - an intention record is written BEFORE the first frame, so a crash cannot leave a
//!   device pointing at a machine that stopped forwarding;
//! - the restore frame names the real router MAC, and is sent on stop and on start-up
//!   whenever an intention exists;
//! - every step is read back. A forwarding state that cannot be verified is a failure
//!   reported to the caller, never a silent success.
//!
//! Only Windows uses this. The phone's tunnel is a real VpnService and needs no tricks.

use std::net::Ipv4Addr;
use std::path::PathBuf;

/// Ethernet frame carrying an ARP reply. Fixed size: 14 byte header, 28 byte body.
pub const FRAME_LEN: usize = 42;

const ETHERTYPE_ARP: u16 = 0x0806;
const ARP_REPLY: u16 = 2;
const HWTYPE_ETHERNET: u16 = 1;
const PROTO_IPV4: u16 = 0x0800;

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

/// An ARP reply that tells `target_ip` that `claimed_ip` is at `sender_mac`.
///
/// This is the whole mechanism. Called with our MAC it takes the device over; called
/// with the router's MAC it hands it back. Same shape, opposite direction, which is
/// why the restore path is testable against the claim path.
pub fn arp_reply(
    sender_mac: &str,
    claimed_ip: Ipv4Addr,
    target_mac: &str,
    target_ip: Ipv4Addr,
) -> Option<[u8; FRAME_LEN]> {
    let smac = mac_bytes(sender_mac)?;
    let tmac = mac_bytes(target_mac)?;
    let mut f = [0u8; FRAME_LEN];

    // Ethernet: the target is the unicast destination, we are the source.
    f[0..6].copy_from_slice(&tmac);
    f[6..12].copy_from_slice(&smac);
    f[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());

    // ARP body.
    f[14..16].copy_from_slice(&HWTYPE_ETHERNET.to_be_bytes());
    f[16..18].copy_from_slice(&PROTO_IPV4.to_be_bytes());
    f[18] = 6; // hardware address length
    f[19] = 4; // protocol address length
    f[20..22].copy_from_slice(&ARP_REPLY.to_be_bytes());
    f[22..28].copy_from_slice(&smac); // sender hardware
    f[28..32].copy_from_slice(&claimed_ip.octets()); // sender protocol
    f[32..38].copy_from_slice(&tmac); // target hardware
    f[38..42].copy_from_slice(&target_ip.octets()); // target protocol
    Some(f)
}

/// The frame that hands a device back to the router.
pub fn arp_release(
    router_mac: &str,
    gateway_ip: Ipv4Addr,
    target_mac: &str,
    target_ip: Ipv4Addr,
) -> Option<[u8; FRAME_LEN]> {
    arp_reply(router_mac, gateway_ip, target_mac, target_ip)
}

// ---------------------------------------------------------------- intention record

fn intent_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("quotavpn").join("lan_route_intent.txt")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub target_ip: String,
    pub target_mac: String,
    pub gateway_ip: String,
    pub interface: String,
}

impl Intent {
    pub fn encode(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.target_ip, self.target_mac, self.gateway_ip, self.interface
        )
    }

    pub fn decode(line: &str) -> Option<Intent> {
        let p: Vec<&str> = line.trim().split('|').collect();
        if p.len() != 4 || p.iter().any(|s| s.is_empty()) {
            return None;
        }
        Some(Intent {
            target_ip: p[0].to_string(),
            target_mac: p[1].to_string(),
            gateway_ip: p[2].to_string(),
            interface: p[3].to_string(),
        })
    }
}

/// Written before the first forged frame. Its existence is the promise that a
/// restore is owed, and that is the only reason a crash is survivable here.
pub fn remember_intent(intent: &Intent) -> Result<(), String> {
    let path = intent_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, intent.encode()).map_err(|e| e.to_string())
}

pub fn read_intent() -> Option<Intent> {
    let text = std::fs::read_to_string(intent_path()).ok()?;
    Intent::decode(&text)
}

pub fn forget_intent() {
    let _ = std::fs::remove_file(intent_path());
}

// ---------------------------------------------------------------- forwarding control

/// Read back whether this interface forwards. Parsing `netsh` is ugly but it is the
/// same source of truth the user sees, and the whole feature depends on it being on.
pub fn forwarding_enabled(interface: &str) -> Result<bool, String> {
    let out = std::process::Command::new("netsh")
        .args(["interface", "ipv4", "show", "interface", interface])
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    Ok(text.contains("forwarding") && text.contains("enabled") && !text.contains("disabled"))
}

pub fn set_forwarding(interface: &str, on: bool) -> Result<(), String> {
    let state = if on { "enabled" } else { "disabled" };
    let out = std::process::Command::new("netsh")
        .args([
            "interface",
            "ipv4",
            "set",
            "interface",
            interface,
            "forwarding",
            state,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    if forwarding_enabled(interface)? != on {
        return Err(format!("forwarding did not become {state} on {interface}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn the_claim_frame_names_us_as_the_gateway() {
        let f = arp_reply(
            "f0:11:22:33:44:55",
            ip("192.168.1.1"),
            "aa:bb:cc:dd:ee:ff",
            ip("192.168.1.28"),
        )
        .expect("frame");

        // ethernet
        assert_eq!(&f[0..6], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(&f[6..12], &[0xf0, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(u16::from_be_bytes([f[12], f[13]]), ETHERTYPE_ARP);
        // arp
        assert_eq!(u16::from_be_bytes([f[14], f[15]]), 1);
        assert_eq!(u16::from_be_bytes([f[16], f[17]]), 0x0800);
        assert_eq!((f[18], f[19]), (6, 4));
        assert_eq!(u16::from_be_bytes([f[20], f[21]]), 2, "must be a reply");
        assert_eq!(&f[22..28], &[0xf0, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(&f[28..32], &[192, 168, 1, 1], "claims the gateway address");
        assert_eq!(&f[32..38], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(&f[38..42], &[192, 168, 1, 28]);
    }

    #[test]
    fn the_release_frame_names_the_router_and_is_otherwise_identical() {
        let claim = arp_reply("f0:11:22:33:44:55", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).unwrap();
        let release = arp_release("f0:a9:51:c1:eb:da", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).unwrap();
        // The difference must be exactly the sender hardware address, which appears
        // twice: once in the Ethernet header and once in the ARP body. Nothing else
        // may differ, or the restore would not be the same request reversed.
        assert_ne!(&claim[6..12], &release[6..12], "ethernet source must differ");
        assert_ne!(&claim[22..28], &release[22..28], "arp sender hardware must differ");
        assert_eq!(&claim[22..28], &claim[6..12], "the two copies must agree");
        assert_eq!(&claim[0..6], &release[0..6], "destination unchanged");
        assert_eq!(&claim[12..22], &release[12..22], "ethertype and arp header unchanged");
        assert_eq!(&claim[28..], &release[28..], "claimed and target addresses unchanged");
        assert_eq!(&release[22..28], &[0xf0, 0xa9, 0x51, 0xc1, 0xeb, 0xda]);
    }

    #[test]
    fn a_bad_mac_produces_nothing_rather_than_a_malformed_frame() {
        assert!(arp_reply("not-a-mac", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).is_none());
        assert!(arp_reply("f0:11:22:33:44", ip("192.168.1.1"), "aa:bb:cc:dd:ee:ff", ip("192.168.1.28")).is_none());
        assert!(arp_reply("f0:11:22:33:44:55", ip("192.168.1.1"), "aa:bb", ip("192.168.1.28")).is_none());
    }

    #[test]
    fn an_intention_round_trips_through_the_record() {
        let i = Intent {
            target_ip: "192.168.1.28".into(),
            target_mac: "aa:bb:cc:dd:ee:ff".into(),
            gateway_ip: "192.168.1.1".into(),
            interface: "Ethernet".into(),
        };
        assert_eq!(Intent::decode(&i.encode()), Some(i));
    }

    #[test]
    fn a_damaged_record_is_not_mistaken_for_an_intention() {
        // Half a line would otherwise produce a device with no gateway to restore.
        for bad in ["", "1.2.3.4", "1.2.3.4|aa:bb", "a|b|c|", "|||"] {
            assert_eq!(Intent::decode(bad), None, "accepted {bad:?}");
        }
    }
}
