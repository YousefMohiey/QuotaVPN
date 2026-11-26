//! Spaces: coordinator client and the desktop commands behind the screen.
//!
//! Secrets (the WireGuard private key and the member token) live in
//! config.json beside the card profiles and are never put in the view sent to
//! the webview. Moving them to a DPAPI-sealed store is a phase-2 item; the
//! card's own WireGuard key already has the same lifetime and location.

use base64::Engine as _;
use quotacards::{
    config::AppConfig,
    spaces::{self, SpaceMember, SpaceRecord},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_API: &str = "https://quotavpn.duckdns.org/spaces";

fn api_slot() -> &'static Mutex<String> {
    static API: OnceLock<Mutex<String>> = OnceLock::new();
    API.get_or_init(|| Mutex::new(String::new()))
}

/// Resolve the coordinator once at startup: the stored address first, then the
/// built-in public default. A stale QC_SPACES_API from an old LAN test must not
/// override the stored value, so the environment is not read here.
pub(crate) fn init_api(cfg: &AppConfig) {
    let mut value = cfg.spaces_api.trim().trim_end_matches('/').to_string();
    if value.is_empty() {
        value = DEFAULT_API.to_string();
    }
    *api_slot().lock().unwrap_or_else(|e| e.into_inner()) = value;
}

pub(crate) fn api_base() -> String {
    let value = api_slot().lock().unwrap_or_else(|e| e.into_inner()).clone();
    if !value.is_empty() {
        return value;
    }
    DEFAULT_API.to_string()
}

fn normalize_api(url: &str) -> Result<String, String> {
    let clean = url.trim().trim_end_matches('/').to_string();
    if clean.is_empty() {
        return Err("enter the coordinator address".into());
    }
    if !(clean.starts_with("http://") || clean.starts_with("https://")) {
        return Err("the coordinator address must start with http:// or https://".into());
    }
    if clean.chars().any(|c| c.is_whitespace()) {
        return Err("the coordinator address cannot contain spaces".into());
    }
    Ok(clean)
}

/// Store a coordinator address and make it live for every Spaces call. The
/// join path uses this when an invite carries the host's address, so the
/// joining PC never needs an environment variable.
pub(crate) fn remember_api(state: &crate::State, url: &str) -> Result<String, String> {
    let clean = normalize_api(url)?;
    {
        let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        cfg.spaces_api = clean.clone();
        cfg.save();
    }
    *api_slot().lock().unwrap_or_else(|e| e.into_inner()) = clean.clone();
    Ok(clean)
}

/// The invite shown to the owner. The coordinator address rides inside it, so
/// the other PC can join with the code alone.
fn wrap_invite(raw: &str) -> String {
    if raw.is_empty() || raw.starts_with("qvs2:") {
        return raw.to_string();
    }
    let api = api_base();
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(api.as_bytes());
    format!("qvs2:{encoded}:{raw}")
}

/// Split a wrapped invite into the coordinator address and the server code.
/// A plain `qvs1` code is still accepted.
fn split_invite(code: &str) -> (String, String) {
    let trimmed = code.trim();
    if let Some(rest) = trimmed.strip_prefix("qvs2:") {
        if let Some((encoded, inner)) = rest.split_once(':') {
            if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded) {
                if let Ok(api) = String::from_utf8(bytes) {
                    return (api.trim().to_string(), inner.to_string());
                }
            }
        }
    }
    (String::new(), trimmed.to_string())
}

#[tauri::command]
pub fn spaces_api() -> String {
    api_base()
}

#[tauri::command]
pub fn set_spaces_api(
    state: tauri::State<'_, crate::State>,
    url: String,
) -> Result<String, String> {
    remember_api(&state, &url)
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("space client: {e}"))
}

