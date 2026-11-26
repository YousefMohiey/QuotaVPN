//! Direct Spaces path: a Noise IK session over UDP.
//!
//! This is the transport under the direct data plane, not the relay. It uses
//! the Noise protocol framework rather than a hand-written handshake, so the
//! key schedule, nonce handling and rekey rules come from a reviewed
//! composition:
//!
//! `Noise_IK_25519_ChaChaPoly_BLAKE2s`
//!
//! IK means the initiator already knows the responder's static public key,
//! which the coordinator hands out with the member list. Both sides know each
//! other's static key and verify it during the handshake. The session has
//! forward secrecy from the ephemeral keys.
//!
//! This module only moves opaque datagrams. The IP/Wintun layer sits above it.

use base64::Engine;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::hmac;
use snow::params::NoiseParams;
use snow::Builder;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const PARAMS: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"quota-spaces-direct-v1";
const FRAME_HANDSHAKE: u8 = 1;
const FRAME_DATA: u8 = 2;
const DATA_HEADER: usize = 1 + 8;
const TAG: usize = 16;

/// One member's static X25519 identity, decoded from the Space record.
pub struct PathIdentity {
    private: Vec<u8>,
    public: Vec<u8>,
}

/// Decode a member's base64 static public key for the Noise handshake.
pub fn decode_public(public_b64: &str) -> Result<Vec<u8>, String> {
    let public = base64::engine::general_purpose::STANDARD
        .decode(public_b64.trim())
        .map_err(|_| "the peer public key is not base64".to_string())?;
    if public.len() != 32 {
        return Err("the peer public key is not 32 bytes".into());
    }
    Ok(public)
}

impl PathIdentity {
    pub fn from_private(private_b64: &str) -> Result<Self, String> {
        let private = base64::engine::general_purpose::STANDARD
            .decode(private_b64.trim())
            .map_err(|_| "the space private key is not base64".to_string())?;
        if private.len() != 32 {
            return Err("the space private key is not 32 bytes".into());
        }
        let public_b64 = crate::spaces::public_key_for(private_b64)?;
        let public = base64::engine::general_purpose::STANDARD
            .decode(public_b64)
            .map_err(|_| "the space public key is not base64".to_string())?;
        Ok(Self { private, public })
    }

    pub fn public(&self) -> &[u8] {
        &self.public
    }
}

/// An established direct session.
///
/// The Noise handshake authenticates the peers and produces a handshake hash.
/// The datagram layer derives separate send and receive keys from that hash and
/// uses an explicit counter with a replay window, because Noise transport
/// messages assume ordered delivery. UDP does not deliver in order, so reusing
/// the Noise transport state directly would drop the session on the first lost
/// packet.
pub struct DirectPath {
    socket: UdpSocket,
    peer: SocketAddr,
    send_key: LessSafeKey,
    recv_key: LessSafeKey,
    send_counter: u64,
    replay: ReplayWindow,
}

