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
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
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
    link: Link,
    peer: SocketAddr,
    send_key: LessSafeKey,
    recv_key: LessSafeKey,
    send_counter: u64,
    replay: ReplayWindow,
}

enum Link {
    Udp {
        socket: UdpSocket,
        peer: SocketAddr,
    },
    Tcp {
        stream: TcpStream,
        rx: Vec<u8>,
    },
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
        match &mut self.link {
            Link::Udp { socket, peer } => {
                socket
                    .send_to(&packet, *peer)
                    .map_err(|e| format!("direct send: {e}"))?;
            }
            Link::Tcp { stream, .. } => write_frame(stream, &packet)?,
        }
        Ok(())
    }

    /// Nonblocking send for the packet loop. A full UDP buffer is reported as
    /// `false`, which lets the caller drop the datagram like any UDP stack.
    pub fn try_send(&mut self, payload: &[u8]) -> Result<bool, String> {
        let packet = self.encrypt(payload)?;
        match &mut self.link {
            Link::Udp { socket, peer } => match socket.send_to(&packet, *peer) {
                Ok(_) => Ok(true),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    Ok(false)
                }
                Err(e) => Err(format!("direct send: {e}")),
            },
            Link::Tcp { stream, .. } => {
                write_frame(stream, &packet)?;
                Ok(true)
            }
        }
    }

    pub fn set_nonblocking(&self) -> Result<(), String> {
        match &self.link {
            Link::Udp { socket, .. } => socket
                .set_nonblocking(true)
                .map_err(|e| format!("direct nonblocking: {e}")),
            Link::Tcp { stream, .. } => {
                stream
                    .set_read_timeout(Some(Duration::from_millis(2)))
                    .map_err(|e| format!("direct timeout: {e}"))?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| format!("direct timeout: {e}"))
            }
        }
    }

    /// Nonblocking receive for the packet loop. `None` means no payload is
    /// waiting, not that the session is dead.
    pub fn try_recv(&mut self) -> Result<Option<Vec<u8>>, String> {
        loop {
            let packet = match &mut self.link {
                Link::Udp { socket, peer } => {
                    let mut incoming = [0u8; 2048];
                    match socket.recv_from(&mut incoming) {
                        Ok((n, src)) if src == *peer => Some(incoming[..n].to_vec()),
                        Ok(_) => continue,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            return Ok(None)
                        }
                        Err(e) => return Err(format!("direct receive: {e}")),
                    }
                }
                Link::Tcp { stream, rx } => read_tcp_packet(stream, rx)?,
            };
            let Some(packet) = packet else {
                return Ok(None);
            };
            if let Some(plain) = self.decrypt(&packet)? {
                return Ok(Some(plain));
            }
        }
    }

    /// Receive one payload. Datagrams from any other source, wrong frame types
    /// and failed tags are ignored until the socket times out, so a stray or
    /// tampered packet cannot confuse a session.
    pub fn recv(&mut self) -> Result<Vec<u8>, String> {
        loop {
            let packet = match &mut self.link {
                Link::Udp { socket, peer } => {
                    let mut incoming = [0u8; 2048];
                    match socket.recv_from(&mut incoming) {
                        Ok((n, src)) if src == *peer => incoming[..n].to_vec(),
                        Ok(_) => continue,
                        Err(e) => return Err(format!("direct receive: {e}")),
                    }
                }
                Link::Tcp { stream, rx } => match read_tcp_packet(stream, rx)? {
                    Some(packet) => packet,
                    None => return Err("the direct receive timed out".into()),
                },
            };
            if let Some(plain) = self.decrypt(&packet)? {
                return Ok(plain);
            }
        }
    }
}

fn write_frame(stream: &mut TcpStream, packet: &[u8]) -> Result<(), String> {
    if packet.len() > u16::MAX as usize {
        return Err("the direct frame is too large".into());
    }
    let len = (packet.len() as u16).to_be_bytes();
    stream
        .write_all(&len)
        .map_err(|e| format!("direct send: {e}"))?;
    stream
        .write_all(packet)
        .map_err(|e| format!("direct send: {e}"))
}

fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut len = [0u8; 2];
    stream.read_exact(&mut len).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut
            || e.kind() == std::io::ErrorKind::WouldBlock
        {
            "the direct handshake timed out".to_string()
        } else {
            format!("direct receive: {e}")
        }
    })?;
    let size = u16::from_be_bytes(len) as usize;
    if size == 0 || size > 4096 {
        return Err("the direct frame is malformed".into());
    }
    let mut packet = vec![0u8; size];
    stream
        .read_exact(&mut packet)
        .map_err(|e| format!("direct receive: {e}"))?;
    Ok(packet)
}

