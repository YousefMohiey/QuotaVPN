//! Spaces: a private layer-3 LAN carried by WireGuard.
//!
//! The coordinator owns membership and addressing. This module owns the parts
//! that must behave identically on Windows and Android: the invite format, the
//! WireGuard keypair, the persisted space record and the endpoint that the
//! engine config gains while a space is active.
//!
//! No network I/O here. The desktop talks to the coordinator with reqwest; the
//! phone will reuse the same records when its screen lands.

use serde::{Deserialize, Serialize};

/// One member as the coordinator reports it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpaceMember {
    pub id: String,
    pub name: String,
    pub address: String,
    /// The member's static X25519 public key, used to authenticate the direct
    /// encrypted channel. Not secret; the coordinator hands it out.
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub self_pc: bool,
    #[serde(default)]
    pub owner: bool,
    /// The member's Ethernet address on a layer-2 space, `02:51:56:..`.
    #[serde(default)]
    pub mac: String,
    #[serde(default)]
    pub last_seen: i64,
}

/// A space this install belongs to. The private key and token are secrets and
/// are never sent to the webview: the UI only sees the view built from this.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpaceRecord {
    pub id: String,
    pub name: String,
    pub member_id: String,
    /// Local address, always a `/32`.
    pub address: String,
    /// The space's `/24`.
    pub subnet: String,
    /// Base64 WireGuard private key. Generated here, never uploaded.
    pub private_key: String,
    /// Base64 WireGuard public key, derived from the private key.
    pub public_key: String,
    /// Bearer token for the coordinator. Returned once at create or join.
    pub token: String,
    pub hub_public_key: String,
    pub hub_endpoint: String,
    pub hub_port: u16,
    #[serde(default)]
    pub owner: bool,
    /// `l3` or `l2`. Empty means the record predates the field and is layer 3.
    #[serde(default)]
    pub mode: String,
    /// Invite code, kept only on the owner's install so it can be shown again.
    #[serde(default)]
    pub invite: String,
    #[serde(default)]
    pub members: Vec<SpaceMember>,
    #[serde(default)]
    pub last_seen: i64,
}

impl SpaceRecord {
    /// The engine fragment for this space. `None` when the record is not valid
    /// enough to build a tunnel, so a damaged config can never poison a connect.
    pub fn tunnel(&self) -> Option<SpaceTunnel> {
        let tunnel = SpaceTunnel {
            private_key: self.private_key.clone(),
            address: self.address.clone(),
            hub_public_key: self.hub_public_key.clone(),
            hub_endpoint: self.hub_endpoint.clone(),
            hub_port: self.hub_port,
            subnet: self.subnet.clone(),
        };
        tunnel.valid().ok().map(|_| tunnel)
    }

    /// True when this space carries Ethernet frames instead of IP packets.
    pub fn layer2(&self) -> bool {
        self.mode.eq_ignore_ascii_case("l2")
    }

    /// The Ethernet address this member uses on a layer-2 space. Derived from
    /// the member id, so every PC computes the same value without a round trip
    /// and the locally administered prefix keeps it out of vendor space.
    pub fn mac(&self) -> [u8; 6] {
        member_mac(&self.member_id)
    }
}

/// `02:51:56:` plus three bytes of the member id. A space with 16 members has a
/// collision chance of about one in a million; the coordinator can reject a
/// duplicate and reissue the id if that ever happens.
pub fn member_mac(member_id: &str) -> [u8; 6] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"quota-spaces-mac-v1");
    hasher.update(member_id.as_bytes());
    let digest = hasher.finalize();
    [0x02, 0x51, 0x56, digest[0], digest[1], digest[2]]
}

/// The minimum needed to add a space endpoint to an engine config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceTunnel {
    pub private_key: String,
    /// `10.77.<space>.<member>/32`. A /32 is deliberate: sing-box rewrites
    /// traffic to the endpoint's own address to loopback, so a /24 here would
    /// turn every peer address into this machine.
    pub address: String,
    pub hub_public_key: String,
    /// The hub's own address, `10.77.0.1`, reached through the main tunnel.
    pub hub_endpoint: String,
    pub hub_port: u16,
    /// `10.77.<space>.0/24`.
    pub subnet: String,
}