impl DirectPath {
    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    fn encrypt(&mut self, payload: &[u8]) -> Result<Vec<u8>, String> {
        let counter = self.send_counter;
        self.send_counter = self.send_counter.wrapping_add(1);
        let counter_bytes = counter.to_be_bytes();
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&counter_bytes);
        let mut cipher = payload.to_vec();
        self.send_key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(&counter_bytes),
                &mut cipher,
            )
            .map_err(|_| "direct encrypt failed".to_string())?;
        let mut packet = Vec::with_capacity(DATA_HEADER + cipher.len());
        packet.push(FRAME_DATA);
        packet.extend_from_slice(&counter_bytes);
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    fn decrypt(&mut self, packet: &[u8]) -> Result<Option<Vec<u8>>, String> {
        if packet.len() < DATA_HEADER + TAG || packet[0] != FRAME_DATA {
            return Ok(None);
        }
        let mut counter_bytes = [0u8; 8];
        counter_bytes.copy_from_slice(&packet[1..9]);
        let counter = u64::from_be_bytes(counter_bytes);
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&counter_bytes);
        let mut cipher = packet[9..].to_vec();
        let plain = match self.recv_key.open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(&counter_bytes),
            &mut cipher,
        ) {
            Ok(plain) => plain,
            Err(_) => return Ok(None),
        };
        if !self.replay.accept(counter) {
            return Ok(None);
        }
        Ok(Some(plain.to_vec()))
    }

    pub fn send(&mut self, payload: &[u8]) -> Result<(), String> {
        let packet = self.encrypt(payload)?;
        self.socket
            .send_to(&packet, self.peer)
            .map_err(|e| format!("direct send: {e}"))?;
        Ok(())
    }

    /// Nonblocking send for the packet loop. A full socket buffer is reported
    /// as `false`, which lets the caller drop the datagram like any UDP stack.
    pub fn try_send(&mut self, payload: &[u8]) -> Result<bool, String> {
        let packet = self.encrypt(payload)?;
        match self.socket.send_to(&packet, self.peer) {
            Ok(_) => Ok(true),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(false)
            }
            Err(e) => Err(format!("direct send: {e}")),
        }
    }

    pub fn set_nonblocking(&self) -> Result<(), String> {
        self.socket
            .set_nonblocking(true)
            .map_err(|e| format!("direct nonblocking: {e}"))
    }

    /// Nonblocking receive for the packet loop. `None` means no payload is
    /// waiting, not that the session is dead.
    pub fn try_recv(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut incoming = [0u8; 2048];
        loop {
            match self.socket.recv_from(&mut incoming) {
                Ok((n, src)) => {
                    if src != self.peer {
                        continue;
                    }
                    if let Some(plain) = self.decrypt(&incoming[..n])? {
                        return Ok(Some(plain));
                    }
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Ok(None)
                }
                Err(e) => return Err(format!("direct receive: {e}")),
            }
        }
    }

    /// Receive one payload. Datagrams from any other source, wrong frame types
    /// and failed tags are ignored until the socket times out, so a stray or
    /// tampered packet cannot confuse a session.
    pub fn recv(&mut self) -> Result<Vec<u8>, String> {
        let mut incoming = [0u8; 2048];
        loop {
            let (n, src) = self
                .socket
                .recv_from(&mut incoming)
                .map_err(|e| format!("direct receive: {e}"))?;
            if src != self.peer {
                continue;
            }
            if let Some(plain) = self.decrypt(&incoming[..n])? {
                return Ok(plain);
            }
        }
    }
}

#[derive(Default)]
struct ReplayWindow {
    initialized: bool,
    last: u64,
    seen: u64,
}

impl ReplayWindow {
    /// Accept a counter exactly once. A 64-packet window tolerates reordering
    /// across the same loss burst that a UDP game session sees.
    fn accept(&mut self, seq: u64) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.last = seq;
            self.seen = 1;
            return true;
        }
        if seq > self.last {
            let shift = seq - self.last;
            self.seen = if shift >= 64 { 0 } else { self.seen << shift };
            self.seen |= 1;
            self.last = seq;
            return true;
        }
        let diff = self.last - seq;
        if diff >= 64 {
            return false;
        }
        let bit = 1u64 << diff;
        if self.seen & bit != 0 {
            return false;
        }
        self.seen |= bit;
        true
    }
}

fn key(bytes: &[u8]) -> Result<LessSafeKey, String> {
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, bytes)
        .map_err(|_| "the direct session key is malformed".to_string())?;
    Ok(LessSafeKey::new(unbound))
}