pub(crate) async fn call(
    method: reqwest::Method,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Result<Value, String> {
    let base = api_base();
    let url = format!("{}{}", base, path);
    let client = http()?;
    let mut req = client.request(method, &url);
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    if let Some(body) = body {
        req = req.header("content-type", "application/json").json(&body);
    }
    let response = req
        .send()
        .await
        .map_err(|e| format!("could not reach the space server at {base}: {e}"))?;
    let code = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !(200..300).contains(&code) {
        let msg = value
            .get("error")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("the space server refused the request")
            .to_string();
        return Err(msg);
    }
    Ok(value)
}

#[derive(Deserialize)]
struct ApiSpace {
    id: String,
    name: String,
    subnet: String,
    #[serde(default)]
    mode: String,
}

#[derive(Deserialize)]
struct ApiMember {
    id: String,
    name: String,
    address: String,
    #[serde(default)]
    token: String,
}

#[derive(Deserialize)]
struct ApiHub {
    public_key: String,
    endpoint: String,
    port: u16,
    #[serde(default)]
    subnet: String,
}

#[derive(Deserialize)]
struct ApiMemberView {
    id: String,
    name: String,
    address: String,
    #[serde(default)]
    public_key: String,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    self_pc: bool,
    #[serde(default)]
    owner: bool,
    #[serde(default)]
    mac: String,
    #[serde(default)]
    last_seen: i64,
}

#[derive(Deserialize)]
struct ApiList {
    space: ApiSpace,
    members: Vec<ApiMemberView>,
    hub: ApiHub,
}

#[derive(Deserialize)]
struct ApiJoin {
    space: ApiSpace,
    member: ApiMember,
    hub: ApiHub,
    #[serde(default)]
    invite: String,
}

#[derive(Serialize)]
pub struct SpaceMemberView {
    id: String,
    name: String,
    address: String,
    online: bool,
    self_pc: bool,
    owner: bool,
    last_seen: i64,
}

#[derive(Serialize)]
pub struct SpaceView {
    id: String,
    name: String,
    address: String,
    subnet: String,
    owner: bool,
    invite: String,
    mode: String,
    members: Vec<SpaceMemberView>,
    online: usize,
}

#[derive(Serialize)]
pub struct SpacesState {
    spaces: Vec<SpaceView>,
    active: String,
    /// The space the live tunnel carries, empty when none.
    tunnel_space: String,
    tunnel_on: bool,
}

/// Result of the direct-path probe. This is a diagnostic, not the data plane:
/// it proves whether two installations can reach each other without the hub.
#[derive(Serialize)]
pub struct ProbeResult {
    direct: bool,
    rtt_ms: Option<f64>,
    self_endpoint: String,
    peer_endpoint: String,
    note: String,
}

fn clean_address(address: &str) -> String {
    address.split('/').next().unwrap_or(address).to_string()
}

fn view(record: &SpaceRecord) -> SpaceView {
    let members: Vec<SpaceMemberView> = record
        .members
        .iter()
        .map(|m| SpaceMemberView {
            id: m.id.clone(),
            name: m.name.clone(),
            address: clean_address(&m.address),
            online: m.online,
            self_pc: m.self_pc,
            owner: m.owner,
            last_seen: m.last_seen,
        })
        .collect();
    SpaceView {
        id: record.id.clone(),
        name: record.name.clone(),
        address: clean_address(&record.address),
        subnet: record.subnet.clone(),
        owner: record.owner,
        invite: if record.owner {
            wrap_invite(&record.invite)
        } else {
            String::new()
        },
        mode: if record.mode.is_empty() {
            "l3".to_string()
        } else {
            record.mode.clone()
        },
        online: members.iter().filter(|m| m.online).count(),
        members,
    }
}

fn snapshot(cfg: &AppConfig) -> SpacesState {
    SpacesState {
        spaces: cfg.spaces.iter().map(view).collect(),
        active: cfg.active_space.clone(),
        tunnel_space: cfg.tunnel_space.clone(),
        tunnel_on: quotacards::vpn::task_engine_pid().is_some(),
    }
}

fn record_from(
    space: ApiSpace,
    member: ApiMember,
    hub: ApiHub,
    private_key: String,
    public_key: String,
    owner: bool,
    invite: String,
) -> SpaceRecord {
    let address = if member.address.ends_with("/32") {
        member.address.clone()
    } else {
        format!("{}/32", member.address)
    };
    let subnet = if hub.subnet.is_empty() {
        space.subnet.clone()
    } else {
        hub.subnet.clone()
    };
    // The roster can render this PC before the first coordinator refresh.
    let self_member = SpaceMember {
        id: member.id.clone(),
        name: member.name.clone(),
        address: address.clone(),
        public_key: public_key.clone(),
        online: true,
        self_pc: true,
        owner,
        mac: String::new(),
        last_seen: spaces_now(),
    };
    let mode = space.mode.clone();
    SpaceRecord {
        id: space.id,
        name: space.name,
        member_id: member.id,
        address,
        subnet,
        private_key,
        public_key,
        token: member.token,
        hub_public_key: hub.public_key,
        hub_endpoint: hub.endpoint,
        hub_port: hub.port,
        owner,
        invite,
        members: vec![self_member],
        last_seen: spaces_now(),
        mode,
    }
}

fn upsert(cfg: &mut AppConfig, record: SpaceRecord) {
    if let Some(existing) = cfg.spaces.iter_mut().find(|s| s.id == record.id) {
        *existing = record;
    } else {
        cfg.spaces.push(record);
    }
}

/// The screen's whole state. Called on open, after every action, and on the
/// screen's slow poll.
#[tauri::command]
pub fn spaces_state(state: tauri::State<'_, crate::State>) -> SpacesState {
    let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    snapshot(&cfg)
}

/// The name this PC should offer in a space. The Windows computer name is a
/// sensible default and saves the user from typing a device name they already
/// have. The UI only calls this when nothing was remembered.
#[tauri::command]
pub fn space_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().count() <= 40)
        .unwrap_or_else(|| "Windows PC".to_string())
}

