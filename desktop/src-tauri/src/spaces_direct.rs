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
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::wintun::WintunIo;

const STUN: &[&str] = &[
    "stun.l.google.com:19302",
    "stun.cloudflare.com:3478",
    "stun.nextcloud.com:3478",
];

pub struct DirectEngine {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<(), String>>>,
    peer: String,
    address: String,
    adapter: String,
    error: Arc<Mutex<Option<String>>>,
}

pub struct DirectState(pub Mutex<Option<DirectEngine>>);

#[derive(Serialize)]
pub struct DirectStatus {
    running: bool,
    peer: String,
    address: String,
    adapter: String,
    error: String,
}

fn stopped() -> DirectStatus {
    DirectStatus {
        running: false,
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

/// Create the direct session and its adapter, then start the packet loop.
#[tauri::command]
pub async fn space_direct_start(
    state: tauri::State<'_, crate::State>,
    direct: tauri::State<'_, DirectState>,
    id: String,
    peer_id: String,
) -> Result<DirectStatus, String> {
    // One direct LAN per app. Stop the old one before replacing it.
    let previous = direct
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(mut engine) = previous {
        engine.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = engine.handle.take() {
            let _ = tauri::async_runtime::spawn_blocking(move || handle.join()).await;
        }
    }

    let (token, host, path, private_key, self_id, address, space_id) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
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
        (
            record.token.clone(),
            host,
            format!("/v1/spaces/{}/probe", record.id),
            record.private_key.clone(),
            record.member_id.clone(),
            record.address.clone(),
            record.id.clone(),
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
    let udp_port = created
        .get("udp_port")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("probe socket: {e}"))?;
    let (socket, stun) = tauri::async_runtime::spawn_blocking(move || {
        let result = quotacards::spaces_stun::discover(
            &socket,
            STUN,
            Duration::from_millis(1500),
        );
        (socket, result)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Ok(endpoint) = stun {
        let endpoint_path = format!("{path}/{probe_id}/endpoint");
        crate::spaces::call(
            reqwest::Method::POST,
            &endpoint_path,
            Some(&token),
            Some(serde_json::json!({"endpoint": endpoint.to_string()})),
        )
        .await?;
    } else if udp_port != 0 && udp_port <= 65535 {
        socket
            .send_to(
                format!("QVP1 {probe_id}").as_bytes(),
                format!("{host}:{udp_port}"),
            )
            .map_err(|e| format!("probe rendezvous: {e}"))?;
    } else {
        return Err("could not discover the public UDP endpoint; STUN did not answer".into());
    }

    let status_path = format!("{path}/{probe_id}");
    let mut peer_endpoint = String::new();
    let mut peer_public = String::new();
    let mut peer_name = String::new();
    for _ in 0..30 {
        let status = crate::spaces::call(
            reqwest::Method::GET,
            &status_path,
            Some(&token),
            None,
        )
        .await?;
        if let Some(peers) = status.get("peers").and_then(Value::as_array) {
            if let Some(peer) = peers.iter().find(|p| {
                p.get("member_id").and_then(Value::as_str) == Some(peer_id.as_str())
            }) {
                if let Some(endpoint) = peer.get("endpoint").and_then(Value::as_str) {
                    peer_endpoint = endpoint.to_string();
                }
                if let Some(public) = peer.get("public_key").and_then(Value::as_str) {
                    peer_public = public.to_string();
                }
                if let Some(name) = peer.get("name").and_then(Value::as_str) {
                    peer_name = name.to_string();
                }
            }
        }
        if !peer_endpoint.is_empty() && !peer_public.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    if peer_endpoint.is_empty() || peer_public.is_empty() {
        return Err(
            "the other PC did not answer the rendezvous; start the direct LAN on both PCs at the same time"
                .into(),
        );
    }
    let peer: SocketAddr = peer_endpoint
        .parse()
        .map_err(|_| "the peer endpoint is not an address".to_string())?;
    let initiator = self_id < peer_id;
    let address_for_status = address.clone();
    let (path, io) = tauri::async_runtime::spawn_blocking(move || {
        let local = PathIdentity::from_private(&private_key)?;
        let public = spaces_path::decode_public(&peer_public)?;
        let path = if initiator {
            spaces_path::connect(socket, peer, &local, &public, Duration::from_secs(6))?
        } else {
            spaces_path::accept(socket, &local, &public, peer, Duration::from_secs(6))?
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
        peer: if peer_name.is_empty() {
            peer_id
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
        if let Some(handle) = engine.handle.take() {
            let _ = handle.join();
        }
    }
}

#[tauri::command]
pub async fn space_direct_stop(
    direct: tauri::State<'_, DirectState>,
) -> Result<DirectStatus, String> {
    let engine = direct
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let Some(mut engine) = engine else {
        return Ok(stopped());
    };
    engine.stop.store(true, Ordering::Relaxed);
    let error = engine
        .error
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    if let Some(handle) = engine.handle.take() {
        let _ = tauri::async_runtime::spawn_blocking(move || handle.join()).await;
    }
    Ok(DirectStatus {
        running: false,
        peer: engine.peer,
        address: engine.address,
        adapter: engine.adapter,
        error,
    })
}

#[tauri::command]
pub fn space_direct_status(direct: tauri::State<'_, DirectState>) -> DirectStatus {
    let slot = direct.0.lock().unwrap_or_else(|e| e.into_inner());
    match slot.as_ref() {
        Some(engine) => status_of(engine),
        None => stopped(),
    }
}