/// Derive the two datagram keys from the Noise handshake hash. One HMAC block
/// is enough for a 32-byte key, so this is HKDF-Expand with a single block.
fn derive_keys(hash: &[u8], initiator: bool) -> Result<(LessSafeKey, LessSafeKey), String> {
    let prk = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, b"quota-spaces-datagram-v1"),
        hash,
    );
    let i2r = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, prk.as_ref()),
        b"i2r\x01",
    );
    let r2i = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, prk.as_ref()),
        b"r2i\x01",
    );
    let (send, recv) = if initiator {
        (i2r, r2i)
    } else {
        (r2i, i2r)
    };
    Ok((key(send.as_ref())?, key(recv.as_ref())?))
}

fn params() -> Result<NoiseParams, String> {
    PARAMS
        .parse()
        .map_err(|e| format!("noise parameters: {e}"))
}

/// Initiator side: send the IK message, wait for the responder, return the
/// transport. The caller has already established UDP reachability.
pub fn connect(
    socket: UdpSocket,
    peer: SocketAddr,
    local: &PathIdentity,
    remote_public: &[u8],
    timeout: Duration,
) -> Result<DirectPath, String> {
    if remote_public.len() != 32 {
        return Err("the peer static key is not 32 bytes".into());
    }
    let mut handshake = Builder::new(params()?)
        .local_private_key(&local.private)
        .remote_public_key(remote_public)
        .prologue(PROLOGUE)
        .build_initiator()
        .map_err(|e| format!("direct handshake: {e}"))?;
    let mut out = vec![0u8; 512];
    out[0] = FRAME_HANDSHAKE;
    let n = handshake
        .write_message(&[], &mut out[1..])
        .map_err(|e| format!("direct hello: {e}"))?;
    socket
        .send_to(&out[..n + 1], peer)
        .map_err(|e| format!("direct hello send: {e}"))?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("direct timeout: {e}"))?;
    let started = Instant::now();
    let mut incoming = [0u8; 1024];
    loop {
        if started.elapsed() >= timeout {
            return Err("the direct handshake timed out".into());
        }
        let (len, src) = match socket.recv_from(&mut incoming) {
            Ok(pair) => pair,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Err("the direct handshake timed out".into())
            }
            Err(e) => return Err(format!("direct hello receive: {e}")),
        };
        if src != peer || len < 2 || incoming[0] != FRAME_HANDSHAKE {
            continue;
        }
        let mut payload = [0u8; 64];
        handshake
            .read_message(&incoming[1..len], &mut payload)
            .map_err(|e| format!("direct handshake reply: {e}"))?;
        break;
    }
    let hash = handshake.get_handshake_hash().to_vec();
    let (send_key, recv_key) = derive_keys(&hash, true)?;
    Ok(DirectPath {
        socket,
        peer,
        send_key,
        recv_key,
        send_counter: 0,
        replay: ReplayWindow::default(),
    })
}

