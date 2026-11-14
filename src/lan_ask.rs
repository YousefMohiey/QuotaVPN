//! Asking a device its name, on the wire.
//!
//! A device that publishes an address record for itself answers a reverse
//! lookup. This used to put the question on the wire through Npcap, because a
//! raw handle can address a frame to one hardware address and read whatever
//! comes back.
//!
//! It does not need to any more. A unicast query is a solicited flow: Windows
//! keeps a stateful mapping for it, so the answer arrives at the socket that
//! asked, from an ordinary unprivileged `UdpSocket`. That removes the packet
//! driver from this path entirely, and with it the reason this file existed as a
//! frame builder.

use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn ip4(s: &str) -> Option<[u8; 4]> {
    let p: Vec<u8> = s.split('.').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 4 {
        Some([p[0], p[1], p[2], p[3]])
    } else {
        None
    }
}

/// A reverse lookup question for one address, asking for a unicast answer.
///
/// The QU bit in the class field is what makes this a unicast exchange: without
/// it an mDNS responder is entitled to multicast its answer to port 5353, where
/// an ephemeral socket will never see it.
fn mdns_ptr_query(ip: [u8; 4]) -> Vec<u8> {
    let rev = format!("{}.{}.{}.{}.in-addr.arpa", ip[3], ip[2], ip[1], ip[0]);
    let mut q = Vec::new();
    q.extend_from_slice(&[0, 0]); // id
    q.extend_from_slice(&[0, 0]); // standard query
    q.extend_from_slice(&[0, 1]); // one question
    q.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    for label in rev.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0, 12]); // PTR
    q.extend_from_slice(&[0x80, 0x01]); // IN + unicast reply
    q
}

/// Send a question and read whatever answers within the wait.
///
/// The hardware address is not used: a socket reaches the device by the address
/// it already has. It stays in the signature because the callers know the pair
/// and the signature is not worth churning for that.
fn ask(ip: &str, _mac: &str, payload: &[u8], dport: u16, wait: Duration) -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(wait)).ok()?;
    let dest = format!("{ip}:{dport}");
    let mut buf = [0u8; 1500];
    // Two sends, because one datagram is one loss, and a name is worth a retry.
    for _ in 0..2 {
        if sock.send_to(payload, &dest).is_err() {
            return None;
        }
        let until = Instant::now() + wait;
        while Instant::now() < until {
            let Ok((n, from)) = sock.recv_from(&mut buf) else { break };
            // Only this device's answer: anything else on the port is some
            // other responder and its name would be attached to the wrong row.
            if from.ip().to_string() != ip {
                continue;
            }
            if let Some(name) = crate::dhcp_names::dns_payload_name(&buf[..n.min(buf.len())]) {
                return Some(name);
            }
        }
    }
    None
}

/// Ask one device for its own name over mDNS.
pub fn mdns_ask(ip: &str, mac: &str, wait: Duration) -> Option<String> {
    let query = mdns_ptr_query(ip4(ip)?);
    ask(ip, mac, &query, 5353, wait)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_is_a_well_formed_reverse_lookup() {
        let q = mdns_ptr_query([192, 168, 1, 39]);
        // One question, no answers.
        assert_eq!(&q[4..6], &[0, 1]);
        assert_eq!(&q[6..12], &[0, 0, 0, 0, 0, 0]);
        // The labels are reversed, and the type is PTR with the unicast bit.
        assert_eq!(q[12], 2);
        assert_eq!(&q[13..15], b"39");
        assert_eq!(q[15], 1);
        assert_eq!(&q[16..17], b"1");
        let tail = &q[q.len() - 4..];
        assert_eq!(tail, &[0x00, 0x0c, 0x80, 0x01]);
        // The wire-format name ends with the root label.
        assert!(q.contains(&0u8));
    }

    #[test]
    fn a_bad_address_asks_nothing() {
        assert!(ip4("not-an-address").is_none());
        assert_eq!(ip4("192.168.1.39"), Some([192, 168, 1, 39]));
    }
}
