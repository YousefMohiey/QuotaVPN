//! Minimal STUN binding client for the Spaces direct path.
//!
//! The coordinator is reached over HTTPS, but the peer data path is UDP. To
//! learn the public UDP mapping without running a UDP listener on the VPS, the
//! client asks a public STUN server from the same socket it will use for the
//! Noise handshake. The mapped address is then reported to the coordinator
//! over HTTPS.
//!
//! Only IPv4 is supported. A dual-stack Spaces path can add IPv6 later.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// Ask the first reachable STUN server for this socket's public IPv4 mapping.
pub fn discover(
    socket: &UdpSocket,
    servers: &[&str],
    timeout: Duration,
) -> Result<SocketAddr, String> {
    let mut last = "no STUN server answered".to_string();
    for server in servers {
        match discover_one(socket, server, timeout) {
            Ok(address) => return Ok(address),
            Err(error) => last = error,
        }
    }
    Err(last)
}

fn discover_one(
    socket: &UdpSocket,
    server: &str,
    timeout: Duration,
) -> Result<SocketAddr, String> {
    let address = server
        .to_socket_addrs()
        .map_err(|e| format!("resolve {server}: {e}"))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| format!("{server} has no IPv4 address"))?;
    let mut transaction = [0u8; 12];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut transaction);
    let mut request = [0u8; 20];
    request[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    request[2..4].copy_from_slice(&0u16.to_be_bytes());
    request[4..8].copy_from_slice(&MAGIC.to_be_bytes());
    request[8..20].copy_from_slice(&transaction);
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("STUN timeout: {e}"))?;
    socket
        .send_to(&request, address)
        .map_err(|e| format!("STUN send: {e}"))?;
    let started = Instant::now();
    let mut buf = [0u8; 512];
    loop {
        let left = timeout.saturating_sub(started.elapsed());
        if left.is_zero() {
            return Err(format!("{server} did not answer"));
        }
        socket
            .set_read_timeout(Some(left))
            .map_err(|e| format!("STUN timeout: {e}"))?;
        let (n, from) = match socket.recv_from(&mut buf) {
            Ok(pair) => pair,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Err(format!("{server} did not answer"))
            }
            Err(e) => return Err(format!("STUN receive: {e}")),
        };
        if from != address {
            continue;
        }
        if let Some(mapped) = parse_response(&buf[..n], &transaction) {
            return Ok(SocketAddr::V4(mapped));
        }
    }
}

/// Parse a Binding Success Response and return its XOR-MAPPED-ADDRESS.
fn parse_response(packet: &[u8], transaction: &[u8; 12]) -> Option<SocketAddrV4> {
    if packet.len() < 20 {
        return None;
    }
    let kind = u16::from_be_bytes([packet[0], packet[1]]);
    let length = u16::from_be_bytes([packet[2], packet[3]]) as usize;
    let magic = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
    if kind != BINDING_SUCCESS
        || magic != MAGIC
        || &packet[8..20] != transaction
        || length + 20 > packet.len()
    {
        return None;
    }
    let end = 20 + length;
    let mut offset = 20;
    while offset + 4 <= end {
        let attr = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
        let size = u16::from_be_bytes([packet[offset + 2], packet[offset + 3]]) as usize;
        let start = offset + 4;
        let stop = start.checked_add(size)?;
        if stop > end {
            return None;
        }
        if attr == XOR_MAPPED_ADDRESS && size >= 8 && packet[start + 1] == 0x01 {
            let port = u16::from_be_bytes([packet[start + 2], packet[start + 3]]) ^ 0x2112;
            let cookie = MAGIC.to_be_bytes();
            let ip = Ipv4Addr::new(
                packet[start + 4] ^ cookie[0],
                packet[start + 5] ^ cookie[1],
                packet[start + 6] ^ cookie[2],
                packet[start + 7] ^ cookie[3],
            );
            return Some(SocketAddrV4::new(ip, port));
        }
        offset = start + ((size + 3) & !3);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(transaction: &[u8; 12], ip: [u8; 4], port: u16) -> Vec<u8> {
        let cookie = MAGIC.to_be_bytes();
        let xport = port ^ 0x2112;
        let mut attr = Vec::new();
        attr.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        attr.extend_from_slice(&8u16.to_be_bytes());
        attr.push(0);
        attr.push(0x01);
        attr.extend_from_slice(&xport.to_be_bytes());
        attr.extend_from_slice(&[
            ip[0] ^ cookie[0],
            ip[1] ^ cookie[1],
            ip[2] ^ cookie[2],
            ip[3] ^ cookie[3],
        ]);
        let mut packet = Vec::new();
        packet.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
        packet.extend_from_slice(&(attr.len() as u16).to_be_bytes());
        packet.extend_from_slice(&cookie);
        packet.extend_from_slice(transaction);
        packet.extend_from_slice(&attr);
        packet
    }

    #[test]
    fn a_binding_response_yields_the_public_mapping() {
        let tx = [7u8; 12];
        let packet = response(&tx, [203, 0, 113, 9], 51820);
        let mapped = parse_response(&packet, &tx).expect("mapped");
        assert_eq!(mapped.ip(), &Ipv4Addr::new(203, 0, 113, 9));
        assert_eq!(mapped.port(), 51820);
    }

    #[test]
    fn a_wrong_transaction_is_ignored() {
        let packet = response(&[7u8; 12], [203, 0, 113, 9], 51820);
        assert!(parse_response(&packet, &[8u8; 12]).is_none());
    }
}