/// Responder side: wait for the IK message from the expected peer, verify the
/// initiator's static key, reply, and return the transport.
pub fn accept(
    socket: UdpSocket,
    local: &PathIdentity,
    expected_public: &[u8],
    expected_peer: SocketAddr,
    timeout: Duration,
) -> Result<DirectPath, String> {
    if expected_public.len() != 32 {
        return Err("the peer static key is not 32 bytes".into());
    }
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("direct timeout: {e}"))?;
    let started = Instant::now();
    let mut incoming = [0u8; 2048];
    let mut handshake = Builder::new(params()?)
        .local_private_key(&local.private)
        .prologue(PROLOGUE)
        .build_responder()
        .map_err(|e| format!("direct responder: {e}"))?;
    loop {
        if started.elapsed() >= timeout {
            return Err("the direct handshake timed out".into());
        }
        let (len, src) = match socket.recv_from(&mut incoming) {
            Ok(pair) => pair,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Err("the direct handshake timed out".into())
            }
            Err(e) => return Err(format!("direct hello receive: {e}")),
        };
        if src != expected_peer || len < 2 || incoming[0] != FRAME_HANDSHAKE {
            continue;
        }
        let mut payload = [0u8; 64];
        handshake
            .read_message(&incoming[1..len], &mut payload)
            .map_err(|e| format!("direct hello: {e}"))?;
        let remote = handshake
            .get_remote_static()
            .ok_or("the direct hello carried no static key")?;
        if remote != expected_public {
            return Err("the direct hello named the wrong member".into());
        }
        let mut out = vec![0u8; 512];
        out[0] = FRAME_HANDSHAKE;
        let n = handshake
            .write_message(&[], &mut out[1..])
            .map_err(|e| format!("direct ack: {e}"))?;
        socket
            .send_to(&out[..n + 1], src)
            .map_err(|e| format!("direct ack send: {e}"))?;
        break;
    }
    let hash = handshake.get_handshake_hash().to_vec();
    let (send_key, recv_key) = derive_keys(&hash, false)?;
    Ok(DirectPath {
        socket,
        peer: expected_peer,
        send_key,
        recv_key,
        send_counter: 0,
        replay: ReplayWindow::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> (PathIdentity, Vec<u8>) {
        let (private, _) = crate::spaces::wireguard_keypair();
        let id = PathIdentity::from_private(&private).expect("identity");
        let public = id.public().to_vec();
        (id, public)
    }

    #[test]
    fn two_peers_exchange_an_encrypted_datagram() {
        let (a_id, a_pub) = identity();
        let (b_id, b_pub) = identity();
        let a = UdpSocket::bind("127.0.0.1:0").expect("a");
        let b = UdpSocket::bind("127.0.0.1:0").expect("b");
        let a_addr = a.local_addr().expect("a addr");
        let b_addr = b.local_addr().expect("b addr");
        let responder = std::thread::spawn(move || {
            let mut path = accept(
                b,
                &b_id,
                &a_pub,
                a_addr,
                Duration::from_secs(3),
            )
            .expect("accept");
            let got = path.recv().expect("recv");
            assert_eq!(got, b"hello");
            path.send(b"world").expect("send");
        });
        let mut path = connect(
            a,
            b_addr,
            &a_id,
            &b_pub,
            Duration::from_secs(3),
        )
        .expect("connect");
        path.send(b"hello").expect("send");
        let reply = path.recv().expect("reply");
        assert_eq!(reply, b"world");
        responder.join().expect("responder");
    }

    #[test]
    fn a_tampered_datagram_fails_to_decrypt() {
        let (a_id, a_pub) = identity();
        let (b_id, b_pub) = identity();
        let a = UdpSocket::bind("127.0.0.1:0").expect("a");
        let b = UdpSocket::bind("127.0.0.1:0").expect("b");
        let a_addr = a.local_addr().expect("a addr");
        let b_addr = b.local_addr().expect("b addr");
        let responder = std::thread::spawn(move || {
            let mut path = accept(
                b,
                &b_id,
                &a_pub,
                a_addr,
                Duration::from_secs(3),
            )
            .expect("accept");
            let got = path.recv().expect("valid packet after the tampered one");
            assert_eq!(got, b"ok");
        });
        let mut path = connect(
            a,
            b_addr,
            &a_id,
            &b_pub,
            Duration::from_secs(3),
        )
        .expect("connect");
        // Put a well-shaped data frame with a bogus ciphertext on the wire,
        // then a real one. The responder must drop the first and deliver the
        // second.
        let mut bogus = vec![FRAME_DATA];
        bogus.extend_from_slice(&[0u8; 8]);
        bogus.extend_from_slice(&[0u8; TAG]);
        path.socket
            .send_to(&bogus, b_addr)
            .expect("tampered send");
        path.send(b"ok").expect("valid send");
        responder.join().expect("responder");
    }

    #[test]
    fn the_replay_window_accepts_reordering_once() {
        let mut window = ReplayWindow::default();
        assert!(window.accept(10));
        assert!(window.accept(12));
        assert!(window.accept(11));
        assert!(!window.accept(11));
        assert!(!window.accept(10));
        assert!(window.accept(70));
        assert!(!window.accept(0));
    }
}
