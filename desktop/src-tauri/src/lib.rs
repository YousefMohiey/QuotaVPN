//! QuotaVPN desktop backend (Tauri + WebView).
//!
//! Same UI and server core as the phone app. The tunnel here is the
//! built-in sing-box + wintun engine (whole-PC TUN), driven by the
//! shared `quotacards::vpn` module - no plugin, no mobile service.
//! Config lives in the OS config dir, so cards carry over from the
//! previous desktop app untouched.

use quotacards::{
    config::{AppConfig, Card, EMBED_KEY, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_USER},
    server, vpn,
};
use std::sync::Mutex;
use tauri::Manager;
use tauri::Emitter;
use tauri::WindowEvent;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_updater::UpdaterExt;

pub mod speed;
pub mod ookla;
pub mod we_quota;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

struct State(Mutex<AppConfig>);
/// Live engine PID. None means no tunnel (or one we no longer track).
struct Engine(Mutex<Option<u32>>);

/// One in-app install at a time. The updater plugin does not serialize
/// downloads, so a second press used to start a whole second transfer and
/// both slowed to a crawl (the user read that as a dead button).
static UPDATE_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(serde::Serialize)]
struct CmdResult {
    ok: bool,
    msg: String,
}

#[derive(serde::Serialize)]
struct UiState {
    server_ip: String,
    ssh_user: String,
    ssh_port: u16,
    cards: Vec<Card>,
    version: String,
    update_channel: String,
}

fn snapshot(cfg: &AppConfig) -> UiState {
    UiState {
        server_ip: cfg.server_ip.clone(),
        ssh_user: cfg.ssh_user.clone(),
        ssh_port: cfg.ssh_port,
        cards: cfg.cards.clone(),
        version: String::new(),
        update_channel: cfg.update_channel.clone(),
    }
}

fn build_link(uuid: &str, host: &str, sni: &str, name: &str) -> String {
    format!("vless://{uuid}@{host}:443?type=tcp&encryption=none&security=tls&fp=random&alpn=h3%2Ch2%2Chttp%2F1.1&allowInsecure=1&sni={sni}#{name}")
}

fn gen_keypair() -> Result<(String, String), String> {
    use ssh_key::{Algorithm, LineEnding, PrivateKey};
    let mut rng = rand_core::OsRng;
    let key = PrivateKey::random(&mut rng, Algorithm::Ed25519).map_err(|e| e.to_string())?;
    let pem: String = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| e.to_string())?
        .to_string();
    Ok((pem, key.public_key().to_string()))
}

fn initial_config() -> AppConfig {
    let mut cfg = AppConfig::load();
    if cfg.private_key.is_empty() {
        if let Ok((priv_pem, pub_line)) = gen_keypair() {
            cfg.private_key = priv_pem;
            cfg.public_key = pub_line;
            cfg.save();
        }
    }
    // Zero-setup: baked-in builds point at the owner's server, nothing to type.
    if let Some(emb) = EMBED_KEY {
        let points_at_mine = cfg.server_ip.is_empty() || cfg.server_ip == DEFAULT_HOST;
        if points_at_mine {
            let mut dirty = false;
            // Heal an empty host on its own: it used to stay stuck on
            // "Set up your server first." forever whenever the embed key was
            // already installed, because the whole block was gated on the key
            // being stale. Each field is repaired independently now.
            if cfg.server_ip.is_empty() {
                cfg.server_ip = DEFAULT_HOST.to_string();
                dirty = true;
            }
            if cfg.private_key != emb {
                cfg.private_key = emb.to_string();
                cfg.ssh_user = DEFAULT_USER.to_string();
                cfg.ssh_port = DEFAULT_PORT;
                cfg.synced = false;
                dirty = true;
            }
            if dirty {
                cfg.save();
            }
        }
    }
    // migrate old cards (no per-card SNI) to a sane default
    let mut dirty = false;
    for c in &mut cfg.cards {
        if c.sni.is_empty() {
            c.sni = "ea.com".to_string();
            dirty = true;
        }
    }
    if dirty {
        cfg.save();
    }
    cfg
}

#[tauri::command]
fn get_state(app: tauri::AppHandle, state: tauri::State<State>) -> UiState {
    let mut st = snapshot(&state.0.lock().unwrap());
    // The running build's own version: the sidebar shows it without waiting
    // for a network round trip to GitHub.
    st.version = app.package_info().version.to_string();
    st
}

#[tauri::command]
async fn probe_server(state: tauri::State<'_, State>) -> Result<CmdResult, String> {
    // Zero-setup: the server is built in, nothing to type. Same probe the
    // phone app runs on open (add + revoke a throwaway client).
    let (host, user, port, key) = {
        let cfg = state.0.lock().unwrap();
        if cfg.server_ip.is_empty() {
            return Ok(CmdResult { ok: false, msg: "No server set up".into() });
        }
        (cfg.server_ip.clone(), cfg.ssh_user.clone(), cfg.ssh_port, cfg.private_key.clone())
    };
    if !server::is_embed_key(&key) {
        if let Err(e) = server::ensure_xray(&host, port, &user, &key).await {
            return Ok(CmdResult { ok: false, msg: format!("Connect failed: {e}") });
        }
    }
    let probe = uuid::Uuid::new_v4().to_string();
    if let Err(e) = server::add_client(&host, port, &user, &key, &probe).await {
        return Ok(CmdResult { ok: false, msg: format!("Connect failed: {e}") });
    }
    if let Err(e) = server::remove_client(&host, port, &user, &key, &probe).await {
        return Ok(CmdResult { ok: false, msg: format!("Connect failed: {e}") });
    }
    // Server moves/rebuilds wipe registrations: re-add any local card the
    // server does not know (add is idempotent, unknown agents just skip).
    if let Ok(remote) = server::list_clients(&host, port, &user, &key).await {
        let missing: Vec<String> = {
            let cfg = state.0.lock().unwrap();
            cfg.cards
                .iter()
                .map(|c| c.uuid.clone())
                .filter(|u| !remote.contains(u))
                .collect()
        };
        for u in missing {
            let _ = server::add_client(&host, port, &user, &key, &u).await;
        }
    }
    Ok(CmdResult { ok: true, msg: "Connected - server ready".into() })
}