#[tauri::command]
pub async fn space_create(
    state: tauri::State<'_, crate::State>,
    name: String,
    password: String,
    device_name: String,
) -> Result<SpacesState, String> {
    let (private_key, public_key) = spaces::wireguard_keypair();
    let body = json!({
        "name": name.trim(),
        "password": password,
        "device_name": device_name.trim(),
        "public_key": public_key,
    });
    let answer = call(reqwest::Method::POST, "/v1/spaces", None, Some(body)).await?;
    let parsed: ApiJoin = serde_json::from_value(answer)
        .map_err(|e| format!("the space server sent an answer this build does not understand: {e}"))?;
    let record = record_from(
        parsed.space,
        parsed.member,
        parsed.hub,
        private_key,
        public_key,
        true,
        parsed.invite,
    );
    let id = record.id.clone();
    {
        let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        upsert(&mut cfg, record);
        cfg.active_space = id;
        cfg.save();
        Ok(snapshot(&cfg))
    }
}

#[tauri::command]
pub async fn space_join(
    state: tauri::State<'_, crate::State>,
    code: String,
    password: String,
    device_name: String,
) -> Result<SpacesState, String> {
    let (api, inner) = split_invite(&code);
    if !api.is_empty() {
        remember_api(&state, &api)?;
    }
    let invite = spaces::parse_invite(&inner)?;
    // Joining a space this PC already belongs to must reuse its key. A fresh
    // key would enrol a second member and leave the old hub peer behind.
    let (private_key, public_key, owner_before) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        match cfg.spaces.iter().find(|s| s.id == invite.space_id) {
            Some(existing) => match spaces::public_key_for(&existing.private_key) {
                Ok(public) => (existing.private_key.clone(), public, existing.owner),
                Err(_) => {
                    let (private, public) = spaces::wireguard_keypair();
                    (private, public, false)
                }
            },
            None => {
                let (private, public) = spaces::wireguard_keypair();
                (private, public, false)
            }
        }
    };
    let body = json!({
        "code": format!("qvs1:{}:{}", invite.space_id, invite.secret),
        "password": password,
        "device_name": device_name.trim(),
        "public_key": public_key,
    });
    let answer = call(reqwest::Method::POST, "/v1/spaces/join", None, Some(body)).await?;
    let parsed: ApiJoin = serde_json::from_value(answer)
        .map_err(|e| format!("the space server sent an answer this build does not understand: {e}"))?;
    let record = record_from(
        parsed.space,
        parsed.member,
        parsed.hub,
        private_key,
        public_key,
        owner_before,
        String::new(),
    );
    let id = record.id.clone();
    {
        let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        upsert(&mut cfg, record);
        cfg.active_space = id;
        cfg.save();
        Ok(snapshot(&cfg))
    }
}

