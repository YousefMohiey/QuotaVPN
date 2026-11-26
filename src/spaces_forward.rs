//! The Spaces packet loop.
//!
//! One thread owns the direct session and one packet interface. On Windows the
//! interface is a Wintun adapter; in tests it is an in-memory queue. The loop
//! deliberately stays single-threaded: the Noise session has one send counter
//! and one receive window, so two threads writing the same datagram state would
//! need a lock on every packet. Polling both sides from one loop keeps the
//! per-packet cost at a UDP send and a UDP receive.

use crate::spaces_path::DirectPath;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// A packet interface above the direct path. `try_read` returns `None` when no
/// packet is waiting; a real adapter must not block here.
pub trait PacketIo {
    fn try_read(&mut self) -> Result<Option<Vec<u8>>, String>;
    fn write(&mut self, packet: &[u8]) -> Result<(), String>;
}

/// Forward IPv4 (or any opaque) packets until `stop` is set.
///
/// The direct path is set nonblocking. A full UDP send buffer drops the packet,
/// which is the same contract as a normal network adapter. Empty packets are
/// keepalives and are never handed to the interface.
pub fn run(
    mut path: DirectPath,
    io: &mut dyn PacketIo,
    stop: &AtomicBool,
) -> Result<(), String> {
    path.set_nonblocking()?;
    let mut last_keepalive = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let mut did = false;
        // Bound each pass so one busy direction cannot starve the other.
        for _ in 0..32 {
            match io.try_read()? {
                Some(packet) => {
                    if !packet.is_empty() {
                        let _ = path.try_send(&packet)?;
                    }
                    did = true;
                }
                None => break,
            }
        }
        for _ in 0..32 {
            match path.try_recv()? {
                Some(packet) => {
                    if !packet.is_empty() {
                        io.write(&packet)?;
                    }
                    did = true;
                }
                None => break,
            }
        }
        if last_keepalive.elapsed() >= Duration::from_secs(15) {
            let _ = path.try_send(&[])?;
            last_keepalive = Instant::now();
        }
        if !did {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spaces_path::{accept, connect, PathIdentity};
    use std::collections::VecDeque;
    use std::net::UdpSocket;
    use std::sync::{Arc, Mutex};

    struct MemoryIo {
        input: Arc<Mutex<VecDeque<Vec<u8>>>>,
        output: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl PacketIo for MemoryIo {
        fn try_read(&mut self) -> Result<Option<Vec<u8>>, String> {
            Ok(self.input.lock().unwrap().pop_front())
        }

        fn write(&mut self, packet: &[u8]) -> Result<(), String> {
            self.output.lock().unwrap().push(packet.to_vec());
            Ok(())
        }
    }

    fn identity() -> (PathIdentity, Vec<u8>) {
        let (private, _) = crate::spaces::wireguard_keypair();
        let id = PathIdentity::from_private(&private).expect("identity");
        let public = id.public().to_vec();
        (id, public)
    }

    fn wait_for(output: &Arc<Mutex<Vec<Vec<u8>>>>, want: &[u8]) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if output.lock().unwrap().iter().any(|packet| packet == want) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn packets_cross_the_direct_path_in_both_directions() {
        let (a_id, a_pub) = identity();
        let (b_id, b_pub) = identity();
        let a_socket = UdpSocket::bind("127.0.0.1:0").expect("a");
        let b_socket = UdpSocket::bind("127.0.0.1:0").expect("b");
        let a_addr = a_socket.local_addr().expect("a addr");
        let b_addr = b_socket.local_addr().expect("b addr");
        let responder = std::thread::spawn(move || {
            accept(b_socket, &b_id, &a_pub, a_addr, Duration::from_secs(3)).expect("accept")
        });
        let a_path = connect(a_socket, b_addr, &a_id, &b_pub, Duration::from_secs(3))
            .expect("connect");
        let b_path = responder.join().expect("responder");

        let a_in = Arc::new(Mutex::new(VecDeque::new()));
        let b_in = Arc::new(Mutex::new(VecDeque::new()));
        let a_out = Arc::new(Mutex::new(Vec::new()));
        let b_out = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));

        let mut a_io = MemoryIo {
            input: a_in.clone(),
            output: a_out.clone(),
        };
        let mut b_io = MemoryIo {
            input: b_in.clone(),
            output: b_out.clone(),
        };
        let a_stop = stop.clone();
        let b_stop = stop.clone();
        let a_thread = std::thread::spawn(move || run(a_path, &mut a_io, &a_stop));
        let b_thread = std::thread::spawn(move || run(b_path, &mut b_io, &b_stop));

        a_in.lock().unwrap().push_back(b"from-a".to_vec());
        assert!(wait_for(&b_out, b"from-a"), "b did not receive a's packet");
        b_in.lock().unwrap().push_back(b"from-b".to_vec());
        assert!(wait_for(&a_out, b"from-b"), "a did not receive b's packet");

        stop.store(true, Ordering::Relaxed);
        a_thread.join().expect("a thread").expect("a run");
        b_thread.join().expect("b thread").expect("b run");
    }
}