/// Watches for the game while the app runs and emits `game-state` on every
/// transition. This lives here, not in the webview: the window spends most
/// of its life minimized or hidden in the tray, and a hidden webview has its
/// JS timers throttled to a crawl, so a JS interval only ever fires while
/// the user is looking at the app - which read as "the voice helper only
/// works from the Valorant page". A plain OS thread cannot be throttled.
fn spawn_game_watch(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut up = false;
        loop {
            let now = process_running("VALORANT-Win64-Shipping.exe".into());
            if now != up {
                up = now;
                let _ = app.emit("game-state", serde_json::json!({ "running": now }));
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

/// One refresh per app build: list the server's clients and re-add every local
/// card it does not know. Silent, detached, and the marker is only written on
/// success, so a launch with no network simply retries on the next one.
fn spawn_launch_refresh(app: tauri::AppHandle) {
    let version = app.package_info().version.to_string();
    let (host, user, port, key, cards) = {
        let state = app.state::<State>();
        let cfg = state.0.lock().unwrap();
        if cfg.server_ip.is_empty() || cfg.cards.is_empty() || cfg.healed_version == version {
            return;
        }
        (
            cfg.server_ip.clone(),
            cfg.ssh_user.clone(),
            cfg.ssh_port,
            cfg.private_key.clone(),
            cfg.cards.iter().map(|c| c.uuid.clone()).collect::<Vec<_>>(),
        )
    };
    tauri::async_runtime::spawn(async move {
        if let Ok(remote) = server::list_clients(&host, port, &user, &key).await {
            for u in cards.iter().filter(|u| !remote.contains(u)) {
                let _ = server::add_client(&host, port, &user, &key, u).await;
            }
            let state = app.state::<State>();
            state.0.lock().unwrap().healed_version = version;
            state.0.lock().unwrap().save();
        }
    });
}

#[tauri::command]
async fn generate_card(
    _app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    name: String,
    kind: String,
    sni: String,
) -> Result<CmdResult, String> {
    let (host, user, port, key) = {
        let cfg = state.0.lock().unwrap();
        if cfg.server_ip.is_empty() {
            return Ok(CmdResult { ok: false, msg: "Set up your server first".into() });
        }
        (cfg.server_ip.clone(), cfg.ssh_user.clone(), cfg.ssh_port, cfg.private_key.clone())
    };
    let uuid = uuid::Uuid::new_v4().to_string();
    let sni = if sni.trim().is_empty() {
        (if kind == "Gamerz" { "ea.com" } else { "youtube.com" }).to_string()
    } else {
        sni.trim().to_string()
    };
    let card = Card {
        name: if name.trim().is_empty() { kind.clone() } else { name.trim().to_string() },
        uuid: uuid.clone(),
        card_type: kind,
        sni: sni.clone(),
        wg_private: String::new(),
        wg_addr: String::new(),
    };
    {
        let mut cfg = state.0.lock().unwrap();
        cfg.cards.push(card);
        cfg.save();
    }
    // Registering on the server is an SSH round trip: run it detached so the
    // card shows up at once. Connect re-adds any card the server is missing.
    if !host.is_empty() {
        let (h, u, k) = (host.clone(), user.clone(), key.clone());
        let id = uuid.clone();
        tauri::async_runtime::spawn(async move {
            let _ = server::add_client(&h, port, &u, &k, &id).await;
        });
    }
    Ok(CmdResult { ok: true, msg: "Card created".into() })
}

#[tauri::command]
async fn import_card(
    state: tauri::State<'_, State>,
    uuid: String,
    name: String,
    kind: String,
    sni: String,
) -> Result<CmdResult, String> {
    let uuid = uuid.trim().to_string();
    if uuid.is_empty() {
        return Ok(CmdResult { ok: false, msg: "That card link has no id".into() });
    }
    let sni = if sni.trim().is_empty() {
        (if kind == "Gamerz" { "ea.com" } else { "youtube.com" }).to_string()
    } else {
        sni.trim().to_string()
    };
    let name = if name.trim().is_empty() { "Card".to_string() } else { name.trim().to_string() };
    {
        let mut cfg = state.0.lock().unwrap();
        if let Some(c) = cfg.cards.iter_mut().find(|c| c.uuid == uuid) {
            // Same card arriving from another device: refresh it, never duplicate.
            c.name = name;
            c.card_type = kind;
            c.sni = sni;
            cfg.save();
            return Ok(CmdResult { ok: true, msg: "Card updated".into() });
        }
        cfg.cards.push(Card {
            name,
            uuid: uuid.clone(),
            card_type: kind,
            sni,
            wg_private: String::new(),
            wg_addr: String::new(),
        });
        cfg.save();
    }
    // Best effort: the server may already know this client, and add is
    // idempotent. Detached so adding a card never waits on SSH.
    let (host, user, port, key) = {
        let cfg = state.0.lock().unwrap();
        (cfg.server_ip.clone(), cfg.ssh_user.clone(), cfg.ssh_port, cfg.private_key.clone())
    };
    if !host.is_empty() {
        let (h, u, k) = (host.clone(), user.clone(), key.clone());
        let id = uuid.clone();
        tauri::async_runtime::spawn(async move {
            let _ = server::add_client(&h, port, &u, &k, &id).await;
        });
    }
    Ok(CmdResult { ok: true, msg: "Card added".into() })
}

#[tauri::command]
async fn revoke_card(state: tauri::State<'_, State>, uuid: String) -> Result<CmdResult, String> {
    let (host, user, port, key) = {
        let mut cfg = state.0.lock().unwrap();
        cfg.cards.retain(|c| c.uuid != uuid);
        cfg.save();
        (cfg.server_ip.clone(), cfg.ssh_user.clone(), cfg.ssh_port, cfg.private_key.clone())
    };
    // The server side is an SSH round trip: run it detached so the list reacts
    // at once. WireGuard peers are per-card, so that peer goes too.
    let (h, u, k) = (host.clone(), user.clone(), key.clone());
    let id = uuid.clone();
    tauri::async_runtime::spawn(async move {
        let _ = server::wg_del(&h, port, &u, &k, &id).await;
        let _ = server::remove_client(&h, port, &u, &k, &id).await;
    });
    Ok(CmdResult { ok: true, msg: "Card revoked".into() })
}

#[tauri::command]
async fn copy_card(state: tauri::State<'_, State>, app: tauri::AppHandle, uuid: String) -> Result<CmdResult, String> {
    let cfg = state.0.lock().unwrap();
    match cfg.cards.iter().find(|c| c.uuid == uuid) {
        Some(c) => {
            let _ = app.clipboard().write_text(build_link(&c.uuid, &cfg.server_ip, &c.sni, &c.name));
            Ok(CmdResult { ok: true, msg: "Link copied".into() })
        }
        None => Ok(CmdResult { ok: false, msg: "Card not found".into() }),
    }
}

/// Change which domain (SNI) a card rides. The card id stays the same, so the
/// server side needs nothing; the value is read on the next connect.
#[tauri::command]
async fn set_card_sni(state: tauri::State<'_, State>, uuid: String, sni: String) -> Result<CmdResult, String> {
    let sni = sni.trim().to_string();
    if sni.is_empty() {
        return Ok(CmdResult { ok: false, msg: "Pick a domain first".into() });
    }
    let mut cfg = state.0.lock().unwrap();
    let found = cfg.cards.iter_mut().find(|c| c.uuid == uuid);
    match found {
        Some(c) => {
            c.sni = sni;
            cfg.save();
            Ok(CmdResult { ok: true, msg: "Domain updated".into() })
        }
        None => Ok(CmdResult { ok: false, msg: "Card not found".into() }),
    }
}

#[derive(serde::Serialize)]
struct TunnelState {
    running: bool,
    error: String,
}

#[derive(serde::Serialize)]
struct TrafficState {
    rx: u64,
    tx: u64,
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| {
            let t = String::from_utf8_lossy(&o.stdout).to_lowercase();
            t.contains("sing-box") && t.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

/// Clear Windows' DNS resolver cache. Standard VPN hygiene, and the fix for
/// lookups that hang in the first seconds after connecting: answers cached
/// before the tunnel existed (or while the fresh tunnel's own DNS was still
/// warming, including failed ones) otherwise keep being served or retried.
/// Best effort, hidden, detached; it never blocks the connect path.
fn flush_dns_cache() {
    use std::process::Stdio;
    let _ = std::process::Command::new("ipconfig")
        .arg("/flushdns")
        .creation_flags(0x08000000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn stop_engine(engine: &tauri::State<Engine>) {
    let pid = engine.0.lock().unwrap().take();
    if let Some(p) = pid {
        vpn::request_graceful_stop(p);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    // The task is the tunnel's owner now: end it, then sweep what a killed
    // engine leaves behind (routes, adapter).
    vpn::stop_engine();
}

fn read_log_tail(n: usize) -> String {
    let txt = std::fs::read_to_string(vpn::engine_log_path()).unwrap_or_default();
    let lines: Vec<&str> = txt.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// True when a process with this exe name is running (case-insensitive).
/// The "Launch with Valorant" toggle leans on this: the UI polls for the
/// game process and arms the voice helper itself when it appears.
#[tauri::command]
fn process_running(name: String) -> bool {
    use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let want = name.to_lowercase();
    if want.is_empty() {
        return false;
    }
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase() == want {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        found
    }
}

/// Start-with-Windows lives in the per-user Run key, so no admin prompt is
/// ever needed. Read and written through the registry API directly: spawning
/// reg.exe for this cost a process start on the UI thread, which is exactly
/// the lag felt when flipping the switch or opening Settings.
const AUTOSTART_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_NAME: &str = "QuotaVPN";

#[cfg(windows)]
mod autostart_reg {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The value data (the stored exe path), or None when absent.
    pub fn read() -> Option<String> {
        let sub = wide(super::AUTOSTART_SUBKEY);
        let name = wide(super::AUTOSTART_NAME);
        let mut buf = [0u16; 2048];
        let mut size = (buf.len() * 2) as u32;
        let st = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(sub.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if st != ERROR_SUCCESS {
            return None;
        }
        let len = ((size as usize / 2).saturating_sub(1)).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    pub fn write(exe: &str) -> Result<(), String> {
        let sub = wide(super::AUTOSTART_SUBKEY);
        let mut hkey = HKEY::default();
        let st = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(sub.as_ptr()),
                None,
                KEY_SET_VALUE,
                &mut hkey,
            )
        };
        if st != ERROR_SUCCESS {
            return Err("Could not open the startup key".into());
        }
        let name = wide(super::AUTOSTART_NAME);
        let data: Vec<u8> = exe
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let st = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(name.as_ptr()),
                None,
                REG_SZ,
                Some(&data),
            )
        };
        let _ = unsafe { RegCloseKey(hkey) };
        if st != ERROR_SUCCESS {
            return Err("Could not write the startup value".into());
        }
        Ok(())
    }

    pub fn remove() -> Result<(), String> {
        let sub = wide(super::AUTOSTART_SUBKEY);
        let mut hkey = HKEY::default();
        let st = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(sub.as_ptr()),
                None,
                KEY_SET_VALUE,
                &mut hkey,
            )
        };
        if st != ERROR_SUCCESS {
            return Err("Could not open the startup key".into());
        }
        let name = wide(super::AUTOSTART_NAME);
        let st = unsafe { RegDeleteValueW(hkey, PCWSTR(name.as_ptr())) };
        let _ = unsafe { RegCloseKey(hkey) };
        if st != ERROR_SUCCESS {
            return Err("Could not remove the startup value".into());
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod autostart_reg {
    pub fn read() -> Option<String> {
        None
    }
    pub fn write(_exe: &str) -> Result<(), String> {
        Err("Not supported on this platform".into())
    }
    pub fn remove() -> Result<(), String> {
        Err("Not supported on this platform".into())
    }
}

#[tauri::command]
fn autostart_get() -> Result<bool, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(autostart_reg::read().as_deref() == Some(exe.display().to_string().as_str()))
}

#[tauri::command]
fn autostart_set(on: bool) -> Result<(), String> {
    if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        autostart_reg::write(&exe.display().to_string())
    } else {
        autostart_reg::remove().or_else(|e| {
            // Removing a value that is not there is success, not an error.
            if autostart_reg::read().is_none() {
                Ok(())
            } else {
                Err(e)
            }
        })
    }
}

#[tauri::command]
async fn tunnel_start(
    app: tauri::AppHandle,
    _state: tauri::State<'_, State>,
    engine: tauri::State<'_, Engine>,
    uuid: String,
    apps_mode: Option<String>,
    apps: Option<Vec<String>>,
    transport: Option<String>,
    voice: Option<bool>,
) -> Result<CmdResult, String> {
    let (host, user, port, key, card) = {
        let state = app.state::<State>();
        let cfg = state.0.lock().unwrap();
        let Some(card) = cfg.cards.iter().find(|c| c.uuid == uuid).cloned() else {
            return Ok(CmdResult { ok: false, msg: "Card not found".into() });
        };
        (
            cfg.server_ip.clone(),
            cfg.ssh_user.clone(),
            cfg.ssh_port,
            cfg.private_key.clone(),
            card,
        )
    };
    if host.is_empty() {
        return Ok(CmdResult { ok: false, msg: "Set up your server first".into() });
    }
    let t = match transport.as_deref().unwrap_or("vless") {
        "hy2" => "hy2",
        "wg" => "wg",
        _ => "vless",
    };
    // Provision whatever the transport needs, cached after first use: the
    // shared Hysteria2 password, or this card's WireGuard keypair + address.
    let mut hy2_pass = String::new();
    let mut wg_tuple: Option<(String, String, String, String)> = None;
    if t == "hy2" {
        let cached = app.state::<State>().0.lock().unwrap().hy2_password.clone();
        hy2_pass = if cached.is_empty() {
            match server::hy2_password(&host, port, &user, &key).await {
                Ok(p) => {
                    let state = app.state::<State>();
                    let mut st = state.0.lock().unwrap();
                    st.hy2_password = p.clone();
                    st.save();
                    p
                }
                Err(e) => {
                    return Ok(CmdResult {
                        ok: false,
                        msg: format!("Could not set up the game transport: {e}"),
                    })
                }
            }
        } else {
            cached
        };
    } else if t == "wg" {
        // The obfuscation parameters are shared by every card: fetch once.
        // A cached copy is only worth keeping if it parses - an older server
        // shipped the params pretty-printed and the first line alone is a
        // bare "{", which is not JSON.
        let mut awg_json = app.state::<State>().0.lock().unwrap().awg_params.clone();
        if awg_json.is_empty() || serde_json::from_str::<serde_json::Value>(&awg_json).is_err() {
            match server::awg_params(&host, port, &user, &key).await {
                Ok(p) => {
                    let state = app.state::<State>();
                    let mut st = state.0.lock().unwrap();
                    st.awg_params = p.clone();
                    st.save();
                    awg_json = p;
                }
                Err(e) => {
                    return Ok(CmdResult {
                        ok: false,
                        msg: format!("Could not set up WireGuard: {e}"),
                    })
                }
            }
        }
        if card.wg_private.is_empty() || card.wg_addr.is_empty() {
            let creds = match server::wg_add(&host, port, &user, &key, &card.uuid).await {
                Ok(c) => c,
                Err(e) => {
                    return Ok(CmdResult {
                        ok: false,
                        msg: format!("Could not set up WireGuard: {e}"),
                    })
                }
            };
            let state = app.state::<State>();
            let mut st = state.0.lock().unwrap();
            if let Some(cc) = st.cards.iter_mut().find(|c| c.uuid == uuid) {
                cc.wg_private = creds.private_key.clone();
                cc.wg_addr = creds.address.clone();
            }
            st.wg_server_pub = creds.server_pub.clone();
            st.save();
            wg_tuple = Some((creds.private_key, creds.address, creds.server_pub, awg_json.clone()));
        } else {
            let mut srvpub = app.state::<State>().0.lock().unwrap().wg_server_pub.clone();
            if srvpub.is_empty() {
                match server::wg_server_pub(&host, port, &user, &key).await {
                    Ok(p) => {
                        srvpub = p;
                        let state = app.state::<State>();
                        let mut st = state.0.lock().unwrap();
                        st.wg_server_pub = srvpub.clone();
                        st.save();
                    }
                    Err(e) => {
                        return Ok(CmdResult {
                            ok: false,
                            msg: format!("Could not set up WireGuard: {e}"),
                        })
                    }
                }
            }
            wg_tuple = Some((card.wg_private.clone(), card.wg_addr.clone(), srvpub, awg_json.clone()));
        }
    }
    // The tunnel runs through a helper task registered once at setup, so this
    // process never needs to be elevated. Without that task the UI offers the
    // one-time setup instead of telling people to run the app as admin.
    if !vpn::task_installed() {
        return Ok(CmdResult { ok: false, msg: "setup-needed".into() });
    }
    let voice_on = voice.unwrap_or(false);
    // The session keeps whatever normal configuration the app was using and
    // the voice rules are added inside it, so the normal VPN and the voice
    // helper run in the same tunnel at once.
    let mode = apps_mode.unwrap_or_default();
    let list = apps.unwrap_or_default();
    vpn::ensure_engine().map_err(|e| e)?;
    let wg_ref = wg_tuple.as_ref().map(|(p, a, s, j)| (p.as_str(), a.as_str(), s.as_str(), j.as_str()));
    vpn::write_tun_config(&card.uuid, &host, &card.sni, &mode, &list, voice_on, t, &hy2_pass, wg_ref)
        .map_err(|e| e)?;
    vpn::check_config().map_err(|e| e)?;
    stop_engine(&engine);
    let mode_label = if voice_on {
        "voice"
    } else if mode == "allow" && !list.is_empty() {
        "apps"
    } else if mode == "block" && !list.is_empty() {
        "except"
    } else {
        "all"
    };
    vpn::app_log(&format!(
        "connect {} mode={} apps={}",
        card.name, mode_label, list.len()
    ));
    // Fresh DNS for the new session: entries from before the tunnel existed
    // are what make the first lookups after connect hang.
    flush_dns_cache();
    // A dead engine leaves its adapter behind, and Windows then refuses to
    // give the next one out until the machine restarts. Anything still there
    // while nothing of ours is running is a ghost: remove it first.
    if vpn::tun_adapter_present() {
        vpn::app_log("leftover tun adapter, clearing it first");
        vpn::reset_tun_adapter();
    }
    let mut pid = vpn::start_engine().map_err(|e| e)?;
    *engine.0.lock().unwrap() = Some(pid);
    // Wait for the tun to actually carry traffic. A cold wintun adapter can
    // take several seconds to publish its routes, and a fixed short sleep
    // made the first connect of a session fail while the engine was healthy;
    // pressing again only worked because the driver was warm by then.
    let mut up = false;
    for attempt in 0..2 {
        for _ in 0..40 {
            if *engine.0.lock().unwrap() != Some(pid) {
                // A stop during settle clears the pid: drop our engine, report stopped.
                vpn::request_graceful_stop(pid);
                return Ok(CmdResult { ok: false, msg: "Stopped".into() });
            }
            if pid != 0 && !pid_alive(pid) {
                break;
            }
            if vpn::tun_routes_present() {
                up = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        if up || (pid != 0 && pid_alive(pid)) || attempt == 1 {
            break;
        }
        // It died before its routes landed: the usual cause is the adapter
        // the previous run left. Clean that up and give it one more go, so a
        // wedged handover never costs the user a reboot.
        vpn::app_log("engine died on start: clearing the adapter and retrying once");
        vpn::stop_engine();
        vpn::reset_tun_adapter();
        pid = vpn::start_engine().map_err(|e| e)?;
        *engine.0.lock().unwrap() = Some(pid);
    }
    if up {
        // Clear whatever the warmup window cached, failed lookups included,
        // so the checks right after connect get fresh answers.
        flush_dns_cache();
        Ok(CmdResult { ok: true, msg: format!("VPN on - {} carries the traffic", card.name) })
    } else if pid_alive(pid) {
        // Alive but not routing yet: keep it running and let the status poll
        // adopt it once the routes land, instead of killing a healthy engine.
        Ok(CmdResult {
            ok: false,
            msg: "The tunnel is still starting - give it a few seconds or connect again".into(),
        })
    } else {
        // A task start can show up a moment after our wait window: adopt what
        // is actually running instead of reporting a failure over a tunnel
        // that is already carrying traffic. Never kill a live engine here -
        // the poll will bring the UI in line either way.
        if let Some(live) = vpn::task_engine_pid() {
            *engine.0.lock().unwrap() = Some(live);
            if vpn::tun_routes_present() {
                flush_dns_cache();
                return Ok(CmdResult {
                    ok: true,
                    msg: format!("VPN on - {} carries the traffic", card.name),
                });
            }
            return Ok(CmdResult {
                ok: false,
                msg: "The tunnel is still starting - give it a few seconds or connect again".into(),
            });
        }
        *engine.0.lock().unwrap() = None;
        vpn::engine_cleanup();
        Ok(CmdResult {
            ok: false,
            msg: format!("The tunnel did not come up: {}", vpn::engine_log_tail()),
        })
    }
}

/// The one-time, privileged setup. Windows asks for administrator here and
/// only here: the elevated pass registers the helper task, and every later
/// connect runs through it without any prompt at all.
#[tauri::command]
async fn setup_helper() -> Result<CmdResult, String> {
    vpn::app_log("setup: asked for the one-time setup");
    if vpn::task_installed() {
        return Ok(CmdResult { ok: true, msg: "ready".into() });
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let ps = format!(
        "Start-Process -FilePath '{}' -ArgumentList '--setup-helper' -Verb RunAs -WindowStyle Hidden",
        exe.display()
    );
    let launched = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(0x08000000)
        .output();
    // Keep what Windows said: "cancelled by the user", "no permission" and a
    // silent failure all look identical from here otherwise.
    let refusal = match &launched {
        Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
        Err(e) => e.to_string(),
    };
    vpn::app_log(&format!("setup: elevation requested, refused={refusal}"));
    // A refused prompt comes back here as a raw PowerShell exception. The user
    // needs to know what to do, not what .NET called it, so keep the detail in
    // the log and answer in plain words.
    let cancelled = refusal.contains("canceled")
        || refusal.contains("cancelled")
        || refusal.contains("Incorrect function")
        || refusal.contains("The operation was canceled");
    if !refusal.is_empty() && !cancelled {
        return Ok(CmdResult { ok: false, msg: "The tunnel setup did not run".into() });
    }
    if cancelled {
        return Ok(CmdResult {
            ok: false,
            msg: "Windows did not allow the setup. Approve the prompt to finish it".into(),
        });
    }
    for _ in 0..40 {
        if vpn::task_installed() {
            return Ok(CmdResult { ok: true, msg: "ready".into() });
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Ok(CmdResult {
        ok: false,
        msg: "Windows did not allow the setup. Approve the prompt to finish it".into(),
    })
}

#[tauri::command]
async fn tunnel_stop(engine: tauri::State<'_, Engine>) -> Result<CmdResult, String> {
    vpn::app_log("stop from app");
    stop_engine(&engine);
    // Drop the tunnel's DNS answers so normal resolution resumes cleanly.
    flush_dns_cache();
    Ok(CmdResult { ok: true, msg: "VPN off".into() })
}

#[tauri::command]
async fn tunnel_status(engine: tauri::State<'_, Engine>) -> Result<TunnelState, String> {
    let pid = engine.0.lock().unwrap().clone();
    match pid {
        Some(p) if pid_alive(p) => {
            // The process is what the state owns: keep it while it lives,
            // even before its routes land (a cold adapter can lag). Clearing
            // it here stranded a live engine that the UI could no longer
            // stop. Only a dead process clears the state and speaks.
            if vpn::tun_routes_present() {
                Ok(TunnelState { running: true, error: String::new() })
            } else {
                Ok(TunnelState { running: false, error: String::new() })
            }
        }
        Some(_) => {
            *engine.0.lock().unwrap() = None;
            Ok(TunnelState { running: false, error: vpn::engine_log_tail() })
        }
        None => {
            // Adopt a live engine we lost track of: the task owns it now, and
            // after an app restart the pid file points at it.
            if let Some(p) = vpn::task_engine_pid() {
                *engine.0.lock().unwrap() = Some(p);
                return Ok(TunnelState { running: vpn::tun_routes_present(), error: String::new() });
            }
            if vpn::tun_routes_present() {
                if let Ok(s) = std::fs::read_to_string(vpn::engine_dir().join("engine.pid")) {
                    if let Ok(p) = s.trim().parse::<u32>() {
                        if p != 0 && pid_alive(p) {
                            *engine.0.lock().unwrap() = Some(p);
                            return Ok(TunnelState { running: true, error: String::new() });
                        }
                    }
                }
            }
            Ok(TunnelState { running: false, error: String::new() })
        }
    }
}

/// A real liveness check for the tunnel: ask the engine, over its localhost
/// Clash API, to dial a known endpoint through the proxy outbound and report
/// the round trip. -1 means the upstream is not carrying traffic, which is
/// what keeps the UI from claiming "connected" while the server path is dead.
#[tauri::command]
async fn tunnel_health() -> Result<i64, String> {
    let raw = std::fs::read_to_string(vpn::engine_dir().join("clash-ctl.txt"))
        .map_err(|e| e.to_string())?;
    let mut lines = raw.lines();
    let port: u16 = lines
        .next()
        .unwrap_or("")
        .trim()
        .parse()
        .map_err(|_| "clash port".to_string())?;
    let secret = lines.next().unwrap_or("").trim().to_string();
    let url = format!(
        "http://127.0.0.1:{port}/proxies/proxy/delay?url=https%3A%2F%2Fwww.gstatic.com%2Fgenerate_204&timeout=6000"
    );
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Ok(-1),
    };
    match client
        .get(url)
        .header("Authorization", format!("Bearer {secret}"))
        .send()
        .await
    {
        Ok(r) => {
            let v: serde_json::Value = r.json().await.unwrap_or(serde_json::Value::Null);
            Ok(v["delay"].as_i64().unwrap_or(-1))
        }
        Err(_) => Ok(-1),
    }
}

/// Session counts for the tunnel itself: the TUN adapter's own byte
/// counters (InOctets = downloaded, OutOctets = uploaded). The adapter
/// only exists while the engine runs, so zeros mean nothing to show.
#[cfg(windows)]
fn tun_octets() -> (u64, u64) {
    use windows::Win32::Foundation::NO_ERROR;
    use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table) != NO_ERROR || table.is_null() {
            return (0, 0);
        }
        let rows = std::slice::from_raw_parts(
            (*table).Table.as_ptr(),
            (*table).NumEntries as usize,
        );
        // The session's own adapter name is written next to the config when
        // the engine starts. Matching it exactly matters: other products'
        // wintun adapters and ghosts of force-killed runs share the vague
        // words in their description, and the old "busiest match" pick could
        // report the wrong adapter's traffic entirely.
        let want = std::fs::read_to_string(vpn::engine_dir().join("tun-ifname.txt"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let mut best = (0u64, 0u64);
        for row in rows {
            let alias = String::from_utf16_lossy(&row.Alias);
            let desc = String::from_utf16_lossy(&row.Description);
            let alias = alias.trim_end_matches('\0');
            let desc = desc.trim_end_matches('\0');
            let ours = match &want {
                Some(w) => alias == w.as_str(),
                None => {
                    alias.starts_with("QuotaCards")
                        || alias.starts_with("QuotaVPN")
                        || desc.contains("QuotaCards")
                        || desc.contains("QuotaVPN")
                        || desc.to_ascii_lowercase().contains("wintun")
                        || desc.to_ascii_lowercase().contains("sing-box")
                }
            };
            if ours && row.InOctets + row.OutOctets > best.0 + best.1 {
                best = (row.InOctets, row.OutOctets);
            }
        }
        FreeMibTable(table as *const core::ffi::c_void);
        best
    }
}

#[cfg(not(windows))]
fn tun_octets() -> (u64, u64) {
    (0, 0)
}

/// Session traffic in bytes, live from the TUN adapter counters.
#[tauri::command]
async fn tunnel_traffic() -> Result<TrafficState, String> {
    let (rx, tx) = tun_octets();
    Ok(TrafficState { rx, tx })
}

#[tauri::command]
async fn tunnel_log() -> Result<String, String> {
    Ok(read_log_tail(300))
}

#[tauri::command]
async fn tunnel_copy_log(app: tauri::AppHandle) -> Result<CmdResult, String> {
    let lines = read_log_tail(300);
    if lines.trim().is_empty() {
        return Ok(CmdResult { ok: false, msg: "No log yet - connect the VPN first".into() });
    }
    app.clipboard()
        .write_text(lines)
        .map_err(|e| e.to_string())?;
    Ok(CmdResult { ok: true, msg: "Log copied - paste it to Cypher".into() })
}

/// Curated Windows processes for per-app routing (exe name + label).
/// The engine matches by process name/path; whole-PC stays the default.
#[tauri::command]
async fn tunnel_apps() -> Result<String, String> {
    // Live list: whatever Windows is actually running right now, one row
    // per distinct exe. Nothing hardcoded, nothing curated.
    const SKIP: &[&str] = &[
        "system", "registry", "smss.exe", "csrss.exe", "wininit.exe", "winlogon.exe",
        "services.exe", "lsass.exe", "lsm.exe", "svchost.exe", "dwm.exe", "explorer.exe",
        "ctfmon.exe", "conhost.exe", "dllhost.exe", "taskhostw.exe", "sihost.exe",
        "fontdrvhost.exe", "backgroundtaskhost.exe", "runtimebroker.exe",
        "searchindexer.exe", "searchhost.exe", "startmenuexperiencehost.exe",
        "shellexperiencehost.exe", "applicationframehost.exe", "systemsettings.exe",
        "lockapp.exe", "widgets.exe", "msedgewebview2.exe", "quotacards.exe", "quotavpn.exe",
        "tasklist.exe",
    ];
    let out = std::process::Command::new("tasklist.exe")
        .args(["/FO", "CSV", "/NH"])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("list failed: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut seen = std::collections::BTreeSet::<String>::new();
    let mut rows: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('"') {
            continue;
        }
        let end = match line[1..].find('"') {
            Some(i) => i + 1,
            None => continue,
        };
        let exe = line[1..end].to_string();
        let low = exe.to_ascii_lowercase();
        if !low.ends_with(".exe") || SKIP.contains(&low.as_str()) {
            continue;
        }
        if !seen.insert(low) {
            continue;
        }
        let label = exe.strip_suffix(".exe").unwrap_or(&exe).to_string();
        rows.push((exe, label));
        if rows.len() >= 400 {
            break;
        }
    }
    rows.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    let v: Vec<serde_json::Value> = rows
        .iter()
        .map(|(pkg, label)| serde_json::json!({ "pkg": pkg, "label": label }))
        .collect();
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

/// Resolve the server name to its numeric IP for display (the UI shows the
/// real IP; the name stays for the tunnel itself). Prefers IPv4.
#[tauri::command]
async fn resolve_host(host: String) -> Result<String, String> {
    let mut addrs = tokio::net::lookup_host((host.as_str(), 443))
        .await
        .map_err(|e| format!("cannot resolve {host}: {e}"))?;
    let mut v6: Option<String> = None;
    for a in &mut addrs {
        let ip = a.ip();
        if ip.is_ipv4() {
            return Ok(ip.to_string());
        }
        if v6.is_none() {
            v6 = Some(ip.to_string());
        }
    }
    v6.ok_or_else(|| format!("cannot resolve {host}"))
}

/// Raw TCP check to server:443. Run BEFORE starting the VPN: if this fails,
/// the carrier blocks the port and no tunnel can work. Run AFTER: proves
/// traffic flows through the tunnel.
#[tauri::command]
async fn tunnel_probe(app: tauri::AppHandle) -> Result<CmdResult, String> {
    // The engine is the honest signal. A TCP connect to the server is only
    // meaningful for TCP transports: Hysteria2 rides UDP, so probing port 443
    // over TCP reported a healthy tunnel as blocked and left the dial on
    // "working" for a full minute of timeouts.
    if vpn::task_engine_pid().is_some() || vpn::tun_routes_present() {
        return Ok(CmdResult { ok: true, msg: "Tunnel up".into() });
    }
    let host = {
        let state = app.state::<State>();
        let guard = state.0.lock().unwrap();
        guard.server_ip.clone()
    };
    if host.is_empty() {
        return Ok(CmdResult { ok: false, msg: "No server set".into() });
    }
    let target = format!("{host}:443");
    let conn = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(&target),
    )
    .await;
    match conn {
        Ok(Ok(_)) => Ok(CmdResult { ok: true, msg: format!("{target} reachable") }),
        Ok(Err(e)) => Ok(CmdResult { ok: false, msg: format!("{target} refused: {e}") }),
        Err(_) => Ok(CmdResult { ok: false, msg: format!("{target} timed out (blocked?)") }),
    }
}

/// Update check through the signed updater feed. Reports only; the
/// install path is `apply_update` (official flow, restarts by itself).
#[derive(serde::Serialize)]
struct UpdateInfo {
    current: String,
    latest: String,
    available: bool,
    url: String,
    notes: String,
}

/// Exit address plus provider for the speed page, resolved in Rust so no
/// webview policy, CORS rule or fetch quirk can block it. The UI tries its
/// own lookups first; this is the dependable path.
#[derive(serde::Serialize)]
struct NetInfo {
    ip: String,
    isp: String,
    place: String,
}

fn join_place(city: Option<&str>, country: Option<&str>) -> String {
    match (city, country) {
        (Some(c), Some(k)) => format!("{c}, {k}"),
        (Some(c), None) => c.to_string(),
        (None, Some(k)) => k.to_string(),
        _ => String::new(),
    }
}

/// One provider's answer. The two JSON services carry the provider and the
/// place; Cloudflare's trace carries the bare address.
async fn net_from_ipwho(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client.get("https://ipwho.is/").send().await.ok()?;
    let v = r.json::<serde_json::Value>().await.ok()?;
    if v.get("success").and_then(|x| x.as_bool()) == Some(false) {
        return None;
    }
    let ip = v.get("ip").and_then(|x| x.as_str())?.to_string();
    if ip.is_empty() {
        return None;
    }
    let isp = v
        .pointer("/connection/isp")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/connection/org").and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string();
    let place = join_place(
        v.get("city").and_then(|x| x.as_str()),
        v.get("country").and_then(|x| x.as_str()),
    );
    Some(NetInfo { ip, isp, place })
}

async fn net_from_ipapi(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client.get("https://ipapi.co/json/").send().await.ok()?;
    let v = r.json::<serde_json::Value>().await.ok()?;
    let ip = v.get("ip").and_then(|x| x.as_str())?.to_string();
    if ip.is_empty() {
        return None;
    }
    let isp = v.get("org").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let place = join_place(
        v.get("city").and_then(|x| x.as_str()),
        v.get("country_name").and_then(|x| x.as_str()),
    );
    Some(NetInfo { ip, isp, place })
}

async fn net_from_cf(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client.get("https://cloudflare.com/cdn-cgi/trace").send().await.ok()?;
    let body = r.text().await.ok()?;
    let ip = body.lines().find_map(|l| l.strip_prefix("ip="))?.trim().to_string();
    if ip.is_empty() {
        return None;
    }
    Some(NetInfo { ip, isp: String::new(), place: String::new() })
}

#[tauri::command]
async fn net_info() -> Option<NetInfo> {
    // The two JSON providers are raced against each other: whichever answers
    // first wins and a blocked or slow one simply hands the job over. Waiting
    // on them one after another is what used to leave this line lagging
    // seconds behind the rest of the window. Cloudflare backs both up with
    // the bare address.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("QuotaVPN")
        .build()
        .ok()?;
    tokio::select! {
        r = net_from_ipwho(&client) => r.or(net_from_ipapi(&client).await).or(net_from_cf(&client).await),
        r = net_from_ipapi(&client) => r.or(net_from_ipwho(&client).await).or(net_from_cf(&client).await),
    }
}

/// One app's icon as a base64 PNG, pulled straight from the executable's own
/// resources. An empty string means the shell had nothing readable for it and
/// the row falls back to a letter tile.
#[derive(serde::Serialize)]
struct AppIconOut {
    pkg: String,
    png: String,
}

#[tauri::command]
async fn app_icons(pkgs: Vec<String>) -> Vec<AppIconOut> {
    tauri::async_runtime::spawn_blocking(move || {
        // The routing list carries process names; the icons live on disk, so
        // resolve every name to its executable's full path first. A name we
        // cannot reach (elevated or protected) simply yields no icon.
        let paths = process_paths();
        pkgs.into_iter()
            .map(|pkg| {
                let path = if pkg.contains('\\') || pkg.contains('/') {
                    pkg.clone()
                } else {
                    paths.get(&pkg.to_ascii_lowercase()).cloned().unwrap_or_default()
                };
                let png = if path.is_empty() {
                    String::new()
                } else {
                    icon_png(&path).map(|bytes| b64(&bytes)).unwrap_or_default()
                };
                AppIconOut { pkg, png }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Full path per running process name, so the icon reader has a real file to
/// point at. Processes we cannot open are absent, and their rows tile.
#[cfg(windows)]
fn process_paths() -> std::collections::HashMap<String, String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut map = std::collections::HashMap::new();
    unsafe {
        let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return map,
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(&entry.szExeFile)
                    .trim_end_matches('\0')
                    .to_ascii_lowercase();
                if !name.is_empty() && !map.contains_key(&name) {
                    if let Ok(h) =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, entry.th32ProcessID)
                    {
                        let mut buf = [0u16; 1024];
                        let mut len = buf.len() as u32;
                        if QueryFullProcessImageNameW(
                            h,
                            PROCESS_NAME_WIN32,
                            windows::core::PWSTR(buf.as_mut_ptr()),
                            &mut len,
                        )
                        .is_ok()
                        {
                            map.insert(name, String::from_utf16_lossy(&buf[..len as usize]));
                        }
                        let _ = CloseHandle(h);
                    }
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    map
}

#[cfg(not(windows))]
fn process_paths() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::new()
}

/// A small base64 so icon bytes can ride inside JSON without a new crate.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Read a GDI bitmap into raw bytes at the requested depth.
#[cfg(windows)]
unsafe fn read_dib(hbm: windows::Win32::Graphics::Gdi::HBITMAP, w: i32, h: i32, bpp: u16) -> Option<Vec<u8>> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    let mut bi = BITMAPINFO::default();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = w;
    bi.bmiHeader.biHeight = -h; // top-down rows
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = bpp;
    bi.bmiHeader.biCompression = BI_RGB.0;
    let row = ((w as usize * bpp as usize + 31) / 32) * 4;
    let mut buf = vec![0u8; row * h as usize];
    let dc = CreateCompatibleDC(None);
    if dc.0.is_null() {
        return None;
    }
    let lines = GetDIBits(dc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
    let _ = DeleteDC(dc);
    if lines == 0 {
        return None;
    }
    Some(buf)
}

/// The executable's own icon (the shell's large size, 32px) as PNG bytes.
#[cfg(windows)]
fn icon_png(path: &str) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HGDIOBJ};
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    unsafe {
        // The shell helpers want COM up on the thread that calls them.
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        );
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut shfi = SHFILEINFOW::default();
        let got = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if got == 0 || shfi.hIcon.is_invalid() {
            return None;
        }
        let hicon = shfi.hIcon;
        let out = (|| {
            let mut ii = ICONINFO::default();
            GetIconInfo(hicon, &mut ii).ok()?;
            let res = (|| {
                let color = if !ii.hbmColor.0.is_null() { ii.hbmColor } else { ii.hbmMask };
                let mut bm = BITMAP::default();
                if GetObjectW(
                    HGDIOBJ(color.0),
                    std::mem::size_of::<BITMAP>() as i32,
                    Some(&mut bm as *mut _ as *mut _),
                ) == 0
                {
                    return None;
                }
                let (w, h) = (bm.bmWidth, bm.bmHeight);
                if w <= 0 || h <= 0 || w > 1024 || h > 1024 {
                    return None;
                }
                let mut px = read_dib(color, w, h, 32)?;
                if !px.chunks(4).any(|p| p[3] != 0) {
                    // Legacy icons keep transparency in the AND mask.
                    let mask = read_dib(ii.hbmMask, w, h, 1)?;
                    let row = ((w as usize + 31) / 32) * 4;
                    for y in 0..h as usize {
                        for x in 0..w as usize {
                            let bit = (mask[y * row + x / 8] >> (7 - (x % 8))) & 1;
                            let p = &mut px[(y * w as usize + x) * 4..][..4];
                            if bit == 1 {
                                p[0] = 0;
                                p[1] = 0;
                                p[2] = 0;
                                p[3] = 0;
                            } else {
                                p[3] = 255;
                            }
                        }
                    }
                }
                for p in px.chunks_mut(4) {
                    p.swap(0, 2); // BGRA -> RGBA
                }
                let img = image::RgbaImage::from_raw(w as u32, h as u32, px)?;
                let mut out = std::io::Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(img)
                    .write_to(&mut out, image::ImageFormat::Png)
                    .ok()?;
                Some(out.into_inner())
            })();
            if !ii.hbmColor.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
            }
            if !ii.hbmMask.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
            }
            res
        })();
        let _ = DestroyIcon(hicon);
        out
    }
}

#[cfg(not(windows))]
fn icon_png(_path: &str) -> Option<Vec<u8>> {
    None
}

/// The speed-test server list, fetched backend-side. Two sources: Ookla's
/// public list (the same one speedtest.net and its CLI select from, which
/// includes servers hosted inside providers like this user's own ISP) and the
/// LibreSpeed public list as a fallback pool. Neither sends CORS headers, so
/// the webview cannot read them, only the backend can.
#[tauri::command]
async fn speed_servers() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent(BROWSER_UA)
        .build()
        .ok()?;

    let mut ookla: Option<String> = None;
    let mut libre: Option<String> = None;

    let ookla_url = "https://www.speedtest.net/api/js/servers?engine=js&limit=60";
    if let Ok(r) = client.get(ookla_url).header("Accept", "application/json").send().await {
        if r.status().is_success() {
            if let Ok(t) = r.text().await {
                let t = t.trim().to_string();
                if t.starts_with('[') {
                    ookla = Some(t);
                }
            }
        }
    }

    if let Ok(r) = client
        .get("https://librespeed.org/backend-servers/servers.php")
        .send()
        .await
    {
        if r.status().is_success() {
            if let Ok(t) = r.text().await {
                let t = t.trim().to_string();
                if t.starts_with('[') {
                    libre = Some(t);
                }
            }
        }
    }

    if ookla.is_none() && libre.is_none() {
        return None;
    }
    Some(format!(
        "{{\"ookla\":{},\"librespeed\":{}}}",
        ookla.unwrap_or_else(|| "null".into()),
        libre.unwrap_or_else(|| "null".into())
    ))
}

const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0 Safari/537.36";

/// No overall timeout: a measurement stream is supposed to run for seconds.
/// `follow` is off for latency probes: the first response is the round trip,
/// and for servers that redirect http to https, following it would time two
/// trips and report a ping twice the real one.
fn speed_client(follow: bool) -> reqwest::Client {
    let policy = if follow {
        reqwest::redirect::Policy::default()
    } else {
        reqwest::redirect::Policy::none()
    };
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(6))
        // No single read may stall forever: a download or upload phase must be
        // able to give up and report why.
        .read_timeout(std::time::Duration::from_secs(4))
        .user_agent(BROWSER_UA)
        .redirect(policy)
        // HTTP/1.1 for the measurement: these endpoints serve bytes and acks,
        // and h1 avoids HTTP/2 flow-control stalls through the odd middlebox.
        .http1_only()
        .build()
        .unwrap_or_default()
}

#[tauri::command]
async fn speed_latency(url: String, probes: u32) -> Vec<f64> {
    speed::latency(&speed_client(false), &url, probes.clamp(1, 10)).await
}

/// Whether a server can actually be used end to end: the probe follows
/// redirects, because several provider hosts answer their own name with a 307
/// to an Ookla hostname, and a machine that cannot resolve or reach that
/// second name fails on every download and upload while a plain ping still
/// works. Candidates that cannot complete the full path are not offered.
#[tauri::command]
async fn speed_reach(url: String) -> bool {
    !speed::latency(&speed_client(true), &url, 1).await.is_empty()
}

#[tauri::command]
async fn speed_down(app: tauri::AppHandle, urls: Vec<String>, seconds: f64) -> speed::SpeedOut {
    speed::download(&speed_client(true), &urls, seconds.clamp(1.0, 30.0), |v| {
        let _ = app.emit("speed-tick", v);
    })
    .await
}

#[tauri::command]
async fn speed_up(
    app: tauri::AppHandle,
    url: String,
    seconds: f64,
    chunk_mb: Option<u32>,
) -> speed::SpeedOut {
    let chunk = chunk_mb.unwrap_or(2).clamp(1, 8) as usize;
    speed::upload(&speed_client(true), &url, seconds.clamp(2.0, 30.0), chunk, |v| {
        let _ = app.emit("speed-tick", v);
    })
    .await
}

/// Whether the bundled official client is fetched and runnable. The first
/// call downloads it once and caches it under the app data dir.
#[tauri::command]
async fn speedtest_cli_ready() -> bool {
    ookla::ensure_binary().await.is_some()
}

/// One full test through the official Ookla client. Live progress arrives as
/// `speed-phase` and `speed-tick` events.
#[tauri::command]
async fn speedtest_cli(app: tauri::AppHandle) -> Option<ookla::CliResult> {
    let exe = ookla::ensure_binary().await?;
    let a1 = app.clone();
    let on_phase: std::sync::Arc<dyn Fn(&str) + Send + Sync> =
        std::sync::Arc::new(move |p: &str| {
            let _ = a1.emit("speed-phase", p.to_string());
        });
    let a2 = app.clone();
    let on_tick: std::sync::Arc<dyn Fn(f64) + Send + Sync> =
        std::sync::Arc::new(move |v: f64| {
            let _ = a2.emit("speed-tick", v);
        });
    let a3 = app.clone();
    let on_result: std::sync::Arc<dyn Fn(serde_json::Value) + Send + Sync> =
        std::sync::Arc::new(move |j: serde_json::Value| {
            let _ = a3.emit("speed-result", j);
        });
    tauri::async_runtime::spawn_blocking(move || ookla::run(&exe, on_phase, on_tick, on_result))
        .await
        .ok()
        .flatten()
}

fn newer(latest: &str, current: &str) -> bool {
    // Numeric dot-part compare, no semver crate needed.
    let p = |s: &str| {
        s.split('.')
            .map(|x| x.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    p(latest) > p(current)
}

/// Short git hash of the build this binary came from (stamped in by
/// build.rs), or "dev" when git was not available at build time. The feed
/// carries the same stamp, which is how a build identifies itself; the
/// updater no longer offers a build of the version it is already running.
#[allow(dead_code)]
const BUILD_STAMP: &str = env!("QC_BUILD");

/// Everything up to date: the shape the UI already renders as "latest".
fn up_to_date(current: &str) -> UpdateInfo {
    UpdateInfo {
        available: false,
        url: String::new(),
        latest: current.to_string(),
        notes: String::new(),
        current: current.to_string(),
    }
}

/// The updater handle for a channel. QC_FEED_URL repoints it at a local
/// feed so the check and install paths can be exercised without publishing
/// a release. The experimental channel resolves the newest published
/// release itself, pre-releases included; the stable channel keeps the
/// built-in endpoint, which GitHub only serves for full releases.
async fn updater(app: &tauri::AppHandle, channel: &str) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app.updater_builder();
    // The plugin's default compare demands a strictly newer version number,
    // but a build of a version line (every test build, every hotfix re-release)
    // keeps its number and differs only by build stamp. Accept equals here and
    // let offer_equal_version make the real call: it refuses a feed stamp that
    // is missing or identical to this build, so nothing loops. Without this the
    // check resolved the right feed and then reported "up to date" anyway
    // because 0.4.0 is not newer than 0.4.0.
    builder = builder.version_comparator(|current, remote| remote.version >= current);
    if let Ok(raw) = std::env::var("QC_FEED_URL") {
        let raw = raw.trim();
        if !raw.is_empty() {
            let url = reqwest::Url::parse(raw).map_err(|e| format!("bad QC_FEED_URL: {e}"))?;
            builder = builder
                .endpoints(vec![url])
                .map_err(|e| format!("bad QC_FEED_URL: {e}"))?;
        }
    } else if channel == "experimental" {
        if let Some(raw) = experimental_feed_url().await {
            if let Ok(url) = reqwest::Url::parse(&raw) {
                builder = builder
                    .endpoints(vec![url])
                    .map_err(|e| format!("bad feed url: {e}"))?;
            }
        }
        // A lookup that fails leaves the stable endpoint in place: the
        // check still works, it just cannot see test builds this round.
    }
    builder
        .build()
        .map_err(|e| format!("updater unavailable: {e}"))
}

/// The newest published release that carries a compiled update feed,
/// pre-releases included, as the experimental channel's endpoint. Releases
/// without a latest.json asset (like the engine package) are skipped.
///
/// The list GitHub returns is NOT reliably newest-first (observed: older
/// test builds before newer ones), so the pick is by the release's own
/// created timestamp, the greatest winning. Taking the first entry that
/// merely looks valid resolved a stale feed and would have offered an
/// older build as an update.
async fn experimental_feed_url() -> Option<String> {
    const REPO: &str = "YousefMohiey/QuotaVPN";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("QuotaVPN")
        .build()
        .ok()?;
    let body = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=30"
        ))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let list: serde_json::Value = serde_json::from_str(&body).ok()?;
    let best = list
        .as_array()?
        .iter()
        .filter(|r| {
            r.get("draft").and_then(|v| v.as_bool()) != Some(true)
                && r.get("tag_name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|t| t.starts_with('v'))
                && r
                    .get("assets")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| {
                        a.iter()
                            .any(|x| x.get("name").and_then(|n| n.as_str()) == Some("latest.json"))
                    })
        })
        .max_by_key(|r| {
            // ISO 8601 UTC timestamps compare as strings, so the latest
            // release wins regardless of the order the API chose.
            r.get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        })?;
    let tag = best.get("tag_name")?.as_str()?;
    Some(format!(
        "https://github.com/{REPO}/releases/download/{tag}/latest.json"
    ))
}

/// The update channel the user picked: "normal" or "experimental". The
/// check and install paths read it back from the config on every call, so
/// a switch takes effect on the next check.
#[tauri::command]
fn set_update_channel(channel: String, state: tauri::State<State>) -> Result<(), String> {
    let ch = if channel == "experimental" { "experimental" } else { "normal" };
    let mut cfg = state.0.lock().unwrap();
    if cfg.update_channel != ch {
        cfg.update_channel = ch.to_string();
        cfg.save();
    }
    Ok(())
}

/// The check failure with the whole source chain spelled out: the plugin's
/// own Display stops at "error sending request", and the cause underneath is
/// the only part that says why.
fn check_failed(e: tauri_plugin_updater::Error) -> String {
    let mut msg = e.to_string();
    let mut src = std::error::Error::source(&e);
    while let Some(s) = src {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        src = s.source();
    }
    format!("check failed: {msg}")
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle, state: tauri::State<'_, State>) -> Result<UpdateInfo, String> {
    const REPO: &str = "YousefMohiey/QuotaVPN";
    let current = app.package_info().version.to_string();
    let channel = state.0.lock().unwrap().update_channel.clone();
    let found = updater(&app, &channel)
        .await?
        .check()
        .await
        .map_err(check_failed)?;
    match found {
        Some(u) => {
            // Only a higher version is an update. Every build of a version
            // line carries the same number, so accepting equals here turned
            // the test channel into a permanent "update ready" notice for a
            // build the user was already running.
            if !newer(&u.version, &current) {
                return Ok(up_to_date(&current));
            }
            Ok(UpdateInfo {
                available: true,
                url: format!("https://github.com/{REPO}/releases/tag/v{}", u.version),
                latest: u.version,
                // First line of the release notes, so the update notice can say
                // what the build actually is (a hotfix reads as one) instead of
                // only a version number.
                notes: u
                    .body
                    .as_deref()
                    .and_then(|b| b.lines().next())
                    .map(|l| {
                        // Keep the sidebar notice to one tidy line: cut long
                        // notes at a word boundary and trim trailing punctuation
                        // so it never ends on a dangling "and,".
                        let l = l.trim();
                        if l.chars().count() <= 44 {
                            return l.to_string();
                        }
                        let mut cut: String = l.chars().take(44).collect();
                        if let Some(i) = cut.rfind(' ') {
                            cut.truncate(i);
                        }
                        while cut.ends_with([',', '.', ';', ':', ' ']) {
                            cut.pop();
                        }
                        cut.push('…');
                        cut
                    })
                    .unwrap_or_default(),
                current,
            })
        }
        None => Ok(up_to_date(&current)),
    }
}

/// Install the update by itself: stop the tunnel (the installer needs
/// the TUN device free), download + verify + run the signed setup,
/// then restart into the new build. Progress goes to the UI as
/// `update-progress` events with a `pct` field.
#[tauri::command]
async fn apply_update(app: tauri::AppHandle, state: tauri::State<'_, State>) -> Result<String, String> {
    let current = app.package_info().version.to_string();
    let channel = state.0.lock().unwrap().update_channel.clone();
    let update = updater(&app, &channel)
        .await?
        .check()
        .await
        .map_err(check_failed)?
        .ok_or_else(|| "Already on the latest build".to_string())?;
    // Same guard as the check: only a higher version is an update, so refuse a
    // feed that repeats the version this build already carries before the
    // installer touches anything.
    if !newer(&update.version, &current) {
        return Err("This build is already current".to_string());
    }
    if UPDATE_BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("An update is already installing".to_string());
    }
    {
        let eng = app.state::<Engine>();
        stop_engine(&eng);
    }
    let prog = app.clone();
    let res = update
        .download_and_install(
            move |chunk, total| {
                let pct = total.map(|t| {
                    if t > 0 {
                        ((chunk as u64 * 100 / t).min(100)) as usize
                    } else {
                        0
                    }
                });
                let _ = prog.emit(
                    "update-progress",
                    serde_json::json!({"chunk": chunk, "total": total, "pct": pct}),
                );
            },
            || {},
        )
        .await;
    match res {
        Ok(()) => app.restart(),
        Err(e) => {
            UPDATE_BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
            Err(format!("install failed: {e}"))
        }
    }
}

static WEBVIEW_LOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// WebView2 memory: LOW while the window is hidden (tray or minimized) and
/// NORMAL when it is back on screen. WebView2 suspends the page and the
/// working set drops to a few MB; nothing is unloaded, so showing the window
/// again resumes it without a reload. Runs on a worker thread: with_webview
/// dispatches onto the main thread and must not be called from it.
fn set_webview_memory_low(app: &tauri::AppHandle, low: bool) {
    if WEBVIEW_LOW.swap(low, std::sync::atomic::Ordering::SeqCst) == low {
        return;
    }
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    std::thread::spawn(move || {
        // Give the hide/minimize a moment to reach the compositor before the
        // suspend request: WebView2 refuses to suspend while it still counts
        // as visible.
        if low {
            std::thread::sleep(std::time::Duration::from_millis(900));
        }
        let _ = w.with_webview(move |pw| {
            #[cfg(windows)]
            {
                use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
                use windows_core::Interface;
                let controller = pw.controller();
                let Ok(core) = (unsafe { controller.CoreWebView2() }) else {
                    return;
                };
                // WebView2 refuses to suspend while it counts as visible, so
                // the controller visibility follows the window state (the
                // documented pattern for a hidden or minimized app window).
                let _ = unsafe { controller.SetIsVisible(!low) };
                // Suspend while hidden: this is the one that actually releases
                // the renderer, and it drives the memory target level itself,
                // so the app must not also call SetMemoryUsageTargetLevel
                // (the WebView2 docs say to pick one of the two, not mix).
                // Resume restores the page as it was, no reload. Suspend can
                // refuse (page visible, media playing), nothing changes then.
                if let Ok(c3) = core.cast::<ICoreWebView2_3>() {
                    if low {
                        let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(
                            |_hr, _suspended| Ok(()),
                        ));
                        let _ = unsafe { c3.TrySuspend(&handler) };
                    } else {
                        let _ = unsafe { c3.Resume() };
                    }
                }
                // Showing again: visibility goes back on after the resume so
                // the page repaints immediately.
                if !low {
                    let _ = unsafe { controller.SetIsVisible(true) };
                }
            }
            #[cfg(not(windows))]
            let _ = &pw;
        });
    });
}

pub fn run() {
    // Headless self-test for the WE quota flow, and a diagnostic the owner
    // can run on any PC: QC_WE_NUMBER and QC_WE_PASS in the environment,
    // QuotaVPN.exe --we-probe. Prints units and PROBE_GREEN, exits nonzero
    // on any failure. Runs before the single-instance plugin on purpose.
    if std::env::args().any(|a| a == "--we-probe") {
        we_quota::probe_main();
        return;
    }
    // The elevated pass: fetch the engine and wintun if this machine has never
    // run them, then register the helper task and leave. This is the only
    // moment the product asks Windows for administrator, once per machine.
    if std::env::args().any(|a| a == "--setup-helper") {
        let _ = vpn::ensure_engine();
        match vpn::install_task() {
            Ok(()) => println!("helper installed"),
            Err(e) => eprintln!("helper failed: {e}"),
        }
        return;
    }
    tauri::Builder::default()
        // Single instance: a second launch brings the running app forward
        // instead of starting a twin. Must be the first plugin registered.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
            set_webview_memory_low(app, false);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        // The updater also sees a build re-released under the SAME version
        // number; check_update and apply_update then decide by the feed's
        // build stamp whether that build is actually new to this PC.
        .plugin(
            tauri_plugin_updater::Builder::new()
                .default_version_comparator(|current, remote| remote.version >= current)
                .build(),
        )
        .manage(State(Mutex::new(AppConfig::default())))
        .manage(Engine(Mutex::new(None)))
        .manage(we_quota::WeQuota::new())
        .setup(|app| {
            // Same OS config dir as the previous desktop app: cards carry
            // over, nothing to re-enter. (No chdir - that is phone-only.)
            let cfg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                initial_config,
            ))
            .unwrap_or_default();
            *app.state::<State>().0.lock().unwrap() = cfg;
            // Tray: the app lives here. Closing the window only hides it;
            // Quit from this menu is the real exit (engine stopped first).
            let show = MenuItem::with_id(app, "show", "Show QuotaVPN", true, None::<&str>)?;
            let check = MenuItem::with_id(app, "check", "Check for updates", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &check, &quit])?;
            let raw = image::load_from_memory_with_format(
                include_bytes!("../icons/32x32.png"),
                image::ImageFormat::Png,
            )
            .map_err(|e| format!("tray icon: {e}"))?
            .to_rgba8();
            let (w, h) = (raw.width(), raw.height());
            let icon = tauri::image::Image::new_owned(raw.into_raw(), w, h);
            let handle = app.handle().clone();
            TrayIconBuilder::with_id("main")
                .icon(icon)
                .tooltip("QuotaVPN")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                        set_webview_memory_low(tray.app_handle(), false);
                    }
                })
                .build(&handle)?;
            // Once per update, quietly re-register every local card on the
            // server so a wiped or rebuilt server heals on launch, not only
            // when the user connects.
            spawn_launch_refresh(app.handle().clone());
            // The Valorant watcher: an OS thread, never a webview timer.
            spawn_game_watch(app.handle().clone());
            // Window-state reconciler: an undecorated window no longer reports
            // a 0x0 size while minimized, and hide/show/minimize events arrive
            // in orders that are easy to get wrong, so poll the real state a
            // couple of times a second and let the flag in set_webview_memory_low
            // swallow the no-op calls. Cheap (two state reads per tick) and it
            // cannot miss a transition.
            //
            // Keep this INSIDE the setup above: Tauri's App holds a single
            // setup slot (App.setup: Option<SetupHook>), so a second .setup()
            // call silently replaces the first and the app loses its config
            // load and its tray - which is exactly what "Set up your server
            // first." looked like on a machine that was already configured.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(1200));
                let Some(w) = handle.get_webview_window("main") else {
                    continue;
                };
                let hidden = !w.is_visible().unwrap_or(true);
                let low = hidden || w.is_minimized().unwrap_or(false);
                set_webview_memory_low(&handle, low);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // X hides to the tray instead of quitting; the tunnel keeps
            // running and the icon stays until Quit is picked there.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
                set_webview_memory_low(window.app_handle(), true);
            }

            // Safety net: any focus on a normal, visible window means the
            // user is looking at it again.
            if let WindowEvent::Focused(true) = event {
                if window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false) {
                    set_webview_memory_low(window.app_handle(), false);
                }
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                set_webview_memory_low(app, false);
            }
            "check" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                set_webview_memory_low(app, false);
                // Resuming the webview is asynchronous; give it a beat so the
                // update check lands in a running page instead of a suspended one.
                let handle = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1200));
                    let _ = handle.emit("tray-check-updates", ());
                });
            }
            "quit" => {
                let eng = app.state::<Engine>();
                stop_engine(&eng);
                app.exit(0);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            setup_helper,
            get_state,
            probe_server,
            generate_card,
            import_card,
            revoke_card,
            copy_card,
            set_card_sni,
            tunnel_start,
            tunnel_stop,
            tunnel_status,
            tunnel_health,
            tunnel_traffic,
            tunnel_log,
            tunnel_copy_log,
            tunnel_probe,
            tunnel_apps,
            resolve_host,
            check_update,
            apply_update,
            set_update_channel,
            net_info,
            app_icons,
            speed_servers,
            speed_latency,
            speed_reach,
            speed_down,
            speed_up,
            speedtest_cli_ready,
            speedtest_cli,
            process_running,
            autostart_get,
            autostart_set,
            we_quota::we_quota_challenge,
            we_quota::we_quota_login,
            we_quota::we_quota_usage,
            we_quota::we_quota_logout
        ])
        .run(tauri::generate_context!())
        .expect("QuotaVPN failed to start");
}