/// Switch a space between IP routing (`l3`) and Ethernet (`l2`). The owner is
/// the only one allowed to change it, and every member picks it up on the next
/// refresh. A running engine keeps its mode until it is restarted.
#[tauri::command]
pub async fn space_set_mode(
    state: tauri::State<'_, crate::State>,
    id: String,
    mode: String,
) -> Result<SpacesState, String> {
    let mode = mode.trim().to_ascii_lowercase();
    if mode != "l2" && mode != "l3" {
        return Err("the space mode must be l2 or l3".into());
    }
    let (token, path) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or("that space is not on this PC")?;
        (
            record.token.clone(),
            format!("/v1/spaces/{}/mode", record.id),
        )
    };
    if token.is_empty() {
        return Err("this space has no access token; join it again".into());
    }
    call(
        reqwest::Method::POST,
        &path,
        Some(&token),
        Some(json!({ "mode": mode })),
    )
    .await?;
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(record) = cfg.spaces.iter_mut().find(|s| s.id == id) {
        record.mode = mode;
    }
    cfg.save();
    Ok(snapshot(&cfg))
}

#[tauri::command]
pub async fn space_refresh(    state: tauri::State<'_, crate::State>,
    id: String,
) -> Result<SpacesState, String> {
    refresh(&state, &id).await
}

/// Refresh a space from the coordinator and cache the roster. Shared by the
/// command and the background supervisor, which needs the online flags
/// without a screen being open.
pub(crate) async fn refresh(state: &crate::State, id: &str) -> Result<SpacesState, String> {
    let (token, path) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id.as_str() == id)
            .ok_or("that space is not on this PC")?;
        (record.token.clone(), format!("/v1/spaces/{}", record.id))
    };
    if token.is_empty() {
        return Err("this space has no access token; join it again".into());
    }
    let answer = call(reqwest::Method::GET, &path, Some(&token), None).await?;
    let parsed: ApiList = serde_json::from_value(answer)
        .map_err(|e| format!("the space server sent an answer this build does not understand: {e}"))?;
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(record) = cfg.spaces.iter_mut().find(|s| s.id.as_str() == id) {
        record.name = parsed.space.name;
        if !parsed.space.mode.is_empty() {
            record.mode = parsed.space.mode;
        }
        record.subnet = if parsed.space.subnet.is_empty() {
            record.subnet.clone()
        } else {
            parsed.space.subnet
        };
        record.hub_public_key = parsed.hub.public_key;
        record.hub_endpoint = parsed.hub.endpoint;
        record.hub_port = parsed.hub.port;
        record.members = parsed
            .members
            .into_iter()
            .map(|m| SpaceMember {
                id: m.id,
                name: m.name,
                address: m.address,
                public_key: m.public_key,
                online: m.online,
                self_pc: m.self_pc,
                owner: m.owner,
                mac: m.mac,
                last_seen: m.last_seen,
            })
            .collect();
        // Ownership can move when the old owner leaves. The screen's actions
        // follow this flag, not a cached "I made it" bit.
        if let Some(me) = record.members.iter().find(|m| m.self_pc) {
            record.owner = me.owner;
        }
        record.last_seen = spaces_now();
        cfg.save();
    }
    Ok(snapshot(&cfg))
}