impl SpaceTunnel {
    pub fn valid(&self) -> Result<(), String> {
        let local = cidr_v4(&self.address).ok_or("the space address is not IPv4 CIDR")?;
        if local.2 != 32 {
            return Err("the space address must be a /32".into());
        }
        let subnet = cidr_v4(&self.subnet).ok_or("the space subnet is not IPv4 CIDR")?;
        if subnet.2 != 24 {
            return Err("the space subnet must be a /24".into());
        }
        if !same_v4_24(local.0, subnet.0) {
            return Err("the space address is outside its subnet".into());
        }
        if self.hub_endpoint.trim().is_empty() {
            return Err("the space hub has no address".into());
        }
        if self.hub_port == 0 {
            return Err("the space hub has no port".into());
        }
        key_bytes(&self.private_key)?;
        key_bytes(&self.hub_public_key)?;
        Ok(())
    }

    /// The sing-box endpoint object. Kept as JSON here so both apps and the
    /// tests see exactly the same shape, and the route rule is built from the
    /// same struct rather than a second copy of the subnet.
    pub fn endpoint_json(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "wireguard",
            "tag": "space",
            "address": [self.address],
            "private_key": self.private_key,
            "mtu": 1280,
            // The main outbound carries these UDP packets, so the DPI sees the
            // already-obfuscated tunnel and never a bare WireGuard handshake.
            "detour": "proxy",
            "peers": [{
                "address": self.hub_endpoint,
                "port": self.hub_port,
                "public_key": self.hub_public_key,
                "allowed_ips": [self.subnet],
                "persistent_keepalive_interval": 25
            }]
        })
    }

    pub fn route_rule(&self) -> serde_json::Value {
        serde_json::json!({"ip_cidr": [self.subnet], "outbound": "space"})
    }
}

/// An invite code as the user pastes it: `qvs1:<space id>:<32 hex>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invite {
    pub space_id: String,
    pub secret: String,
}

/// Crockford base32 without the ambiguous letters. The coordinator generates
/// from the same alphabet.
const SPACE_ID_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn valid_space_id(text: &str) -> bool {
    text.len() == 8
        && text
            .bytes()
            .all(|b| SPACE_ID_ALPHABET.contains(&b.to_ascii_uppercase()))
}

pub fn parse_invite(text: &str) -> Result<Invite, String> {
    let t = text.trim();
    let rest = t
        .strip_prefix("qvs1:")
        .ok_or("that is not a space invite (expected qvs1:<id>:<secret>)")?;
    let (id_text, secret) = rest
        .split_once(':')
        .ok_or("the invite is missing its secret")?;
    let space_id = id_text.trim().to_ascii_uppercase();
    if !valid_space_id(&space_id) {
        return Err("the space id in that invite is malformed".into());
    }
    let secret = secret.trim().to_ascii_lowercase();
    if secret.len() != 32 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the invite secret is not 32 hex characters".into());
    }
    Ok(Invite { space_id, secret })
}

/// Build the code the coordinator hands out. Kept here so the server helper and
/// the client tests cannot drift on the shape.
pub fn make_invite(space_id: &str, secret_hex: &str) -> Result<String, String> {
    let id = space_id.trim().to_ascii_uppercase();
    if !valid_space_id(&id) {
        return Err("the space id is malformed".into());
    }
    let secret = secret_hex.trim().to_ascii_lowercase();
    if secret.len() != 32 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the invite secret is not 32 hex characters".into());
    }
    Ok(format!("qvs1:{id}:{secret}"))
}

/// A WireGuard keypair, base64 as the engine and `wg` expect.
///
/// `x25519-dalek` clamps inside the scalar multiplication, so the bytes written
/// here are exactly the bytes `wg genkey` would write: 32 random bytes, base64.
pub fn wireguard_keypair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    let secret = x25519_dalek::StaticSecret::from(bytes);
    let public = x25519_dalek::PublicKey::from(&secret);
    (
        b64_encode(&secret.to_bytes()),
        b64_encode(public.as_bytes()),
    )
}

/// Recover the public key from a stored private key. Used when an old record
/// has the private half but no public half.
pub fn public_key_for(private_b64: &str) -> Result<String, String> {
    let bytes = key_bytes(private_b64)?;
    let secret = x25519_dalek::StaticSecret::from(bytes);
    let public = x25519_dalek::PublicKey::from(&secret);
    Ok(b64_encode(public.as_bytes()))
}