fn read_tcp_packet(stream: &mut TcpStream, rx: &mut Vec<u8>) -> Result<Option<Vec<u8>>, String> {
    loop {
        if rx.len() >= 2 {
            let size = u16::from_be_bytes([rx[0], rx[1]]) as usize;
            if size == 0 || size > 4096 {
                return Err("the direct frame is malformed".into());
            }
            if rx.len() >= 2 + size {
                let packet = rx[2..2 + size].to_vec();
                rx.drain(..2 + size);
                return Ok(Some(packet));
            }
        }
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => return Err("the direct peer closed the connection".into()),
            Ok(n) => rx.extend_from_slice(&chunk[..n]),
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
        link: Link::Udp { socket, peer },
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
        link: Link::Udp {
            socket,
            peer: expected_peer,
        },
        peer: expected_peer,
        send_key,
        recv_key,
        send_counter: 0,
        replay: ReplayWindow::default(),
    })
}

/// TCP initiator. The peer is known through the coordinator; the Noise
/// handshake is identical to the UDP one, with a two-byte length around each
/// handshake message and each data frame.
pub fn connect_tcp(
    mut stream: TcpStream,
    local: &PathIdentity,
    remote_public: &[u8],
    timeout: Duration,
) -> Result<DirectPath, String> {
    if remote_public.len() != 32 {
        return Err("the peer static key is not 32 bytes".into());
    }
    let peer = stream
        .peer_addr()
        .map_err(|e| format!("direct peer: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("direct timeout: {e}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("direct timeout: {e}"))?;
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
    write_frame(&mut stream, &out[..n + 1])?;
    let incoming = read_frame(&mut stream)?;
    if incoming.len() < 2 || incoming[0] != FRAME_HANDSHAKE {
        return Err("the direct reply is not a handshake".into());
    }
    let mut payload = [0u8; 64];
    handshake
        .read_message(&incoming[1..], &mut payload)
        .map_err(|e| format!("direct handshake reply: {e}"))?;
    let hash = handshake.get_handshake_hash().to_vec();
    let (send_key, recv_key) = derive_keys(&hash, true)?;
    Ok(DirectPath {
        link: Link::Tcp {
            stream,
            rx: Vec::new(),
        },
        peer,
        send_key,
        recv_key,
        send_counter: 0,
        replay: ReplayWindow::default(),
    })
}

/// TCP responder. The connecting peer must present the expected static key.
pub fn accept_tcp(
    mut stream: TcpStream,
    local: &PathIdentity,
    expected_public: &[u8],
    timeout: Duration,
) -> Result<DirectPath, String> {
    if expected_public.len() != 32 {
        return Err("the peer static key is not 32 bytes".into());
    }
    let peer = stream
        .peer_addr()
        .map_err(|e| format!("direct peer: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("direct timeout: {e}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("direct timeout: {e}"))?;
    let mut handshake = Builder::new(params()?)
        .local_private_key(&local.private)
        .prologue(PROLOGUE)
        .build_responder()
        .map_err(|e| format!("direct responder: {e}"))?;
    let incoming = read_frame(&mut stream)?;
    if incoming.len() < 2 || incoming[0] != FRAME_HANDSHAKE {
        return Err("the direct hello is not a handshake".into());
    }
    let mut payload = [0u8; 64];
    handshake
        .read_message(&incoming[1..], &mut payload)
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
    write_frame(&mut stream, &out[..n + 1])?;
    let hash = handshake.get_handshake_hash().to_vec();
    let (send_key, recv_key) = derive_keys(&hash, false)?;
    Ok(DirectPath {
        link: Link::Tcp {
            stream,
            rx: Vec::new(),
        },
        peer,
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
        match &path.link {
            Link::Udp { socket, .. } => {
                socket.send_to(&bogus, b_addr).expect("tampered send");
            }
            Link::Tcp { .. } => panic!("this test uses UDP"),
        }
        path.send(b"ok").expect("valid send");
        responder.join().expect("responder");
    }

    #[test]
    fn tcp_peers_exchange_an_encrypted_datagram() {
        let (a_id, a_pub) = identity();
        let (b_id, b_pub) = identity();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let responder = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            accept_tcp(stream, &b_id, &a_pub, Duration::from_secs(5)).expect("accept tcp")
        });
        let stream = TcpStream::connect(addr).expect("connect");
        let mut a =
            connect_tcp(stream, &a_id, &b_pub, Duration::from_secs(5)).expect("connect tcp");
        let mut b = responder.join().expect("responder");
        a.set_nonblocking().expect("a nonblocking");
        b.set_nonblocking().expect("b nonblocking");
        a.send(b"hello").expect("send");
        let mut got = None;
        for _ in 0..100 {
            if let Some(value) = b.try_recv().expect("recv") {
                got = Some(value);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(got.expect("payload"), b"hello");
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