fn spaces_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Establish the real Noise IK session with the peer and send one encrypted
/// echo. This is the same channel the data plane will use; the probe is not a
/// separate diagnostic protocol.
fn probe_secure(
    socket: UdpSocket,
    private_key: String,
    peer_public_b64: String,
    peer: String,
    initiator: bool,
) -> Result<(bool, Option<f64>, String), String> {
    let peer: SocketAddr = peer
        .parse()
        .map_err(|_| "the peer endpoint is not an address".to_string())?;
    let local = quotacards::spaces_path::PathIdentity::from_private(&private_key)?;
    let peer_public = quotacards::spaces_path::decode_public(&peer_public_b64)?;
    if initiator {
        let mut path = quotacards::spaces_path::connect(
            socket,
            peer,
            &local,
            &peer_public,
            Duration::from_secs(6),
        )?;
        let started = Instant::now();
        path.send(b"qv-probe")?;
        let reply = path.recv()?;
        if reply != b"qv-probe" {
            return Err("the peer returned an unexpected probe reply".into());
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        Ok((true, Some(ms), String::new()))
    } else {
        let mut path = quotacards::spaces_path::accept(
            socket,
            &local,
            &peer_public,
            peer,
            Duration::from_secs(6),
        )?;
        let got = path.recv()?;
        if got != b"qv-probe" {
            return Err("the peer sent an unexpected probe payload".into());
        }
        path.send(&got)?;
        Ok((true, None, String::new()))
    }
}

/// Test whether this PC and another member can reach each other directly.
///
/// Both PCs must run the test at roughly the same time: the coordinator issues
/// a short-lived probe id to each side, records the public UDP endpoint it sees,
/// and returns both endpoints. The two sockets then punch each other. A direct
/// result here is the green light for the encrypted data channel.
#[tauri::command]
pub async fn space_probe(
    state: tauri::State<'_, crate::State>,
    id: String,
    peer_id: String,
) -> Result<ProbeResult, String> {
    let (token, host, path, private_key, self_id) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or("that space is not on this PC")?;
        if record.token.is_empty() {
            return Err("this space has no access token; join it again".into());
        }
        let base = api_base();
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
        )
    };
    let created = call(
        reqwest::Method::POST,
        &path,
        Some(&token),
        Some(json!({})),
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
        .ok_or("the space server did not return a probe port")?;
    if udp_port == 0 || udp_port > 65535 {
        return Err("the space server's direct-path listener is not running".into());
    }
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("probe socket: {e}"))?;
    let (socket, stun) = tauri::async_runtime::spawn_blocking(move || {
        let result = quotacards::spaces_stun::discover(
            &socket,
            &[
                "stun.l.google.com:19302",
                "stun.cloudflare.com:3478",
                "stun.nextcloud.com:3478",
            ],
            Duration::from_millis(1500),
        );
        (socket, result)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Ok(endpoint) = stun {
        let endpoint_path = format!("{path}/{probe_id}/endpoint");
        call(
            reqwest::Method::POST,
            &endpoint_path,
            Some(&token),
            Some(json!({"endpoint": endpoint.to_string()})),
        )
        .await?;
    } else if udp_port != 0 {
        // Fallback for a deployment that still runs the UDP observer.
        socket
            .send_to(
                format!("QVP1 {probe_id}").as_bytes(),
                format!("{host}:{udp_port}"),
            )
            .map_err(|e| format!("probe rendezvous: {e}"))?;
    } else {
        return Err(
            "could not discover the public UDP endpoint; STUN did not answer".into(),
        );
    }

    let status_path = format!("{path}/{probe_id}");
    let mut self_endpoint = String::new();
    let mut peer_endpoint = String::new();
    let mut peer_public = String::new();
    for _ in 0..30 {
        let status = call(reqwest::Method::GET, &status_path, Some(&token), None).await?;
        if let Some(endpoint) = status.get("self").and_then(Value::as_str) {
            self_endpoint = endpoint.to_string();
        }
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
            }
        }
        if !self_endpoint.is_empty() && !peer_endpoint.is_empty() && !peer_public.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    if self_endpoint.is_empty() {
        return Err(
            "the coordinator did not see the probe datagram; check the UDP probe port".into(),
        );
    }
    if peer_endpoint.is_empty() {
        return Ok(ProbeResult {
            direct: false,
            rtt_ms: None,
            self_endpoint,
            peer_endpoint,
            note: "the other PC did not answer the rendezvous; run the probe on both PCs at the same time"
                .into(),
        });
    }
    if peer_public.is_empty() {
        return Ok(ProbeResult {
            direct: false,
            rtt_ms: None,
            self_endpoint,
            peer_endpoint,
            note: "this PC does not know the peer's key yet; refresh the space".into(),
        });
    }
    let peer_for_punch = peer_endpoint.clone();
    let initiator = self_id < peer_id;
    let (direct, rtt_ms, note) = tauri::async_runtime::spawn_blocking(move || {
        probe_secure(
            socket,
            private_key,
            peer_public,
            peer_for_punch,
            initiator,
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(ProbeResult {
        direct,
        rtt_ms,
        self_endpoint,
        peer_endpoint,
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secure_probe_measures_the_direct_path() {
        let (a_priv, a_pub) = quotacards::spaces::wireguard_keypair();
        let (b_priv, b_pub) = quotacards::spaces::wireguard_keypair();
        let a = UdpSocket::bind("127.0.0.1:0").expect("a");
        let b = UdpSocket::bind("127.0.0.1:0").expect("b");
        let a_addr = a.local_addr().expect("a addr");
        let b_addr = b.local_addr().expect("b addr");
        let responder = std::thread::spawn(move || {
            probe_secure(b, b_priv, a_pub, a_addr.to_string(), false).expect("responder")
        });
        let (direct, rtt, note) =
            probe_secure(a, a_priv, b_pub, b_addr.to_string(), true).expect("initiator");
        assert!(direct);
        assert!(rtt.is_some());
        assert!(note.is_empty());
        let (responder_direct, responder_rtt, _) = responder.join().expect("responder");
        assert!(responder_direct);
        assert!(responder_rtt.is_none());
    }
}

/// Turn a space on for the next tunnel, or off when `id` is empty. The engine
/// reads the active record when it builds the config; an already running
/// session keeps its config until the user reconnects.
#[tauri::command]
pub fn space_activate(
    state: tauri::State<'_, crate::State>,
    id: String,
) -> Result<SpacesState, String> {
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if id.is_empty() {
        cfg.active_space.clear();
        cfg.save();
        return Ok(snapshot(&cfg));
    }
    let record = cfg
        .spaces
        .iter()
        .find(|s| s.id == id)
        .ok_or("that space is not on this PC")?;
    if record.tunnel().is_none() {
        return Err("that space record is incomplete; join it again".into());
    }
    cfg.active_space = record.id.clone();
    cfg.save();
    Ok(snapshot(&cfg))
}

#[tauri::command]
pub async fn space_leave(
    state: tauri::State<'_, crate::State>,
    id: String,
) -> Result<SpacesState, String> {
    let (token, path) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or("that space is not on this PC")?;
        (record.token.clone(), format!("/v1/spaces/{}/leave", record.id))
    };
    if !token.is_empty() {
        // A server that cannot be reached must not trap the user in a space.
        // The local record is still removed; the hub drops the peer when its
        // membership expires or the owner kicks it.
        let _ = call(reqwest::Method::POST, &path, Some(&token), Some(json!({}))).await;
    }
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    cfg.spaces.retain(|s| s.id != id);
    if cfg.active_space == id {
        cfg.active_space.clear();
    }
    cfg.save();
    Ok(snapshot(&cfg))
}

#[tauri::command]
pub async fn space_delete(
    state: tauri::State<'_, crate::State>,
    id: String,
) -> Result<SpacesState, String> {
    let (token, path) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or("that space is not on this PC")?;
        (record.token.clone(), format!("/v1/spaces/{}", record.id))
    };
    if token.is_empty() {
        return Err("this space has no access token; join it again".into());
    }
    call(reqwest::Method::DELETE, &path, Some(&token), None).await?;
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    cfg.spaces.retain(|s| s.id != id);
    if cfg.active_space == id {
        cfg.active_space.clear();
    }
    cfg.save();
    Ok(snapshot(&cfg))
}

#[tauri::command]
pub async fn space_rotate(
    state: tauri::State<'_, crate::State>,
    id: String,
) -> Result<SpacesState, String> {
    let (token, path) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or("that space is not on this PC")?;
        (
            record.token.clone(),
            format!("/v1/spaces/{}/invite", record.id),
        )
    };
    if token.is_empty() {
        return Err("this space has no access token; join it again".into());
    }
    let answer = call(reqwest::Method::POST, &path, Some(&token), Some(json!({}))).await?;
    let invite = answer
        .get("invite")
        .and_then(Value::as_str)
        .ok_or("the space server did not return a new invite")?
        .to_string();
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(record) = cfg.spaces.iter_mut().find(|s| s.id == id) {
        record.invite = invite;
    }
    cfg.save();
    Ok(snapshot(&cfg))
}

#[tauri::command]
pub async fn space_kick(
    state: tauri::State<'_, crate::State>,
    id: String,
    member_id: String,
) -> Result<SpacesState, String> {
    let (token, path) = {
        let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let record = cfg
            .spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or("that space is not on this PC")?;
        (
            record.token.clone(),
            format!("/v1/spaces/{}/kick", record.id),
        )
    };
    if token.is_empty() {
        return Err("this space has no access token; join it again".into());
    }
    call(
        reqwest::Method::POST,
        &path,
        Some(&token),
        Some(json!({"member_id": member_id})),
    )
    .await?;
    drop(token);
    let state = space_refresh(state, id).await?;
    Ok(state)
}