fn b64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn key_bytes(text: &str) -> Result<[u8; 32], String> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|_| "a space key is not base64".to_string())?;
    if raw.len() != 32 {
        return Err("a space key is not 32 bytes".into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}

/// Parse `a.b.c.d/n` without a dependency. Returns the network address, the
/// host address and the prefix length; the two addresses are the same when the
/// caller supplied a network address.
fn cidr_v4(text: &str) -> Option<(std::net::Ipv4Addr, std::net::Ipv4Addr, u8)> {
    let (addr_text, prefix_text) = text.trim().split_once('/')?;
    let addr: std::net::Ipv4Addr = addr_text.parse().ok()?;
    let prefix: u8 = prefix_text.parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let network = std::net::Ipv4Addr::from(u32::from(addr) & mask);
    Some((network, addr, prefix))
}

fn same_v4_24(a: std::net::Ipv4Addr, b: std::net::Ipv4Addr) -> bool {
    (u32::from(a) & 0xffff_ff00) == (u32::from(b) & 0xffff_ff00)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel() -> SpaceTunnel {
        let (private_key, _) = wireguard_keypair();
        SpaceTunnel {
            private_key,
            address: "10.77.7.9/32".into(),
            hub_public_key: {
                let (_, public) = wireguard_keypair();
                public
            },
            hub_endpoint: "10.77.0.1".into(),
            hub_port: 51821,
            subnet: "10.77.7.0/24".into(),
        }
    }

    #[test]
    fn a_keypair_is_two_different_32_byte_keys() {
        let (private, public) = wireguard_keypair();
        assert_eq!(key_bytes(&private).expect("private").len(), 32);
        assert_eq!(key_bytes(&public).expect("public").len(), 32);
        assert_ne!(private, public);
        assert_eq!(public_key_for(&private).expect("derive"), public);
    }

    #[test]
    fn an_invite_round_trips_and_rejects_the_wrong_shape() {
        let code = make_invite("k7m2q9x4", "00112233445566778899aabbccddeeff").expect("make");
        assert_eq!(code, "qvs1:K7M2Q9X4:00112233445566778899aabbccddeeff");
        let invite = parse_invite(&code).expect("parse");
        assert_eq!(invite.space_id, "K7M2Q9X4");
        assert_eq!(invite.secret, "00112233445566778899aabbccddeeff");
        assert!(parse_invite("qvs1:short:00112233445566778899aabbccddeeff").is_err());
        assert!(parse_invite("qvs1:K7M2Q9X4:nothex").is_err());
        assert!(parse_invite("https://example.com").is_err());
    }

    #[test]
    fn the_endpoint_uses_a_32_bit_local_address_and_the_space_24() {
        let t = tunnel();
        let ep = t.endpoint_json();
        assert_eq!(ep["address"][0], "10.77.7.9/32");
        assert_eq!(ep["detour"], "proxy");
        assert_eq!(ep["mtu"], 1280);
        assert_eq!(ep["peers"][0]["allowed_ips"][0], "10.77.7.0/24");
        assert_eq!(ep["peers"][0]["address"], "10.77.0.1");
        assert_eq!(ep["peers"][0]["port"], 51821);
        assert_eq!(t.route_rule()["ip_cidr"][0], "10.77.7.0/24");
        assert_eq!(t.route_rule()["outbound"], "space");
    }

    #[test]
    fn a_space_address_must_be_inside_its_subnet_and_a_32() {
        let mut t = tunnel();
        assert!(t.valid().is_ok());
        t.address = "10.77.7.9/24".into();
        assert!(t.valid().is_err());
        t.address = "10.77.8.9/32".into();
        assert!(t.valid().is_err());
        t.address = "10.77.7.9/32".into();
        t.subnet = "10.77.7.0/16".into();
        assert!(t.valid().is_err());
    }

    #[test]
    fn a_member_mac_is_stable_and_locally_administered() {
        let a = member_mac("abc123");
        assert_eq!(a, member_mac("abc123"));
        assert_eq!(&a[..3], &[0x02, 0x51, 0x56]);
        assert_ne!(a, member_mac("abc124"));
    }

    #[test]
    fn a_record_with_a_bad_key_cannot_build_a_tunnel() {
        let mut r = SpaceRecord {
            private_key: wireguard_keypair().0,
            ..SpaceRecord::default()
        };
        r.address = "10.77.7.9/32".into();
        r.subnet = "10.77.7.0/24".into();
        r.hub_public_key = wireguard_keypair().1;
        r.hub_endpoint = "10.77.0.1".into();
        r.hub_port = 51821;
        assert!(r.tunnel().is_some());
        r.private_key = "not base64".into();
        assert!(r.tunnel().is_none());
    }
}
