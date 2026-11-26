//! The persistent Spaces direct LAN.
//!
//! This is the data path behind the Spaces screen: establish the same Noise
//! session the probe uses, create a second Wintun adapter, and forward IPv4
//! packets over the direct path. One engine carries one peer for now; a
//! multi-peer session is a later step.
//!
//! Windows only. The Wintun adapter is created here, so the process must be
//! elevated, which the QuotaVPN manifest already requests.

use quotacards::spaces_forward;
use quotacards::spaces_path::{self, PathIdentity};
use serde::Serialize;
use serde_json::Value;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::wintun::WintunIo;

const STUN: &[&str] = &[
    "stun.l.google.com:19302",
    "stun.cloudflare.com:3478",
    "stun.nextcloud.com:3478",
];

/// A manual Stop keeps the supervisor from reconnecting until the user asks
/// again or activates another space. Radmin's network stays on; this is the
/// equivalent of the power button.
static PAUSED: AtomicBool = AtomicBool::new(false);
/// One start at a time. The supervisor and the screen can both ask.
static STARTING: AtomicBool = AtomicBool::new(false);

pub fn paused() -> bool {
    PAUSED.load(Ordering::Relaxed)
}

pub fn set_paused(value: bool) {
    PAUSED.store(value, Ordering::Relaxed);
}