#[cfg(all(test, windows))]
mod tests {
    use windows::Win32::Foundation::NO_ERROR;
    use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};

    /// The traffic chart leans on this API; prove the table reads here.
    #[test]
    fn if_table_is_readable() {
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            assert_eq!(GetIfTable2(&mut table), NO_ERROR);
            let rows = std::slice::from_raw_parts(
                (*table).Table.as_ptr(),
                (*table).NumEntries as usize,
            );
            assert!(!rows.is_empty());
            for row in rows.iter().take(8) {
                let alias = String::from_utf16_lossy(&row.Alias);
                println!(
                    "{} in={} out={}",
                    alias.trim_end_matches('\0'),
                    row.InOctets,
                    row.OutOctets
                );
            }
            FreeMibTable(table as *const core::ffi::c_void);
        }
    }

    #[test]
    fn tun_octets_never_panics() {
        let (rx, tx) = super::tun_octets();
        println!("tun_octets -> {rx}/{tx}");
    }

    /// Start-with-Windows round trip through the registry API: write a test
    /// path, read it back, remove it, then restore whatever the machine had.
    #[test]
    fn autostart_round_trip() {
        let before = super::autostart_reg::read();
        super::autostart_reg::write("C:\\test\\quotavpn-autostart-probe.exe").unwrap();
        assert_eq!(
            super::autostart_reg::read().as_deref(),
            Some("C:\\test\\quotavpn-autostart-probe.exe")
        );
        super::autostart_reg::remove().unwrap();
        assert_eq!(super::autostart_reg::read(), None);
        if let Some(prev) = before {
            super::autostart_reg::write(&prev).unwrap();
            assert_eq!(super::autostart_reg::read().as_deref(), Some(prev.as_str()));
        }
    }

    /// The launch watcher leans on this: a name nobody runs is false, and
    /// Explorer (always present on a real desktop) is true.
    #[test]
    fn process_running_answers() {
        assert!(!super::process_running("quotavpn-definitely-not-a-process-xyz.exe".into()));
        assert!(!super::process_running(String::new()));
        assert!(super::process_running("explorer.exe".into()));
        assert!(super::process_running("EXPLORER.EXE".into()));
    }

    /// Same-version re-release rule: covered by unit tests in the shared
    /// crate (`quotacards::updater`), because this crate's lib test exe
    /// cannot start on this toolchain (no Common-Controls 6 manifest on
    /// test targets, comctl32 v6 import in the updater code).

    /// build.rs must always embed a stamp: a short hash or the "dev"
    /// fallback, never an empty string.
    #[test]
    fn build_stamp_is_embedded() {
        let stamp = super::BUILD_STAMP;
        assert!(
            stamp == "dev" || (!stamp.is_empty() && stamp.chars().all(|c| c.is_ascii_hexdigit())),
            "unexpected QC_BUILD value: {stamp:?}"
        );
        println!("QC_BUILD = {stamp}");
    }
}