pub fn is_starting() -> bool {
    STARTING.load(Ordering::Relaxed)
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
static FIREWALL: Once = Once::new();

/// Radmin's service adds its own firewall exceptions. The direct engine uses an
/// ephemeral UDP socket, so an inbound rule for this executable is what lets the
/// other PC's handshake reach it on Private and Domain networks as well as the
/// Public one the installer already covered.
fn ensure_firewall() {
    FIREWALL.call_once(|| {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let exe = exe.to_string_lossy().replace('\'', "''");
        for (name, proto) in [
            ("QuotaVPN Spaces UDP", "UDP"),
            ("QuotaVPN Spaces TCP", "TCP"),
        ] {
            let script = format!(
                "$n='{name}'; if (-not (Get-NetFirewallRule -DisplayName $n -ErrorAction SilentlyContinue)) {{ New-NetFirewallRule -DisplayName $n -Direction Inbound -Action Allow -Program '{exe}' -Protocol {proto} -Profile Any | Out-Null }}"
            );
            let _ = Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    });
}

pub struct DirectEngine {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<(), String>>>,
    helper_stop: Arc<AtomicBool>,
    helper: Option<JoinHandle<()>>,
    space: String,
    peer: String,
    address: String,
    adapter: String,
    error: Arc<Mutex<Option<String>>>,
}

pub struct DirectState(pub Mutex<Option<DirectEngine>>);

#[derive(Serialize)]
pub struct DirectStatus {
    pub running: bool,
    pub starting: bool,
    pub space: String,
    pub peer: String,
    pub address: String,
    pub adapter: String,
    pub error: String,
}

fn stopped() -> DirectStatus {
    DirectStatus {
        running: false,
        starting: STARTING.load(Ordering::Relaxed),
        space: String::new(),
        peer: String::new(),
        address: String::new(),
        adapter: String::new(),
        error: String::new(),
    }
}

fn status_of(engine: &DirectEngine) -> DirectStatus {
    DirectStatus {
        running: engine
            .handle
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false),
        starting: STARTING.load(Ordering::Relaxed),
        space: engine.space.clone(),
        peer: engine.peer.clone(),
        address: engine.address.clone(),
        adapter: engine.adapter.clone(),
        error: engine
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_default(),
    }
}

/// The space the running engine belongs to, if any.
pub fn running_space(direct: &DirectState) -> Option<String> {
    let slot = direct.0.lock().unwrap_or_else(|e| e.into_inner());
    slot.as_ref().map(|engine| engine.space.clone())
}

pub fn running_for(direct: &DirectState, space: &str) -> bool {
    let slot = direct.0.lock().unwrap_or_else(|e| e.into_inner());
    slot.as_ref()
        .map(|engine| {
            engine.space == space
                && engine
                    .handle
                    .as_ref()
                    .map(|handle| !handle.is_finished())
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Stop the engine and wait for its thread. The forward loop wakes at least
/// every 2 ms, so the join is short. Used by the supervisor, the Stop button
/// and the exit path.
pub fn stop_engine(direct: &DirectState) -> DirectStatus {
    let engine = direct
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let Some(mut engine) = engine else {
        return stopped();
    };
    engine.stop.store(true, Ordering::Relaxed);
    engine.helper_stop.store(true, Ordering::Relaxed);
    let error = engine
        .error
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    if let Some(helper) = engine.helper.take() {
        let _ = helper.join();
    }
    if let Some(handle) = engine.handle.take() {
        let _ = handle.join();
    }
    DirectStatus {
        running: false,
        starting: false,
        space: engine.space,
        peer: engine.peer,
        address: engine.address,
        adapter: engine.adapter,
        error,
    }
}

/// A stable TCP listener plus a helper connection from the same local port to
/// the coordinator. The helper keeps the NAT mapping open, which is how Radmin
/// accepts peers on its control port. Without it an inbound SYN has nowhere to
/// go on a home router.
struct TcpGateway {
    listener: TcpListener,
    port: u16,
    stop: Arc<AtomicBool>,
    helper: JoinHandle<()>,
}

fn tcp_gateway(host: &str, tls: bool) -> Result<TcpGateway, String> {
    let remote_port = if tls { 443 } else { 80 };
    let remote = format!("{host}:{remote_port}");
    let remote: SocketAddr = remote
        .to_socket_addrs()
        .map_err(|e| format!("coordinator address: {e}"))?
        .next()
        .ok_or("the coordinator address did not resolve")?;
    let mut last = String::new();
    for port in 51394u16..51420 {
        match gateway_on(port, remote) {
            Ok(gateway) => return Ok(gateway),
            Err(e) => last = e,
        }
    }
    Err(format!("could not open a TCP listener: {last}"))
}

fn gateway_on(port: u16, remote: SocketAddr) -> Result<TcpGateway, String> {
    let local = SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, port));
    let listener = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| format!("tcp listener: {e}"))?;
    listener
        .set_reuse_address(true)
        .map_err(|e| format!("tcp listener: {e}"))?;
    listener
        .bind(&local.into())
        .map_err(|e| format!("tcp listener on {port}: {e}"))?;
    listener
        .listen(8)
        .map_err(|e| format!("tcp listener: {e}"))?;
    let listener: TcpListener = listener.into();
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("tcp listener: {e}"))?;

    let helper = connect_helper(local, remote)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for = stop.clone();
    let helper = std::thread::spawn(move || {
        let mut current = Some(helper);
        while !stop_for.load(Ordering::Relaxed) {
            if current.is_none() {
                current = connect_helper(local, remote).ok();
            }
            if let Some(stream) = current.as_ref() {
                let mut probe = [0u8; 1];
                stream.set_nonblocking(true).ok();
                match stream.peek(&mut probe) {
                    Ok(0) => current = None,
                    Ok(_) => {}
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => current = None,
                }
            }
            std::thread::sleep(Duration::from_millis(800));
        }
    });
    Ok(TcpGateway {
        listener,
        port,
        stop,
        helper,
    })
}

fn connect_helper(local: SocketAddr, remote: SocketAddr) -> Result<TcpStream, String> {
    let sock = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| format!("tcp helper: {e}"))?;
    sock.set_reuse_address(true)
        .map_err(|e| format!("tcp helper: {e}"))?;
    sock.bind(&local.into())
        .map_err(|e| format!("tcp helper on {}: {e}", local.port()))?;
    sock.connect(&remote.into())
        .map_err(|e| format!("tcp helper connect: {e}"))?;
    sock.set_nonblocking(true)
        .map_err(|e| format!("tcp helper: {e}"))?;
    Ok(sock.into())
}

/// Create the direct session and its adapter, then start the packet loop.
#[tauri::command]
pub async fn space_direct_start(
    state: tauri::State<'_, crate::State>,
    direct: tauri::State<'_, DirectState>,
    id: String,
    peer_id: String,
) -> Result<DirectStatus, String> {
    set_paused(false);
    start(&state, &direct, &id, &peer_id).await
}

/// The command and the supervisor share this. `state` and `direct` are the
/// managed app state, so the caller does not need a Tauri command context.
pub async fn start(
    state: &crate::State,
    direct: &DirectState,
    id: &str,
    peer_id: &str,
) -> Result<DirectStatus, String> {
    if STARTING.swap(true, Ordering::SeqCst) {
        return Err("a direct connection is already starting".into());
    }
    struct Starting;
    impl Drop for Starting {
        fn drop(&mut self) {
            STARTING.store(false, Ordering::SeqCst);
        }
    }
    let _starting = Starting;
    ensure_firewall();
    // One direct LAN per app. Stop the old one before replacing it.
    let previous = direct
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(mut engine) = previous {
        engine.stop.store(true, Ordering::Relaxed);
        engine.helper_stop.store(true, Ordering::Relaxed);
        if let Some(helper) = engine.helper.take() {
            let _ = helper.join();
        }
        if let Some(handle) = engine.handle.take() {
            let _ = tauri::async_runtime::spawn_blocking(move || handle.join()).await;
        }
    }

    let (token, host, tls, path, private_key, self_id, address, space_id, owner) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id.as_str() == id)
            .ok_or("that space is not on this PC")?;
        if record.token.is_empty() {
            return Err("this space has no access token; join it again".into());
        }
        if record.private_key.is_empty() {
            return Err("this space has no private key; join it again".into());
        }
        let base = crate::spaces::api_base();
        let url = reqwest::Url::parse(&base)
            .map_err(|e| format!("the space server address is invalid: {e}"))?;
        let host = url
            .host_str()
            .ok_or("the space server address has no host")?
            .to_string();
        let tls = url.scheme() == "https";
        (
            record.token.clone(),
            host,
            tls,
            format!("/v1/spaces/{}/probe", record.id),
            record.private_key.clone(),
            record.member_id.clone(),
            record.address.clone(),
            record.id.clone(),
            record.owner,
        )
    };
    if address.is_empty() {
        return Err("this space has no local address".into());
    }

    let created = crate::spaces::call(
        reqwest::Method::POST,
        &path,
        Some(&token),
        Some(serde_json::json!({})),
    )
    .await?;
    let probe_id = created
        .get("probe_id")
        .and_then(Value::as_str)
        .ok_or("the space server did not issue a probe")?
        .to_string();
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("probe socket: {e}"))?;
    let (_socket, stun) = tauri::async_runtime::spawn_blocking(move || {
        let result = quotacards::spaces_stun::discover(
            &socket,
            STUN,
            Duration::from_millis(1500),
        );
        (socket, result)
    })
    .await
    .map_err(|e| e.to_string())?;
    let endpoint =
        stun.map_err(|e| format!("could not discover the public UDP endpoint: {e}"))?;
    let gateway = tcp_gateway(&host, tls)?;
    let tcp_endpoint = format!("{}:{}", endpoint.ip(), gateway.port);
    let endpoint_path = format!("{path}/{probe_id}/endpoint");
    crate::spaces::call(
        reqwest::Method::POST,
        &endpoint_path,
        Some(&token),
        Some(serde_json::json!({
            "endpoint": endpoint.to_string(),
            "tcp_endpoint": tcp_endpoint,
        })),
    )
    .await?;

    let status_path = format!("{path}/{probe_id}");
    let mut peer_public = String::new();
    let mut peer_tcp = String::new();
    let mut peer_name = String::new();
    // Both sides create a probe and wait for the other. The supervisor can be
    // a few seconds behind the other app, so give the rendezvous 18 seconds
    // instead of 9; a manual Connect on both PCs still pairs immediately.
    for _ in 0..60 {
        let status = crate::spaces::call(
            reqwest::Method::GET,
            &status_path,
            Some(&token),
            None,
        )
        .await?;
        if let Some(peers) = status.get("peers").and_then(Value::as_array) {
            if let Some(peer) = peers.iter().find(|p| {
                p.get("member_id").and_then(Value::as_str) == Some(peer_id)
            }) {
                if let Some(public) = peer.get("public_key").and_then(Value::as_str) {
                    peer_public = public.to_string();
                }
                if let Some(tcp) = peer.get("tcp_endpoint").and_then(Value::as_str) {
                    peer_tcp = tcp.to_string();
                }
                if let Some(name) = peer.get("name").and_then(Value::as_str) {
                    peer_name = name.to_string();
                }
            }
        }
        if !peer_public.is_empty() && !peer_tcp.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    if peer_public.is_empty() {
        return Err(
            "the other PC did not answer the rendezvous; start the direct LAN on both PCs at the same time"
                .into(),
        );
    }
    if peer_tcp.is_empty() {
        return Err("the other PC did not publish a TCP endpoint; update it and retry".into());
    }
    let peer_tcp: SocketAddr = peer_tcp
        .parse()
        .map_err(|_| "the peer TCP endpoint is not an address".to_string())?;
    let initiator = self_id.as_str() < peer_id;
    let address_for_status = address.clone();
    let engine_space = space_id.clone();
    let TcpGateway {
        listener,
        port: _,
        stop: helper_stop,
        helper,
    } = gateway;
    let (path, io) = tauri::async_runtime::spawn_blocking(move || {
        let local = PathIdentity::from_private(&private_key)?;
        let public = spaces_path::decode_public(&peer_public)?;
        let connect_timeout = Duration::from_secs(10);
        let stream = if owner {
            let deadline = Instant::now() + connect_timeout;
            loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Err("the peer did not connect over TCP".into());
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => return Err(format!("tcp accept: {e}")),
                }
            }
        } else {
            TcpStream::connect_timeout(&peer_tcp, connect_timeout)
                .map_err(|e| format!("tcp connect: {e}"))?
        };
        stream
            .set_nonblocking(false)
            .map_err(|e| format!("tcp stream: {e}"))?;
        let path = if initiator {
            spaces_path::connect_tcp(stream, &local, &public, Duration::from_secs(8))?
        } else {
            spaces_path::accept_tcp(stream, &local, &public, Duration::from_secs(8))?
        };
        let io = WintunIo::new(&space_id, &address)?;
        Ok::<_, String>((path, io))
    })
    .await
    .map_err(|e| e.to_string())??;

    let adapter = io.name().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None));
    let stop_for_thread = stop.clone();
    let error_for_thread = error.clone();
    let handle = std::thread::spawn(move || {
        let mut io = io;
        let result = spaces_forward::run(path, &mut io, &stop_for_thread);
        if let Err(e) = &result {
            *error_for_thread
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(e.clone());
            quotacards::vpn::app_log(&format!("spaces direct: {e}"));
        }
        result
    });
    let engine = DirectEngine {
        stop,
        handle: Some(handle),
        helper_stop,
        helper: Some(helper),
        space: engine_space,
        peer: if peer_name.is_empty() {
            peer_id.to_string()
        } else {
            peer_name
        },
        address: address_for_status,
        adapter: adapter.clone(),
        error,
    };
    let status = status_of(&engine);
    *direct.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(engine);
    Ok(status)
}

/// Stop from the exit path, where there is no async runtime to await a join.
/// The forwarding thread wakes at least every 2 ms and owns the Wintun handle,
/// so dropping it removes the adapter before the process exits.
pub fn stop_now(direct: &DirectState) {
    let engine = direct
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(mut engine) = engine {
        engine.stop.store(true, Ordering::Relaxed);
        engine.helper_stop.store(true, Ordering::Relaxed);
        if let Some(helper) = engine.helper.take() {
            let _ = helper.join();
        }
        if let Some(handle) = engine.handle.take() {
            let _ = handle.join();
        }
    }
}

#[tauri::command]
pub async fn space_direct_stop(
    direct: tauri::State<'_, DirectState>,
) -> Result<DirectStatus, String> {
    set_paused(true);
    Ok(stop_engine(&direct))
}

#[tauri::command]
pub fn space_direct_status(direct: tauri::State<'_, DirectState>) -> DirectStatus {
    let slot = direct.0.lock().unwrap_or_else(|e| e.into_inner());
    match slot.as_ref() {
        Some(engine) => status_of(engine),
        None => stopped(),
    }
}
