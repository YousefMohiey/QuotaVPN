//! QuotaVPN desktop backend (Tauri + WebView).
//!
//! Same UI and server core as the phone app. The tunnel here is the
//! built-in sing-box + wintun engine (whole-PC TUN), driven by the
//! shared `quotacards::vpn` module - no plugin, no mobile service.
//! Config lives in the OS config dir, so cards carry over from the
//! previous desktop app untouched.

use quotacards::{
    config::{AppConfig, Card, DEFAULT_HOST},
    vpn,
};
use std::sync::Mutex;
#[cfg(not(windows))]
mod wincompat;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Emitter;
use tauri::Manager;
use tauri::WindowEvent;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_updater::UpdaterExt;
#[cfg(not(windows))]
use wincompat::CmdExt;

mod cardapi;
mod credentials;
pub mod ookla;
pub mod router_huawei;
mod router_remember;
mod spaces;
#[cfg(windows)]
mod spaces_direct;
pub mod speed;
#[cfg(windows)]
mod wintun;
mod we_quota;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

struct State(Mutex<AppConfig>);
/// Live engine PID. None means no tunnel (or one we no longer track).
struct Engine(Mutex<Option<u32>>);
/// Serialises start and stop. Without this, a stop that was already sweeping
/// routes could land halfway through the next start and kill its engine, and
/// pressing Connect while the dial was still sliding back connected into a
/// tunnel that immediately came down.
struct Lifecycle(tokio::sync::Mutex<()>);

/// What this session's tunnel is carrying, per (process, host). In memory only:
/// no table, no file, nothing that survives the process.
struct Dests(Mutex<vpn::DestTracker>);

/// How often the engine's connection list is read. Two seconds is slow enough
/// to cost nothing and fast enough that a screen opened at random is current.
const DEST_POLL: std::time::Duration = std::time::Duration::from_secs(2);

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
    cards: Vec<Card>,
    version: String,
    update_channel: String,
}

/// Record which space the live session really carries. Only called after a
/// connect has proved the engine is routing, so a failed start cannot claim a
/// space the user cannot use.
fn note_tunnel_space(app: &tauri::AppHandle, applied: bool) {
    let state = app.state::<State>();
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    let next = if applied {
        cfg.active_space.clone()
    } else {
        String::new()
    };
    if cfg.tunnel_space != next {
        cfg.tunnel_space = next;
        cfg.save();
    }
}

fn snapshot(cfg: &AppConfig) -> UiState {
    // The window never needs a WireGuard private key: the backend reads it
    // from config.json when it builds the engine. Keep it out of the webview.
    let cards = cfg
        .cards
        .iter()
        .cloned()
        .map(|mut card| {
            card.wg_private.clear();
            card
        })
        .collect();
    UiState {
        server_ip: cfg.server_ip.clone(),
        cards,
        version: String::new(),
        update_channel: cfg.update_channel.clone(),
    }
}

fn build_link(uuid: &str, host: &str, sni: &str, name: &str) -> String {
    format!("vless://{uuid}@{host}:443?type=tcp&encryption=none&security=tls&fp=random&alpn=h3%2Ch2%2Chttp%2F1.1&allowInsecure=1&sni={sni}#{name}")
}

fn initial_config() -> AppConfig {
    AppConfig::migrate_dirs();
    let mut cfg = AppConfig::load();
    let mut dirty = false;
    // The desktop has no SSH control plane any more. Clear any key an older
    // build left in config.json so the shared credential does not sit in the
    // profile, and so a stale key cannot make the app look set up.
    if !cfg.private_key.is_empty() {
        cfg.private_key.clear();
        dirty = true;
    }
    if !cfg.public_key.is_empty() {
        cfg.public_key.clear();
        dirty = true;
    }
    if !cfg.ssh_user.is_empty() {
        cfg.ssh_user.clear();
        dirty = true;
    }
    if cfg.ssh_port != 0 {
        cfg.ssh_port = 0;
        dirty = true;
    }
    // The DuckDNS name moved from quotacards to quotavpn. The old one no
    // longer resolves, so an install that kept it would report "the server
    // name did not resolve" forever. Heal the old default in place.
    if cfg.server_ip.is_empty() || cfg.server_ip == "quotacards.duckdns.org" {
        cfg.server_ip = DEFAULT_HOST.to_string();
        dirty = true;
    }
    // migrate old cards (no per-card SNI) to a sane default
    for c in &mut cfg.cards {
        if c.sni.is_empty() {
            c.sni = "ea.com".to_string();
            dirty = true;
        }
    }
    // Owner delivery path: a profile beside the exe or in the config folder
    // imports the customer's card silently, so the customer never pastes.
    match credentials::import_profile(&mut cfg) {
        Ok(count) if count > 0 => {
            vpn::app_log(&format!("profile: imported {count} card(s)"));
            dirty = true;
        }
        Ok(_) => {}
        Err(e) => vpn::app_log(&format!("profile: {e}")),
    }
    if dirty {
        cfg.save();
    }
    credentials::remove_legacy_device_secret();
    cfg
}

/// Headless timing for the real connect path. Uses the real card, server,
/// engine and TUN, then stops the engine. Run once cold and twice warm.
fn bench_connect(voice: bool) -> Result<(), String> {
    AppConfig::migrate_dirs();
    let cfg = AppConfig::load();
    let card = cfg.cards.first().ok_or("no card on this PC")?;
    if cfg.server_ip.is_empty() {
        return Err("no server configured".into());
    }
    if vpn::task_engine_pid().is_some() {
        return Err("an engine is already running; disconnect first".into());
    }
    let started = std::time::Instant::now();
    let _engine = vpn::ensure_engine()?;
    let ensure = started.elapsed();
    let t = std::time::Instant::now();
    vpn::write_tun_config(
        &card.uuid,
        &cfg.server_ip,
        &card.sni,
        "all",
        &[],
        voice,
        "vless",
        "",
        None,
    )?;
    let write = t.elapsed();
    let t = std::time::Instant::now();
    vpn::check_config()?;
    let check = t.elapsed();
    let t = std::time::Instant::now();
    let pid = vpn::start_engine()?;
    let start = t.elapsed();
    let t = std::time::Instant::now();
    let mut up = false;
    for _ in 0..200 {
        if vpn::tun_routes_present() {
            up = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let routes = t.elapsed();
    let total = started.elapsed();
    let t = std::time::Instant::now();
    vpn::stop_engine();
    let stop = t.elapsed();
    println!(
        "BENCH voice={voice} ensure={}ms write={}ms check={}ms start={}ms routes={}ms total={}ms stop={}ms up={up} pid={pid}",
        ensure.as_millis(),
        write.as_millis(),
        check.as_millis(),
        start.as_millis(),
        routes.as_millis(),
        total.as_millis(),
        stop.as_millis()
    );
    Ok(())
}

#[tauri::command]
fn get_state(app: tauri::AppHandle, state: tauri::State<State>) -> UiState {
    let mut st = snapshot(&state.0.lock().unwrap());
    // The running build's own version: the sidebar shows it without waiting
    // for a network round trip to GitHub.
    st.version = app.package_info().version.to_string();
    st
}

#[derive(serde::Serialize)]
struct CardStatus {
    imported: bool,
    uuid: String,
    ok: bool,
    state: String,
    registered: bool,
    message: String,
}

fn pick_card<'a>(cfg: &'a AppConfig, uuid: &str) -> Option<&'a Card> {
    if !uuid.trim().is_empty() {
        return cfg.cards.iter().find(|card| card.uuid == uuid);
    }
    cfg.cards
        .iter()
        .find(|card| credentials::has(&card.uuid))
        .or_else(|| cfg.cards.first())
}

/// Read-only state of one card, scoped to its own credential. No mutation, so
/// polling this cannot restart Xray.
#[tauri::command]
async fn card_status(
    state: tauri::State<'_, State>,
    uuid: Option<String>,
) -> Result<CardStatus, String> {
    let card_uuid = {
        let cfg = state.0.lock().unwrap();
        pick_card(&cfg, uuid.as_deref().unwrap_or_default()).map(|card| card.uuid.clone())
    };
    let Some(uuid) = card_uuid else {
        return Ok(CardStatus {
            imported: false,
            uuid: String::new(),
            ok: false,
            state: "none".into(),
            registered: false,
            message: "No card is provisioned on this PC yet. Contact your provider.".into(),
        });
    };
    if !credentials::has(&uuid) {
        return Ok(CardStatus {
            imported: false,
            uuid,
            ok: false,
            state: "unknown".into(),
            registered: false,
            message: "No management credential is imported for this card.".into(),
        });
    }
    match cardapi::call_with_timeout("status", &uuid, 8).await {
        Ok(payload) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
            let registered = parsed
                .get("registered")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            {
                let mut cfg = state.0.lock().unwrap();
                if let Some(card) = cfg.cards.iter_mut().find(|card| card.uuid == uuid) {
                    card.registered = registered;
                    cfg.save();
                }
            }
            Ok(CardStatus {
                imported: true,
                uuid,
                ok: true,
                state: parsed
                    .get("state")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                registered,
                message: payload,
            })
        }
        Err(e) => Ok(CardStatus {
            imported: true,
            uuid,
            ok: false,
            state: "unknown".into(),
            registered: false,
            message: e,
        }),
    }
}

#[derive(serde::Serialize)]
struct ImportResult {
    ok: bool,
    msg: String,
    uuid: String,
}

/// Store the server-issued secret and place the card in config.json. This is
/// local and fast: the status check is a separate, read-only action, and the
/// tunnel failure path registers a card the server has never seen.
#[tauri::command]
async fn import_credential(
    state: tauri::State<'_, State>,
    text: String,
    kind: Option<String>,
) -> Result<ImportResult, String> {
    let uuid = credentials::import_into_config(&text, kind.as_deref())?;
    {
        let mut cfg = state.0.lock().unwrap();
        *cfg = AppConfig::load();
    }
    // The credential is local and must land even when the network is down.
    // Settings has a read-only status check, and the tunnel failure path
    // registers a card the server has never seen.
    Ok(ImportResult {
        ok: true,
        msg: "Credential imported.".into(),
        uuid,
    })
}

/// Remove the card from this PC only. The server entry stays until the owner
/// revokes it with qv-provision, which is the deliberate two-step the card
/// model needs.
#[tauri::command]
async fn forget_card(state: tauri::State<'_, State>, uuid: String) -> Result<CmdResult, String> {
    credentials::forget(&uuid)?;
    {
        let mut cfg = state.0.lock().unwrap();
        cfg.cards.retain(|card| card.uuid != uuid);
        cfg.save();
    }
    Ok(CmdResult {
        ok: true,
        msg: "Card removed from this PC. The server still has it until it is revoked.".into(),
    })
}

/// The executables that mean the game is up. The shipping build is the first
/// name; some installs and the game's own launcher expose the plain one, and
/// watching only the shipping name missed those, which read as "it does not
/// notice the game".
#[cfg(windows)]
const GAME_EXES: [&str; 2] = ["VALORANT-Win64-Shipping.exe", "VALORANT.exe"];

/// How many extra ticks the transition is repeated for after it happens. The
/// window spends its life in the tray, so its webview is suspended, and
/// resuming it is asynchronous: an event sent a moment before the page is
/// running again is delivered to nothing and lost. The page's own fallback
/// timer cannot cover this either, because a suspended webview runs no timers.
/// Six ticks at one second covers a resume and then stops; the faster poll is
/// what gets the voice helper armed while the game is still loading.
#[cfg(windows)]
const GAME_RESEND_TICKS: u8 = 6;

#[cfg(windows)]
fn game_is_running() -> bool {
    GAME_EXES.iter().any(|n| process_running((*n).to_string()))
}

/// One call for both Valorant executable names. The webview fallback used to
/// check only the shipping name, so an install that exposed `VALORANT.exe`
/// was never noticed.
#[tauri::command]
fn game_running() -> bool {
    #[cfg(windows)]
    {
        game_is_running()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
fn emit_game_state(app: &tauri::AppHandle, running: bool) {
    let _ = app.emit("game-state", serde_json::json!({ "running": running }));
}

/// Watches for the game while the app runs and emits `game-state` on every
/// transition. This lives here, not in the webview: the window spends most
/// of its life minimized or hidden in the tray, and a hidden webview has its
/// JS timers throttled to a crawl, so a JS interval only ever fires while
/// the user is looking at the app - which read as "the voice helper only
/// works from the Valorant page". A plain OS thread cannot be throttled.
///
/// The transition is repeated for a few ticks rather than sent once. The
/// single send was the auto-connect bug: it went out the moment the game
/// appeared, before the asynchronous webview resume had landed, so the page
/// never saw it and starting the game armed nothing.
#[cfg(windows)]
fn spawn_game_watch(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut up = false;
        let mut resend: u8 = 0;
        loop {
            let now = game_is_running();
            if now != up {
                up = now;
                resend = GAME_RESEND_TICKS;
                vpn::app_log(&format!("game: running={now}"));
                wake_for_game(&app);
                emit_game_state(&app, now);
            } else if resend > 0 {
                resend -= 1;
                wake_for_game(&app);
                emit_game_state(&app, up);
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

/// How long the page is kept awake after the game opens or closes. A voice
/// start can wait on a cold engine, so this covers the whole run plus the
/// retry that follows a slow first report.
const GAME_WAKE_SECS: u64 = 90;

/// True while a re-suspend timer is already pending. The watcher wakes the
/// page on every repeat tick now, and one timer per wake would stack a
/// suspend request for each.
static WAKE_ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Resume a hidden or minimized window's webview just long enough for the
/// game event to be acted on, then suspend it again if it is still hidden.
fn wake_for_game(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    let hidden = !w.is_visible().unwrap_or(true) || w.is_minimized().unwrap_or(false);
    if !hidden {
        return;
    }
    set_webview_memory_low(app, false);
    if WAKE_ARMED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(GAME_WAKE_SECS));
        WAKE_ARMED.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(w) = handle.get_webview_window("main") {
            let hidden = !w.is_visible().unwrap_or(true) || w.is_minimized().unwrap_or(false);
            if hidden {
                set_webview_memory_low(&handle, true);
            }
        }
    });
}

#[tauri::command]
async fn copy_card(
    state: tauri::State<'_, State>,
    app: tauri::AppHandle,
    uuid: String,
) -> Result<CmdResult, String> {
    let cfg = state.0.lock().unwrap();
    match cfg.cards.iter().find(|c| c.uuid == uuid) {
        Some(c) => {
            let _ =
                app.clipboard()
                    .write_text(build_link(&c.uuid, &cfg.server_ip, &c.sni, &c.name));
            Ok(CmdResult {
                ok: true,
                msg: "Link copied".into(),
            })
        }
        None => Ok(CmdResult {
            ok: false,
            msg: "Card not found".into(),
        }),
    }
}

/// Change which domain (SNI) a card rides. The card id stays the same, so the
/// server side needs nothing; the value is read on the next connect.
#[tauri::command]
async fn set_card_sni(
    state: tauri::State<'_, State>,
    uuid: String,
    sni: String,
) -> Result<CmdResult, String> {
    let sni = sni.trim().to_string();
    if sni.is_empty() {
        return Ok(CmdResult {
            ok: false,
            msg: "Pick a domain first".into(),
        });
    }
    let mut cfg = state.0.lock().unwrap();
    let found = cfg.cards.iter_mut().find(|c| c.uuid == uuid);
    match found {
        Some(c) => {
            c.sni = sni;
            cfg.save();
            Ok(CmdResult {
                ok: true,
                msg: "Domain updated".into(),
            })
        }
        None => Ok(CmdResult {
            ok: false,
            msg: "Card not found".into(),
        }),
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
#[cfg(windows)]
fn process_running(name: String) -> bool {
    use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
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
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
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
        let st = unsafe { RegSetValueExW(hkey, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&data)) };
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
    #[cfg(windows)]
    {
        Ok(autostart_reg::read().as_deref() == Some(exe.display().to_string().as_str()))
    }
    #[cfg(not(windows))]
    {
        let entry = desktop_entry_path();
        Ok(std::fs::read_to_string(entry)
            .map(|t| t.contains(&exe.display().to_string()))
            .unwrap_or(false))
    }
}

#[tauri::command]
fn autostart_set(on: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        if on {
            autostart_reg::write(&exe.display().to_string())
        } else {
            autostart_reg::remove().or_else(|e| {
                if autostart_reg::read().is_none() {
                    Ok(())
                } else {
                    Err(e)
                }
            })
        }
    }
    #[cfg(not(windows))]
    {
        let entry = desktop_entry_path();
        if on {
            if let Some(dir) = entry.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let body = format!(
                "[Desktop Entry]
Type=Application
Name=QuotaVPN
Exec={}
X-GNOME-Autostart-enabled=true
",
                exe.display()
            );
            std::fs::write(&entry, body).map_err(|e| e.to_string())
        } else {
            match std::fs::remove_file(&entry) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

/// Where a desktop session looks for things to start with it.
#[cfg(not(windows))]
fn desktop_entry_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("quotavpn.desktop")
}

#[tauri::command]
async fn tunnel_start(
    app: tauri::AppHandle,
    _state: tauri::State<'_, State>,
    engine: tauri::State<'_, Engine>,
    lifecycle: tauri::State<'_, Lifecycle>,
    uuid: String,
    apps_mode: Option<String>,
    apps: Option<Vec<String>>,
    transport: Option<String>,
    voice: Option<bool>,
) -> Result<CmdResult, String> {
    // Wait for any stop that is still sweeping, so this start owns the
    // engine from spawn to routes instead of racing a teardown.
    let _gate = lifecycle.0.lock().await;
    let connect_started = std::time::Instant::now();
    let (host, card, space) = {
        let state = app.state::<State>();
        let cfg = state.0.lock().unwrap();
        let Some(card) = cfg.cards.iter().find(|c| c.uuid == uuid).cloned() else {
            return Ok(CmdResult {
                ok: false,
                msg: "Card not found".into(),
            });
        };
        // One active Space rides this session. A damaged record is ignored
        // rather than allowed to break the card's connect.
        let space = cfg
            .spaces
            .iter()
            .find(|s| s.id == cfg.active_space)
            .and_then(|s| s.tunnel());
        (cfg.server_ip.clone(), card, space)
    };
    if host.is_empty() {
        return Ok(CmdResult {
            ok: false,
            msg: "Set up your server first".into(),
        });
    }
    let t = match transport.as_deref().unwrap_or("vless") {
        "hy2" => "hy2",
        "wg" => "wg",
        _ => "vless",
    };
    // The transport profiles are provisioned once by the owner and cached in
    // config.json. The desktop no longer holds an SSH key with which to fetch
    // or mint them. A card that has never had them is VLESS only.
    let mut hy2_pass = String::new();
    let mut wg_tuple: Option<(String, String, String, String)> = None;
    if t == "hy2" {
        hy2_pass = app.state::<State>().0.lock().unwrap().hy2_password.clone();
        if hy2_pass.is_empty() {
            return Ok(CmdResult {
                ok: false,
                msg: "This card has no Hysteria2 profile on this PC. Connect with Standard, or ask for a new card.".into(),
            });
        }
    } else if t == "wg" {
        let (awg_json, srvpub) = {
            let state = app.state::<State>();
            let st = state.0.lock().unwrap();
            (st.awg_params.clone(), st.wg_server_pub.clone())
        };
        if awg_json.is_empty()
            || serde_json::from_str::<serde_json::Value>(&awg_json).is_err()
            || srvpub.is_empty()
            || card.wg_private.is_empty()
            || card.wg_addr.is_empty()
        {
            return Ok(CmdResult {
                ok: false,
                msg: "This card has no WireGuard profile on this PC. Connect with Standard, or ask for a new card.".into(),
            });
        }
        wg_tuple = Some((
            card.wg_private.clone(),
            card.wg_addr.clone(),
            srvpub,
            awg_json,
        ));
    }
    // The tunnel runs through a helper task registered once at setup, so this
    // process never needs to be elevated. Without that task the UI offers the
    // one-time setup instead of telling people to run the app as admin.
    if !vpn::task_installed() {
        return Ok(CmdResult {
            ok: false,
            msg: "setup-needed".into(),
        });
    }
    // The management credential is the control-plane identity. If the server
    // does not have this UUID in Xray yet, add it once before the engine dials.
    // Status is read-only; only a real gap triggers register, which is the one
    // call that restarts Xray. A control-plane outage never blocks the tunnel.
    let control_started = std::time::Instant::now();
    if credentials::has(&card.uuid) && !card.registered {
        let mut confirmed = false;
        if let Ok(payload) = cardapi::call_with_timeout("status", &card.uuid, 6).await {
            let registered = serde_json::from_str::<serde_json::Value>(&payload)
                .ok()
                .and_then(|value| value.get("registered").and_then(|v| v.as_bool()))
                .unwrap_or(true);
            if registered {
                confirmed = true;
            } else {
                vpn::app_log("connect: card is not in Xray, registering it once");
                if cardapi::call("register", &card.uuid).await.is_ok() {
                    confirmed = true;
                }
            }
        }
        if confirmed {
            let state = app.state::<State>();
            let mut cfg = state.0.lock().unwrap();
            if let Some(live) = cfg.cards.iter_mut().find(|c| c.uuid == card.uuid) {
                live.registered = true;
                cfg.save();
            }
        }
    }
    vpn::app_log(&format!(
        "connect: control {}ms (cached={})",
        control_started.elapsed().as_millis(),
        card.registered
    ));
    let voice_on = voice.unwrap_or(false);
    // The session keeps whatever normal configuration the app was using and
    // the voice rules are added inside it, so the normal VPN and the voice
    // helper run in the same tunnel at once.
    let mode = apps_mode.unwrap_or_default();
    let list = apps.unwrap_or_default();
    let ensure_started = std::time::Instant::now();
    vpn::ensure_engine().map_err(|e| e)?;
    vpn::app_log(&format!(
        "connect: ensure {}ms",
        ensure_started.elapsed().as_millis()
    ));
    let wg_ref = wg_tuple
        .as_ref()
        .map(|(p, a, s, j)| (p.as_str(), a.as_str(), s.as_str(), j.as_str()));
    // The config is rebuilt on every press, but it is usually byte-identical to
    // the one a live engine is already running. Restarting in that case costs a
    // full engine stop, driver handover and route settle for nothing, which is
    // most of the wait on a second connect.
    let write_started = std::time::Instant::now();
    let before = vpn::tun_config_digest();
    vpn::write_tun_config_with_space(
        &card.uuid,
        &host,
        &card.sni,
        &mode,
        &list,
        voice_on,
        t,
        &hy2_pass,
        wg_ref,
        space.as_ref(),
    )
    .map_err(|e| e)?;
    vpn::app_log(&format!(
        "connect: write {}ms",
        write_started.elapsed().as_millis()
    ));
    let unchanged = before.is_some() && before == vpn::tun_config_digest();
    let live = engine
        .0
        .lock()
        .ok()
        .and_then(|g| *g)
        .filter(|p| *p != 0 && pid_alive(*p));
    if unchanged && live.is_some() && vpn::tun_routes_present() {
        vpn::app_log("connect: config unchanged and the tunnel is already carrying it");
        note_tunnel_space(&app, true);
        return Ok(CmdResult {
            ok: true,
            msg: format!("VPN on - {} carries the traffic", card.name),
        });
    }
    let check_started = std::time::Instant::now();
    vpn::check_config().map_err(|e| e)?;
    vpn::app_log(&format!(
        "connect: check {}ms",
        check_started.elapsed().as_millis()
    ));
    // Only stop what is actually running. The old unconditional call also
    // swept routes and the adapter when there was no engine, which cost more
    // than the engine start on a fresh connect. start_engine still handles a
    // stray engine that the app does not track.
    if live.is_some() {
        stop_engine(&engine);
    }
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
        card.name,
        mode_label,
        list.len()
    ));
    // Fresh DNS for the new session: entries from before the tunnel existed
    // are what make the first lookups after connect hang.
    flush_dns_cache();
    // A dead engine can leave a ghost adapter behind, but every session picks
    // a fresh adapter name in start_engine, so the ghost does not block this
    // one. Resetting here cost about two seconds on every connect and often
    // could not remove the ghost anyway (it is hidden from Get-NetAdapter).
    // The engine-failure path still resets when a start actually dies.
    // A session-scoped number must not carry the previous tunnel's bytes.
    if let Ok(mut t) = app.state::<Dests>().0.lock() {
        t.clear();
    }
    let engine_started = std::time::Instant::now();
    let mut pid = vpn::start_engine().map_err(|e| e)?;
    vpn::app_log(&format!(
        "connect: engine start {}ms",
        engine_started.elapsed().as_millis()
    ));
    *engine.0.lock().unwrap() = Some(pid);
    // An explicit connect takes the engine over: releasing a routed device later
    // must not stop a tunnel the user asked for.
    #[cfg(windows)]
    quotacards::lan_route::note_engine_adopted();
    // Wait for the tun to actually carry traffic. A cold wintun adapter can
    // take several seconds to publish its routes, and a fixed short sleep
    // made the first connect of a session fail while the engine was healthy;
    // pressing again only worked because the driver was warm by then.
    let mut up = false;
    let routes_started = std::time::Instant::now();
    for attempt in 0..2 {
        for _ in 0..40 {
            if *engine.0.lock().unwrap() != Some(pid) {
                // A stop during settle clears the pid: drop our engine, report stopped.
                vpn::request_graceful_stop(pid);
                return Ok(CmdResult {
                    ok: false,
                    msg: "Stopped".into(),
                });
            }
            // Native check: the old tasklist spawn cost more than the route
            // poll it guarded.
            if pid != 0 && !vpn::engine_alive(pid) {
                break;
            }
            if vpn::tun_routes_present() {
                up = true;
                vpn::app_log(&format!(
                    "connect: routes {}ms",
                    routes_started.elapsed().as_millis()
                ));
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        }
        if up || (pid != 0 && vpn::engine_alive(pid)) || attempt == 1 {
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
    // One destination poller per session, following the pid this call settled
    // on. It exits by itself when the state stops owning that pid, so a stop, a
    // reconnect or a crash needs no bookkeeping here.
    if let Some(live) = engine.0.lock().ok().and_then(|g| *g) {
        spawn_dest_poll(app.clone(), live);
    }
    if up {
        // Clear whatever the warmup window cached, failed lookups included,
        // so the checks right after connect get fresh answers.
        flush_dns_cache();
        vpn::app_log(&format!(
            "connect: ready in {}ms",
            connect_started.elapsed().as_millis()
        ));
        note_tunnel_space(&app, true);
        Ok(CmdResult {
            ok: true,
            msg: format!("VPN on - {} carries the traffic", card.name),
        })
    } else if pid_alive(pid) {
        // Alive but not routing yet: keep it running and let the status poll
        // adopt it once the routes land, instead of killing a healthy engine.
        // Where did it break? Only a server that answers and still carries
        // nothing is a registration problem; a name that will not resolve, a
        // blocked port or a broken handshake must never trigger a repair.
        // The data plane is VLESS on TCP 443. The old call passed the SSH
        // port here, so "reachable" described a control port the tunnel never
        // used. This is the port the engine actually dials.
        let stage = vpn::stage_of(&host, 443, &vpn::engine_log_tail());
        if stage == vpn::Stage::Reachable {
            if !credentials::has(&card.uuid) {
                return Ok(CmdResult {
                    ok: false,
                    msg: "The tunnel came up but carried nothing. This card is not provisioned for management on this PC; contact your provider.".into(),
                });
            }
            let repaired = cardapi::call("register", &card.uuid).await;
            let tail = match repaired {
                Ok(_) => {
                    " The server did not know this card; it is registered now, press Connect again."
                }
                Err(_) => {
                    " The server did not know this card and refused the repair; support is needed."
                }
            };
            return Ok(CmdResult {
                ok: false,
                msg: format!("The tunnel came up but carried nothing.{tail}"),
            });
        }
        vpn::clear_server_ip_cache();
        Ok(CmdResult {
            ok: false,
            msg: vpn::stage_words(stage).to_string(),
        })
    } else {
        // A task start can show up a moment after our wait window: adopt what
        // is actually running instead of reporting a failure over a tunnel
        // that is already carrying traffic. Never kill a live engine here -
        // the poll will bring the UI in line either way.
        if let Some(live) = vpn::task_engine_pid() {
            *engine.0.lock().unwrap() = Some(live);
            spawn_dest_poll(app.clone(), live);
            if vpn::tun_routes_present() {
                flush_dns_cache();
                note_tunnel_space(&app, true);
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
        // Same rule on the hard failure: repair only when the server itself
        // answers. The engine's log stays out of the message, it can carry the
        // card id and the server name.
        // The data plane is VLESS on TCP 443. The old call passed the SSH
        // port here, so "reachable" described a control port the tunnel never
        // used. This is the port the engine actually dials.
        let stage = vpn::stage_of(&host, 443, &vpn::engine_log_tail());
        if stage == vpn::Stage::Reachable {
            if !credentials::has(&card.uuid) {
                return Ok(CmdResult {
                    ok: false,
                    msg: "The tunnel did not come up. This card is not provisioned for management on this PC; contact your provider.".into(),
                });
            }
            let repaired = cardapi::call("register", &card.uuid).await;
            let tail = match repaired {
                Ok(_) => " The card was re-registered; press Connect again.",
                Err(_) => " The server refused to register this card.",
            };
            return Ok(CmdResult {
                ok: false,
                msg: format!("The tunnel did not come up.{tail}"),
            });
        }
        vpn::clear_server_ip_cache();
        Ok(CmdResult {
            ok: false,
            msg: format!("The tunnel did not come up. {}", vpn::stage_words(stage)),
        })
    }
}

/// The one-time, privileged setup. Windows asks for administrator here and
/// only here: the elevated pass registers the helper task, and every later
/// connect runs through it without any prompt at all.
#[tauri::command]
async fn setup_helper() -> Result<CmdResult, String> {
    vpn::app_log("setup: asked for the one-time setup");
    if vpn::task_installed() && vpn::lan_task_installed() {
        return Ok(CmdResult {
            ok: true,
            msg: "ready".into(),
        });
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let ps = format!(
        "Start-Process -FilePath '{}' -ArgumentList '--setup-helper','--as','{}' -Verb RunAs -WindowStyle Hidden",
        exe.display(),
        vpn::account_name()
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
        return Ok(CmdResult {
            ok: false,
            msg: "The tunnel setup did not run".into(),
        });
    }
    if cancelled {
        return Ok(CmdResult {
            ok: false,
            msg: "Windows did not allow the setup. Approve the prompt to finish it".into(),
        });
    }
    // The first setup also downloads the engine and the driver, which on a
    // slow line takes minutes. Giving up after twenty seconds told people the
    // setup had failed while it was still running.
    for _ in 0..360 {
        if vpn::task_installed() {
            // The tunnel is what the setup was for. The network pass is the
            // second one; when it did not land, its features fall back to the
            // prompt rather than blocking a tunnel that is ready.
            if !vpn::lan_task_installed() {
                vpn::app_log("setup: network helper missing, LAN features will prompt once");
            }
            return Ok(CmdResult {
                ok: true,
                msg: "ready".into(),
            });
        }
        // The privileged pass says what went wrong as soon as it knows. Waiting
        // three minutes to report a failure that happened in one second is how
        // "setting up the tunnel" turned into a hang.
        let said = vpn::setup_log_tail();
        if said.contains("task failed") || said.contains("engine failed") {
            return Ok(CmdResult {
                ok: false,
                msg: format!("The tunnel setup did not finish: {said}"),
            });
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    // Nothing appeared: say what the privileged pass wrote, not a guess.
    let detail = vpn::setup_log_tail();
    Ok(CmdResult {
        ok: false,
        msg: if detail.is_empty() {
            "The tunnel setup did not finish. Approve the Windows prompt to let it run".into()
        } else {
            format!("The tunnel setup did not finish: {detail}")
        },
    })
}

#[tauri::command]
async fn tunnel_stop(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    lifecycle: tauri::State<'_, Lifecycle>,
) -> Result<CmdResult, String> {
    // Queued behind a start that is still running: pressing stop during a
    // connect cancels it, and the cancel must not overtake the spawn.
    let _gate = lifecycle.0.lock().await;
    vpn::app_log("stop from app");
    stop_engine(&engine);
    // The next session starts without a space unless the user selects one.
    note_tunnel_space(&app, false);
    // Drop the tunnel's DNS answers so normal resolution resumes cleanly.
    flush_dns_cache();
    Ok(CmdResult {
        ok: true,
        msg: "VPN off".into(),
    })
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
                Ok(TunnelState {
                    running: true,
                    error: String::new(),
                })
            } else {
                Ok(TunnelState {
                    running: false,
                    error: String::new(),
                })
            }
        }
        Some(_) => {
            *engine.0.lock().unwrap() = None;
            Ok(TunnelState {
                running: false,
                error: vpn::engine_log_tail(),
            })
        }
        None => {
            // Adopt a live engine we lost track of: the task owns it now, and
            // after an app restart the pid file points at it.
            if let Some(p) = vpn::task_engine_pid() {
                *engine.0.lock().unwrap() = Some(p);
                return Ok(TunnelState {
                    running: vpn::tun_routes_present(),
                    error: String::new(),
                });
            }
            if vpn::tun_routes_present() {
                if let Ok(s) = std::fs::read_to_string(vpn::engine_dir().join("engine.pid")) {
                    if let Ok(p) = s.trim().parse::<u32>() {
                        if p != 0 && pid_alive(p) {
                            *engine.0.lock().unwrap() = Some(p);
                            return Ok(TunnelState {
                                running: true,
                                error: String::new(),
                            });
                        }
                    }
                }
            }
            Ok(TunnelState {
                running: false,
                error: String::new(),
            })
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

/// What this session has carried, per (process, host), heaviest first. Five
/// rows, in memory only. The list is replaced when the next session starts and
/// dies with the process, so a stopped tunnel keeps showing the session it just
/// described but nothing is written down. An empty list is a normal answer: no
/// tunnel yet, nothing moved yet, or an engine that does not report
/// connections.
#[tauri::command]
fn tunnel_destinations(dests: tauri::State<'_, Dests>) -> Vec<vpn::Destination> {
    match dests.0.lock() {
        Ok(t) => t.top(5),
        Err(_) => Vec::new(),
    }
}

/// Read the engine's Clash `/connections` list while `pid` is the session's
/// engine, and fold each poll into the in-memory totals.
///
/// It polls because the engine offers no stream, and it exits on its own the
/// moment the state stops owning `pid`: no task outlives the tunnel it
/// describes, so a stop path does not have to remember to cancel it.
///
/// Every failure here is silent on purpose. A missing endpoint is what a tunnel
/// that is still starting looks like, and it must never become an error on a
/// screen that is only trying to say where the bytes went.
fn spawn_dest_poll(app: tauri::AppHandle, pid: u32) {
    tauri::async_runtime::spawn(async move {
        let ctl = vpn::engine_dir().join("clash-ctl.txt");
        let client = match reqwest::Client::builder().timeout(DEST_POLL).build() {
            Ok(c) => c,
            Err(_) => return,
        };
        loop {
            // The same ownership test the rest of the backend uses. A new
            // connect, a stop, or a crashed engine all clear this pid.
            let owned = match app.state::<Engine>().0.lock() {
                Ok(g) => *g == Some(pid),
                Err(_) => false,
            };
            if !owned {
                return;
            }
            // Re-read every tick: a reconnect rewrites this file with a fresh
            // port and secret, and an old poller must not keep dialling.
            let coords = std::fs::read_to_string(&ctl).ok().and_then(|raw| {
                let mut lines = raw.lines();
                let port = lines.next()?.trim().parse::<u16>().ok()?;
                let secret = lines.next().unwrap_or("").trim().to_string();
                Some((port, secret))
            });
            if let Some((port, secret)) = coords {
                let url = format!("http://127.0.0.1:{port}/connections");
                let sent = client
                    .get(url)
                    .header("Authorization", format!("Bearer {secret}"))
                    .send()
                    .await;
                if let Ok(r) = sent {
                    if let Ok(body) = r.json::<serde_json::Value>().await {
                        if let Ok(mut t) = app.state::<Dests>().0.lock() {
                            t.observe(&body);
                        }
                    }
                }
            }
            tokio::time::sleep(DEST_POLL).await;
        }
    });
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
        let rows =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
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
                    alias.starts_with("quotavpn")
                        || alias.starts_with("QuotaVPN")
                        || desc.contains("quotavpn")
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
        return Ok(CmdResult {
            ok: false,
            msg: "No log yet - connect the VPN first".into(),
        });
    }
    app.clipboard()
        .write_text(lines)
        .map_err(|e| e.to_string())?;
    Ok(CmdResult {
        ok: true,
        msg: "Log copied - paste it to Cypher".into(),
    })
}

/// Put text on the clipboard through the native clipboard, which does not
/// depend on what the web engine decides about the async clipboard API.
#[tauri::command]
fn copy_text(app: tauri::AppHandle, text: String) -> Result<bool, String> {
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Curated Windows processes for per-app routing (exe name + label).
/// The engine matches by process name/path; whole-PC stays the default.
#[cfg(windows)]
#[tauri::command]
async fn tunnel_apps() -> Result<String, String> {
    // Live list: whatever Windows is actually running right now, one row
    // per distinct exe. Nothing hardcoded, nothing curated.
    const SKIP: &[&str] = &[
        "system",
        "registry",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "winlogon.exe",
        "services.exe",
        "lsass.exe",
        "lsm.exe",
        "svchost.exe",
        "dwm.exe",
        "explorer.exe",
        "ctfmon.exe",
        "conhost.exe",
        "dllhost.exe",
        "taskhostw.exe",
        "sihost.exe",
        "fontdrvhost.exe",
        "backgroundtaskhost.exe",
        "runtimebroker.exe",
        "searchindexer.exe",
        "searchhost.exe",
        "startmenuexperiencehost.exe",
        "shellexperiencehost.exe",
        "applicationframehost.exe",
        "systemsettings.exe",
        "lockapp.exe",
        "widgets.exe",
        "msedgewebview2.exe",
        "quotacards.exe",
        "quotavpn.exe",
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

/// The routing list on Linux: whatever is actually running. sing-box matches
/// on the executable path, so the path is the value and the file name is the
/// label. Kernel threads have no exe and are skipped; our own process is not
/// a thing anyone routes through itself.
#[cfg(not(windows))]
#[tauri::command]
async fn tunnel_apps() -> Result<String, String> {
    let mut seen = std::collections::BTreeSet::<String>::new();
    let mut rows: Vec<(String, String)> = Vec::new();
    let me = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if let Ok(dir) = std::fs::read_dir("/proc") {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let exe = match std::fs::read_link(format!("/proc/{name}/exe")) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => continue,
            };
            if exe.is_empty() || exe.contains("(deleted)") || exe == me {
                continue;
            }
            if exe.starts_with("/proc/") || exe.starts_with("/sys/") {
                continue;
            }
            if !seen.insert(exe.clone()) {
                continue;
            }
            let label = exe.rsplit('/').next().unwrap_or(&exe).to_string();
            rows.push((exe, label));
            if rows.len() >= 400 {
                break;
            }
        }
    }
    rows.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    let v: Vec<serde_json::Value> = rows
        .iter()
        .map(|(pkg, label)| serde_json::json!({ "pkg": pkg, "label": label }))
        .collect();
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

/// Which system this build runs on, so the interface can leave out what does
/// not exist here.
#[tauri::command]
fn platform() -> String {
    std::env::consts::OS.to_string()
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
        return Ok(CmdResult {
            ok: true,
            msg: "Tunnel up".into(),
        });
    }
    let host = {
        let state = app.state::<State>();
        let guard = state.0.lock().unwrap();
        guard.server_ip.clone()
    };
    if host.is_empty() {
        return Ok(CmdResult {
            ok: false,
            msg: "No server set".into(),
        });
    }
    let target = format!("{host}:443");
    let conn = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(&target),
    )
    .await;
    match conn {
        Ok(Ok(_)) => Ok(CmdResult {
            ok: true,
            msg: format!("{target} reachable"),
        }),
        Ok(Err(e)) => Ok(CmdResult {
            ok: false,
            msg: format!("{target} refused: {e}"),
        }),
        Err(_) => Ok(CmdResult {
            ok: false,
            msg: format!("{target} timed out (blocked?)"),
        }),
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
    let isp = v
        .get("org")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let place = join_place(
        v.get("city").and_then(|x| x.as_str()),
        v.get("country_name").and_then(|x| x.as_str()),
    );
    Some(NetInfo { ip, isp, place })
}

async fn net_from_cf(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client
        .get("https://cloudflare.com/cdn-cgi/trace")
        .send()
        .await
        .ok()?;
    let body = r.text().await.ok()?;
    let ip = body
        .lines()
        .find_map(|l| l.strip_prefix("ip="))?
        .trim()
        .to_string();
    if ip.is_empty() {
        return None;
    }
    Some(NetInfo {
        ip,
        isp: String::new(),
        place: String::new(),
    })
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
                // Only names are accepted. A caller-supplied path used to be taken
                // verbatim and handed to SHGetFileInfoW, so `\\host\share\x.ico`
                // made this elevated process authenticate to a machine it had never
                // heard of, and run whatever shell handler lived there.
                let path = paths
                    .get(&pkg.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or_default();
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
                    if let Ok(h) = OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    ) {
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
        out.push(if c.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Read a GDI bitmap into raw bytes at the requested depth.
#[cfg(windows)]
unsafe fn read_dib(
    hbm: windows::Win32::Graphics::Gdi::HBITMAP,
    w: i32,
    h: i32,
    bpp: u16,
) -> Option<Vec<u8>> {
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
    let lines = GetDIBits(
        dc,
        hbm,
        0,
        h as u32,
        Some(buf.as_mut_ptr() as *mut _),
        &mut bi,
        DIB_RGB_COLORS,
    );
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
                let color = if !ii.hbmColor.0.is_null() {
                    ii.hbmColor
                } else {
                    ii.hbmMask
                };
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
#[cfg(windows)]
async fn speed_servers() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent(BROWSER_UA)
        .build()
        .ok()?;

    let mut ookla: Option<String> = None;
    let mut libre: Option<String> = None;

    let ookla_url = "https://www.speedtest.net/api/js/servers?engine=js&limit=60";
    if let Ok(r) = client
        .get(ookla_url)
        .header("Accept", "application/json")
        .send()
        .await
    {
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
#[cfg(windows)]
async fn speed_latency(url: String, probes: u32) -> Vec<f64> {
    speed::latency(&speed_client(false), &url, probes.clamp(1, 10)).await
}

/// Whether a server can actually be used end to end: the probe follows
/// redirects, because several provider hosts answer their own name with a 307
/// to an Ookla hostname, and a machine that cannot resolve or reach that
/// second name fails on every download and upload while a plain ping still
/// works. Candidates that cannot complete the full path are not offered.
#[tauri::command]
#[cfg(windows)]
async fn speed_reach(url: String) -> bool {
    !speed::latency(&speed_client(true), &url, 1)
        .await
        .is_empty()
}

#[tauri::command]
#[cfg(windows)]
async fn speed_down(app: tauri::AppHandle, urls: Vec<String>, seconds: f64) -> speed::SpeedOut {
    speed::uncancel();
    speed::download(&speed_client(true), &urls, seconds.clamp(1.0, 30.0), |v| {
        let _ = app.emit("speed-tick", v);
    })
    .await
}

#[tauri::command]
#[cfg(windows)]
async fn speed_up(
    app: tauri::AppHandle,
    url: String,
    seconds: f64,
    chunk_mb: Option<u32>,
) -> speed::SpeedOut {
    let chunk = chunk_mb.unwrap_or(2).clamp(1, 8) as usize;
    speed::upload(
        &speed_client(true),
        &url,
        seconds.clamp(2.0, 30.0),
        chunk,
        |v| {
            let _ = app.emit("speed-tick", v);
        },
    )
    .await
}

/// Whether the bundled official client is fetched and runnable. The first
/// call downloads it once and caches it under the app data dir.
/// Stop whatever is measuring: the cli process and the built-in loops.
/// The wire this machine sits on: adapter, mac, gateway.
#[tauri::command]
#[cfg(windows)]
async fn lan_wire() -> Option<quotacards::lan_ctrl::Wire> {
    tauri::async_runtime::spawn_blocking(quotacards::lan_ctrl::wire)
        .await
        .ok()
        .flatten()
}

/// Start carrying one device. Nothing changes on the wire unless the capture is
/// open, the device has answered for its address, and the first claim went out.
#[tauri::command]
#[cfg(windows)]
fn lan_start(
    state: tauri::State<'_, LanState>,
    cfg: tauri::State<'_, State>,
    ip: String,
    mac: Option<String>,
    blocked: bool,
) -> Result<bool, String> {
    {
        let guard = cfg.0.lock().map_err(|_| "config".to_string())?;
        if let Some(why) = protected_reason(&ip, mac.as_deref().unwrap_or(""), &guard) {
            return Err(why);
        }
    }
    // One session at a time, and the old one is handed back before the new one
    // claims anything: two relays would fight over the same adapter.
    let previous = {
        let mut guard = state.0.lock().map_err(|_| "state".to_string())?;
        guard.take()
    };
    if let Some(s) = previous {
        s.stop();
    }
    let session = quotacards::lan_relay::start_with(&ip, blocked, mac)?;
    let mut guard = state.0.lock().map_err(|_| "state".to_string())?;
    *guard = Some(session);
    Ok(true)
}

/// Keep a device out of reach of the controls, or let it back in.
#[tauri::command]
#[cfg(windows)]
fn lan_protect(mac: String, on: bool, state: tauri::State<State>) -> Result<bool, String> {
    let mac = mac.to_lowercase();
    let mut cfg = state.0.lock().map_err(|_| "config".to_string())?;
    let already = cfg
        .lan_protected
        .iter()
        .any(|m| m.eq_ignore_ascii_case(&mac));
    if on && !already {
        cfg.lan_protected.push(mac);
        cfg.save();
    } else if !on && already {
        cfg.lan_protected.retain(|m| !m.eq_ignore_ascii_case(&mac));
        cfg.save();
    }
    Ok(true)
}

#[cfg(windows)]
/// Whether this address is off limits, and why, in words the window can show.
fn protected_reason(ip: &str, mac: &str, cfg: &AppConfig) -> Option<String> {
    if let Some(w) = quotacards::lan_ctrl::wire() {
        if ip == w.ip {
            return Some("This PC is protected".to_string());
        }
        if !w.gateway_ip.is_empty() && ip == w.gateway_ip {
            return Some("The router is protected".to_string());
        }
    }
    if cfg
        .lan_protected
        .iter()
        .any(|m| m.eq_ignore_ascii_case(mac))
    {
        return Some("This device is protected".to_string());
    }
    None
}

/// Whether anyone is trying to stand between this machine and the router.
#[tauri::command]
#[cfg(windows)]
fn lan_guard() -> quotacards::lan_ctrl::Guard {
    quotacards::lan_ctrl::guard_state()
}

/// What the two drivers need: the packet driver WinDivert (loaded from the
/// driver folder) and Npcap for forging the claim frame. The app ships the
/// WinDivert pair; this copies it out of the resource folder when it is not
/// already unpacked, so control works on a fresh install without a second
/// installer.
#[derive(serde::Serialize)]
#[cfg(windows)]
pub struct LanPrepare {
    pub windivert: bool,
    pub npcap: bool,
    pub detail: String,
}

#[tauri::command]
#[cfg(windows)]
async fn lan_prepare(_app: tauri::AppHandle) -> Result<LanPrepare, String> {
    // There is no bundled driver any more. Carrying a device is Npcap's job, and
    // its installer has no silent mode, so the only thing this can report is
    // whether it is already there.
    let npcap = quotacards::lan_route::npcap_installed();
    Ok(LanPrepare {
        windivert: npcap,
        npcap,
        detail: if npcap {
            String::new()
        } else {
            "Npcap is not installed".to_string()
        },
    })
}

/// Block or unblock the device that is already under control, without ending
/// the session. This is the quick switch; a fresh block on an uncontrolled
/// device goes through `lan_start` with `blocked: true`.
#[tauri::command]
#[cfg(windows)]
fn lan_block(state: tauri::State<'_, LanState>, on: bool) -> Result<bool, String> {
    let guard = state.0.lock().map_err(|_| "state".to_string())?;
    let Some(session) = guard.as_ref() else {
        return Ok(false);
    };
    session
        .blocked
        .store(on, std::sync::atomic::Ordering::Relaxed);
    Ok(true)
}

/// Set the caps for the active device. Zero means no cap.
#[tauri::command]
#[cfg(windows)]
fn lan_limits(    state: tauri::State<'_, LanState>,
    cfg: tauri::State<'_, State>,
    down_kbps: u32,
    up_kbps: u32,
) -> Result<bool, String> {
    let guard = state.0.lock().map_err(|_| "state".to_string())?;
    let Some(session) = guard.as_ref() else {
        return Ok(false);
    };
    {
        let conf = cfg.0.lock().map_err(|_| "config".to_string())?;
        if let Some(why) = protected_reason(&session.target_ip, &session.target_mac, &conf) {
            return Err(why);
        }
    }
    session
        .down_kbps
        .store(down_kbps, std::sync::atomic::Ordering::Relaxed);
    session
        .up_kbps
        .store(up_kbps, std::sync::atomic::Ordering::Relaxed);
    Ok(true)
}

/// Stop redirecting and hand the network back.
#[tauri::command]
#[cfg(windows)]
fn lan_stop(state: tauri::State<'_, LanState>) -> Result<bool, String> {
    let session = {
        let mut guard = state.0.lock().map_err(|_| "state".to_string())?;
        guard.take()
    };
    if let Some(s) = session {
        s.stop();
    }
    Ok(true)
}

/// Whether the raw packet driver is installed. The relay is the only thing that
/// carries a device now, and it runs on Npcap.
#[tauri::command]
#[cfg(windows)]
async fn pipeline_ready() -> bool {
    quotacards::lan_route::npcap_installed()
}

/// The devices the machine already knows about, without sweeping.
#[tauri::command]
#[cfg(windows)]
fn lan_known(cfg: tauri::State<'_, State>) -> Vec<quotacards::lan::Device> {
    let mut list = quotacards::lan::known();
    // The cached list is painted before the scan answers, so the two facts the
    // page needs at once are filled here as well: who the router is, and which
    // addresses the owner protected.
    let wire = quotacards::lan_ctrl::wire();
    let mut gateway = String::new();
    if let Some(w) = wire.as_ref() {
        gateway = w.gateway_ip.clone();
        // The neighbour table has no entry for this PC's own address, so a quick
        // read would otherwise show everybody except the machine running the app.
        if !list.iter().any(|d| d.ip == w.ip) {
            list.push(quotacards::lan::Device {
                ip: w.ip.clone(),
                mac: w.mac.clone(),
                name: String::new(),
                self_pc: true,
                name_source: "self".into(),
                protected: true,
                protected_always: true,
                live: true,
            });
        }
        // A poisoned cache during a session can put this PC's address on the
        // router or a phone. Those rows are claims, not devices, and they must
        // not reach the page: one MAC there is also one React key.
        list = quotacards::lan::tidy_roster(
            list,
            &w.ip,
            &w.mac,
            &w.gateway_ip,
            &w.gateway_mac,
        );
    }
    let protected = cfg
        .0
        .lock()
        .map(|c| c.lan_protected.clone())
        .unwrap_or_default();
    for d in list.iter_mut() {
        if d.self_pc || (!gateway.is_empty() && d.ip == gateway) {
            d.protected_always = !d.self_pc;
            d.protected = true;
        }
        if protected.iter().any(|m| m == &d.mac) {
            d.protected = true;
        }
    }
    list
}

/// Who is on this network: ip, mac and a name when the system resolves one.
///
/// The router is the only source that knows every name, so when a session is
/// open its full client table is merged in. That also adds the phones the ping
/// sweep slept through, which is what makes the roster read "everybody" rather
/// than "everybody awake". A router-only row is a lease, not a live sighting;
/// the screen labels it as such.
#[cfg(windows)]
static LAN_SWEEP: std::sync::OnceLock<std::sync::Mutex<Vec<quotacards::lan::Device>>> =
    std::sync::OnceLock::new();
#[cfg(windows)]
static LAN_SWEEP_BUSY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Wake the sleeping devices, in the background. The page's roster is already
/// on screen from the neighbour and router tables; this runs only when the
/// owner presses Refresh, and the result is kept for the next snapshot.
#[tauri::command]
#[cfg(windows)]
async fn lan_scan() -> Result<bool, String> {
    if LAN_SWEEP_BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Ok(false);
    }
    let found = tauri::async_runtime::spawn_blocking(quotacards::lan::scan).await;
    LAN_SWEEP_BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
    let list = found.map_err(|e| e.to_string())?;
    let slot = LAN_SWEEP.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    *slot.lock().map_err(|_| "the sweep cache is poisoned".to_string())? = list;
    Ok(true)
}

#[cfg(windows)]
async fn scan_with_router(
    router: &tauri::State<'_, RouterState>,
    cfg: &tauri::State<'_, State>,
) -> Result<Vec<quotacards::lan::Device>, String> {
    // Fast roster: the neighbour table plus, when the router session is live,
    // its client table. The 254-address sweep is not here: it takes up to
    // sixteen seconds, and a page that waits for it reads as broken. The sweep
    // remains a headless diagnostic; the page uses what the network already
    // knows.
    let mut list = quotacards::lan::known();
    let wire = quotacards::lan_ctrl::wire();
    if let Some(w) = wire.as_ref() {
        if !list.iter().any(|d| d.ip == w.ip) {
            list.push(quotacards::lan::Device {
                ip: w.ip.clone(),
                mac: w.mac.clone(),
                name: String::new(),
                self_pc: true,
                name_source: "self".into(),
                protected: true,
                protected_always: true,
                live: true,
            });
        }
    }
    // The last explicit sweep found the devices that were asleep when the
    // neighbour table was written. Fold them in before the router names so the
    // router can still name them.
    if let Some(slot) = LAN_SWEEP.get() {
        if let Ok(cached) = slot.lock() {
            for d in cached.iter() {
                let seen = list.iter().any(|x| {
                    x.ip == d.ip || (!d.mac.is_empty() && x.mac.eq_ignore_ascii_case(&d.mac))
                });
                if !seen {
                    list.push(d.clone());
                }
            }
        }
    }
    let mut clients: Vec<router_huawei::RouterClient> = Vec::new();
    // Never wait for the lock: a router session that is mid-poll held it for
    // the whole scan once, and the page sat on "Scanning" forever. A skipped
    // enrichment costs names, never the list.
    if let Ok(guard) = router.0.try_lock() {
        if let Some(session) = guard.as_ref() {
            // A router page can sit for its own twelve second timeout, and the
            // page must not wait for it. The local roster is already complete;
            // names are an enrichment, and a slow router costs one round.
            if let Ok(Ok(found)) = tokio::time::timeout(
                std::time::Duration::from_millis(1200),
                session.clients(),
            )
            .await
            {
                clients = found;
            }
        }
    }
    let mut learned: Vec<(String, String)> = Vec::new();
    for c in &clients {
        let named = !c.name.trim().is_empty();
        if let Some(d) = list.iter_mut().find(|d| {
            (!c.mac.is_empty() && d.mac.eq_ignore_ascii_case(&c.mac)) || d.ip == c.ip
        }) {
            if named {
                quotacards::lan::offer_name(d, &c.name, "router");
                if !d.mac.is_empty() {
                    learned.push((d.mac.clone(), c.name.clone()));
                }
            }
            continue;
        }
        // The router has a lease the sweep did not see. Keep the row: a phone
        // that is asleep is still on the network, it just did not answer.
        if c.ip.is_empty() {
            continue;
        }
        list.push(quotacards::lan::Device {
            ip: c.ip.clone(),
            mac: c.mac.clone(),
            name: if named { c.name.clone() } else { String::new() },
            self_pc: false,
            name_source: if named { "router".into() } else { String::new() },
            protected: false,
            protected_always: false,
            live: false,
        });
        if named && !c.mac.is_empty() {
            learned.push((c.mac.clone(), c.name.clone()));
        }
    }
    if !learned.is_empty() {
        quotacards::lan::save_router_names(learned);
    }
    // The router's own table is merged before this: a lease for a phone that
    // renewed while the relay carried it can carry the relay's source address.
    // Tidy once so one device is one row and the two fixed addresses are true,
    // then mark protection on the rows that survived.
    if let Some(w) = wire.as_ref() {
        list = quotacards::lan::tidy_roster(
            list,
            &w.ip,
            &w.mac,
            &w.gateway_ip,
            &w.gateway_mac,
        );
    }
    // Protection, marked here so every screen agrees: the router and this PC
    // are always safe, plus whatever the user marked.
    let chosen = cfg
        .0
        .lock()
        .map(|c| c.lan_protected.clone())
        .unwrap_or_default();
    let gateway_ip = wire
        .as_ref()
        .map(|w| w.gateway_ip.clone())
        .unwrap_or_default();
    for d in list.iter_mut() {
        let fixed = d.self_pc || (!gateway_ip.is_empty() && d.ip == gateway_ip);
        d.protected_always = fixed;
        d.protected = fixed || chosen.iter().any(|m| m.eq_ignore_ascii_case(&d.mac));
    }
    // Names that are still missing are looked up in the background, so the list
    // is on screen in a second and fills itself in a few seconds later. The
    // screen only shows names that came from the router.
    let probe = list.clone();
    std::thread::spawn(move || quotacards::lan::enrich_names(probe));
    Ok(list)
}

#[cfg(windows)]
#[tauri::command]
async fn lan_devices(
    router: tauri::State<'_, RouterState>,
    cfg: tauri::State<'_, State>,
) -> Result<Vec<quotacards::lan::Device>, String> {
    scan_with_router(&router, &cfg).await
}

/// Whole-network interception is retired. Watching every device put the whole
/// house through one userland queue and collapsed the network after a couple
/// of minutes. Discovery is passive now; only a device the user explicitly
/// controls is claimed, one at a time, through `lan_start`.
#[tauri::command]
#[cfg(windows)]
async fn lan_monitor_start(_state: tauri::State<'_, MonitorState>) -> Result<bool, String> {
    Err("Passive discovery is always on. Select a device to control it.".into())
}

/// Stop watching and hand the network back.
#[tauri::command]
#[cfg(windows)]
fn lan_monitor_stop(state: tauri::State<'_, MonitorState>) -> Result<bool, String> {
    let taken = {
        let mut guard = state.0.lock().map_err(|_| "state".to_string())?;
        guard.take()
    };
    if let Some(session) = taken {
        session.stop();
    }
    Ok(true)
}

/// Whether the whole network is being watched right now.
#[tauri::command]
#[cfg(windows)]
fn lan_monitor_active(state: tauri::State<'_, MonitorState>) -> Result<bool, String> {
    let guard = state.0.lock().map_err(|_| "state".to_string())?;
    Ok(guard.is_some())
}

/// Bytes seen per device since the watch began.
#[tauri::command]
#[cfg(windows)]
fn lan_traffic(state: tauri::State<'_, MonitorState>) -> Result<Vec<TrafficRow>, String> {
    let guard = state.0.lock().map_err(|_| "state".to_string())?;
    let Some(session) = guard.as_ref() else {
        return Ok(Vec::new());
    };
    Ok(session
        .snapshot()
        .into_iter()
        .map(|c| TrafficRow {
            mac: c.mac,
            ip: c.ip,
            up: c.up,
            down: c.down,
        })
        .collect())
}

/// Whether the kill switch is armed, and whether the block is on right now.
#[tauri::command]
fn kill_switch_state() -> serde_json::Value {
    serde_json::json!({
        "armed": vpn::kill_switch_on(),
        "blocked": vpn::traffic_blocked(),
    })
}

#[tauri::command]
fn kill_switch_set(on: bool) -> Result<bool, String> {
    vpn::set_kill_switch(on)?;
    Ok(true)
}

/// Lift a block that is in place without starting a tunnel, through the one
/// elevated pass (`--killswitch off`). Elevates once.
fn release_block_elevated() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        let script = format!(
            "Start-Process -FilePath '{}' -ArgumentList '--killswitch','off' -Verb RunAs -WindowStyle Hidden",
            exe.display()
        );
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("pkexec")
            .arg(&exe)
            .arg("--killswitch")
            .arg("off")
            .spawn()
            .map_err(|e| format!("pkexec: {e}"))?;
    }
    Ok(())
}

/// Lift a block that is in place without starting a tunnel.
#[tauri::command]
fn kill_switch_release() -> Result<bool, String> {
    release_block_elevated()?;
    Ok(true)
}

/// Open a web page in the user's own browser. Used for the one thing the app
/// cannot install for them.
#[tauri::command]
fn open_page(url: String) -> bool {
    // A prefix test is not a check. `cmd /C start` re-parses its argument, so
    // `https://x&calc.exe` ran a program from an "https" string, inside an
    // elevated process. The alphabet below is a URL's and nothing else: no
    // space, no `&`, `|`, `^`, `<`, `>`, `"`, `'`, `;`, `$`, backtick, braces.
    // What survives leaves cmd nothing to parse as a second command.
    fn safe(c: char) -> bool {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '-' | '.'
                    | '_'
                    | '~'
                    | ':'
                    | '/'
                    | '?'
                    | '#'
                    | '['
                    | ']'
                    | '@'
                    | '!'
                    | '*'
                    | '+'
                    | ','
                    | '='
                    | '%'
                    | '('
                    | ')'
            )
    }
    if !url.starts_with("https://") || url.len() > 2048 || !url.chars().all(safe) {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        return std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(NO_WINDOW)
            .spawn()
            .is_ok();
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Log in to the router so the device list can carry real names.
#[tauri::command]
#[cfg(windows)]
async fn router_link(
    state: tauri::State<'_, RouterState>,
    user: String,
    pass: String,
) -> Result<bool, String> {
    let host = quotacards::lan_ctrl::wire()
        .map(|w| w.gateway_ip)
        .ok_or("no gateway")?;
    let session = router_huawei::login(&host, &user, &pass).await?;
    // Keep the router's own control pages for the next step: caps and blocking
    // move there, so the switch enforces them without this PC forwarding.
    session.dump_pages().await;
    // The password lives in this request only. A previous build kept a sealed
    // copy for autolink; that file is deleted here so it cannot outlive the
    // upgrade.
    let _ = router_remember::clear();
    let mut guard = state.0.lock().await;
    *guard = Some(session);
    Ok(true)
}

/// Reconnect with the saved login, if there is one.
#[tauri::command]
#[cfg(windows)]
async fn router_autolink(state: tauri::State<'_, RouterState>) -> Result<bool, String> {
    let Some((user, pass)) = router_remember::load() else {
        return Ok(false);
    };
    let host = quotacards::lan_ctrl::wire()
        .map(|w| w.gateway_ip)
        .ok_or("no gateway")?;
    match router_huawei::login(&host, &user, &pass).await {
        Ok(session) => {
            session.dump_pages().await;
            let mut guard = state.0.lock().await;
            *guard = Some(session);
            Ok(true)
        }
        Err(_) => {
            // The router changed its password or the account changed. Drop the
            // stale secret so the button asks for a fresh sign in.
            router_remember::clear();
            Ok(false)
        }
    }
}

/// The router's own client table, names included. The page calls this after a
/// fresh sign in so a device the sweep cannot wake still appears with its name.
#[tauri::command]
#[cfg(windows)]
async fn router_clients(
    state: tauri::State<'_, RouterState>,
) -> Result<Vec<router_huawei::RouterClient>, String> {
    let guard = state.0.lock().await;
    let Some(session) = guard.as_ref() else {
        return Err("Sign in to your router to load names.".into());
    };
    session.clients().await
}

#[tauri::command]
#[cfg(windows)]
async fn router_status(state: tauri::State<'_, RouterState>) -> Result<bool, String> {
    let guard = state.0.lock().await;
    Ok(guard.is_some())
}

#[tauri::command]
#[cfg(windows)]
async fn router_unlink(state: tauri::State<'_, RouterState>) -> Result<bool, String> {
    let mut guard = state.0.lock().await;
    *guard = None;
    router_remember::clear();
    Ok(true)
}

/// What the redirect is doing right now, so the screen can say so after a
/// restart or a tab switch instead of forgetting it.
#[tauri::command]
#[cfg(windows)]
fn lan_status(state: tauri::State<'_, LanState>) -> Option<LanStatus> {
    let guard = state.0.lock().ok()?;
    let session = guard.as_ref()?;
    // The relay is the honest owner of "under control". If it has stopped for
    // any reason the panel must not keep claiming the device.
    if !session.alive() {
        return None;
    }
    Some(LanStatus {
        ip: session.target_ip.clone(),
        mac: session.target_mac.clone(),
        down_kbps: session.down_kbps.load(std::sync::atomic::Ordering::Relaxed),
        up_kbps: session.up_kbps.load(std::sync::atomic::Ordering::Relaxed),
        blocked: session.blocked.load(std::sync::atomic::Ordering::Relaxed),
        up_bytes: session.bytes_up.load(std::sync::atomic::Ordering::Relaxed),
        down_bytes: session
            .bytes_down
            .load(std::sync::atomic::Ordering::Relaxed),
    })
}

#[derive(serde::Serialize)]
#[cfg(windows)]
pub struct LanStatus {
    pub ip: String,
    pub mac: String,
    pub down_kbps: u32,
    pub up_kbps: u32,
    pub blocked: bool,
    /// Bytes seen from this device and to it since the session began.
    pub up_bytes: u64,
    pub down_bytes: u64,
}

#[tauri::command]
#[cfg(windows)]
async fn speed_cancel() -> bool {
    speed::CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed);
    ookla::cancel();
    true
}

#[tauri::command]
#[cfg(windows)]
async fn speedtest_cli_ready() -> bool {
    ookla::ensure_binary().await.is_some()
}

/// One full test through the official Ookla client. Live progress arrives as
/// `speed-phase` and `speed-tick` events.
#[tauri::command]
#[cfg(windows)]
async fn speedtest_cli(app: tauri::AppHandle) -> Option<ookla::CliResult> {
    speed::uncancel();
    let exe = ookla::ensure_binary().await?;
    let a1 = app.clone();
    let on_phase: std::sync::Arc<dyn Fn(&str) + Send + Sync> =
        std::sync::Arc::new(move |p: &str| {
            let _ = a1.emit("speed-phase", p.to_string());
        });
    let a2 = app.clone();
    let on_tick: std::sync::Arc<dyn Fn(f64) + Send + Sync> = std::sync::Arc::new(move |v: f64| {
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

fn parse_version(text: &str) -> ((u64, u64, u64), Option<Vec<String>>) {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (
            core,
            Some(pre.split('.').map(|part| part.to_string()).collect::<Vec<_>>()),
        ),
        None => (text, None),
    };
    let mut parts = core.split('.').map(|part| {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        ),
        pre,
    )
}

fn compare_pre(a: &[String], b: &[String]) -> std::cmp::Ordering {
    for (left, right) in a.iter().zip(b) {
        let left_num = left.parse::<u64>().ok();
        let right_num = right.parse::<u64>().ok();
        let order = match (left_num, right_num) {
            (Some(l), Some(r)) => l.cmp(&r),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        };
        if order != std::cmp::Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

fn compare_version(latest: &str, current: &str) -> std::cmp::Ordering {
    let (latest_core, latest_pre) = parse_version(latest);
    let (current_core, current_pre) = parse_version(current);
    match latest_core.cmp(&current_core) {
        std::cmp::Ordering::Equal => match (latest_pre, current_pre) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(a), Some(b)) => compare_pre(&a, &b),
        },
        other => other,
    }
}

fn newer(latest: &str, current: &str) -> bool {
    compare_version(latest, current) == std::cmp::Ordering::Greater
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

/// The updater handle for a channel. The experimental channel resolves the newest published
/// release itself, pre-releases included; the stable channel keeps the
/// built-in endpoint, which GitHub only serves for full releases.
async fn updater(
    app: &tauri::AppHandle,
    channel: &str,
) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app.updater_builder();
    // The plugin's default compare demands a strictly newer version number,
    // but a build of a version line (every test build, every hotfix re-release)
    // keeps its number and differs only by build stamp. Accept equals here and
    // let offer_equal_version make the real call: it refuses a feed stamp that
    // is missing or identical to this build, so nothing loops. Without this the
    // check resolved the right feed and then reported "up to date" anyway
    // because 0.4.0 is not newer than 0.4.0.
    builder = builder.version_comparator(|current, remote| remote.version >= current);
    if channel == "experimental" {
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
/// test builds before newer ones), so the pick is made here: the highest tag
/// version wins, published_at breaks a tie. Choosing the first entry that
/// merely looks valid resolved a stale feed and would have offered an older
/// build as an update.
/// "v0.4.11" becomes (0, 4, 11). A tag that carries no version ranks at zero,
/// so it can never win the experimental pick by accident.
fn version_rank(tag: &str) -> (u64, u64, u64) {
    let mut parts = tag.trim_start_matches(['v', 'V']).split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

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
                && r.get("assets").and_then(|v| v.as_array()).is_some_and(|a| {
                    a.iter()
                        .any(|x| x.get("name").and_then(|n| n.as_str()) == Some("latest.json"))
                })
        })
        .max_by_key(|r| {
            // By version, not by time. Two releases published in the same
            // second share created_at, and the timestamp pick then resolves
            // whichever one the API happened to list last: 0.4.10 and 0.4.11
            // were created at 18:20:36Z to the second, and a 0.4.10 install
            // was told it was up to date. published_at breaks a version tie.
            let tag = r.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
            let stamp = r
                .get("published_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (version_rank(tag), stamp)
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
    let ch = if channel == "experimental" {
        "experimental"
    } else {
        "normal"
    };
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
async fn check_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<UpdateInfo, String> {
    const REPO: &str = "YousefMohiey/QuotaVPN";
    let current = app.package_info().version.to_string();
    let channel = state.0.lock().unwrap().update_channel.clone();
    let found = updater(&app, &channel).await?.check().await.map_err(|e| {
        let msg = check_failed(e);
        // The window shows whatever comes back, and an empty string used to
        // fall back to "Could not reach GitHub" - a guess that hid the real
        // reason. Every failure is logged with its cause now, and the string
        // always carries something.
        vpn::app_log(&format!("update: {msg}"));
        if msg.trim().is_empty() {
            "the update check failed without a reason".to_string()
        } else {
            msg
        }
    })?;
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
async fn apply_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
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
    // The installer ends this process before it replaces the binary, so the
    // restart below never runs and the window does not come back. The marker
    // tells the install hook this was an in-app update and that the app it
    // just replaced should be started again; a fresh install writes nothing.
    mark_update_started();
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
            clear_update_marker();
            Err(format!("install failed: {e}"))
        }
    }
}

/// Records that an in-app update is installing, so the install hook knows to
/// start the app again. Cleared when the install fails.
fn update_marker() -> Option<std::path::PathBuf> {
    let base = std::env::var("LOCALAPPDATA").ok()?;
    Some(
        std::path::PathBuf::from(base)
            .join("quotavpn")
            .join("updating"),
    )
}

fn mark_update_started() {
    if let Some(p) = update_marker() {
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(&p));
        let _ = std::fs::write(&p, b"1");
    }
}

fn clear_update_marker() {
    if let Some(p) = update_marker() {
        let _ = std::fs::remove_file(p);
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
            // The window can be restored before this fires. A suspend that
            // lands after the resume leaves a white, hidden page, which is
            // what repeated minimize/maximize produced. Recheck the current
            // intent and abandon the stale request.
            if !WEBVIEW_LOW.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
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

#[cfg(windows)]
/// A logged-in router, kept for the session so the device list can ask it for
/// names without logging in again on every scan.
pub struct RouterState(pub tokio::sync::Mutex<Option<router_huawei::Session>>);

#[cfg(windows)]
/// The one device being carried at a time.
pub struct LanState(pub std::sync::Mutex<Option<quotacards::lan_relay::Session>>);

#[cfg(windows)]
/// The whole-network watcher, separate from the one-device session above:
/// watching does not drop or pace anything, and it covers every device at once.
pub struct MonitorState(pub std::sync::Mutex<Option<quotacards::lan_ctrl::MonitorSession>>);

#[derive(serde::Serialize)]
#[cfg(windows)]
pub struct TrafficRow {
    /// Hardware address, lower case and `:` separated. Empty when the watch
    /// could not resolve one: that bucket is keyed by its address instead.
    pub mac: String,
    pub ip: String,
    pub up: u64,
    pub down: u64,
}

/// Everything the Network screen polls, in one payload.
///
/// The screen used to make four to six separate calls per tick (`lan_guard`,
/// `pipeline_ready`, `router_status`, `lan_monitor_active`, `lan_traffic`), which
/// is why the interval could not be slowed down: each call had its own state,
/// so the roster and the rail could disagree and a rename could race a rate
/// sample. One snapshot makes them one reading, taken at one moment.
#[derive(serde::Serialize)]
#[cfg(windows)]
pub struct NetworkSnapshot {
    /// Whether anyone is trying to stand between this machine and the router.
    pub guard: quotacards::lan_ctrl::Guard,
    /// Whether the packet driver the control layer needs is installed.
    pub driver_ready: bool,
    /// Whether a claim frame can be forged, which is the only thing Npcap is
    /// still for. False means this machine can still discover, cap and block
    /// when the router points its devices here.
    pub claim_ready: bool,
    /// Whether the router is signed in, so its names are available.
    pub router_linked: bool,
    /// Every device the sweep or the router knows about.
    pub devices: Vec<quotacards::lan::Device>,
    /// The one device currently under this PC's control, if any.
    pub active: Option<LanStatus>,
    /// When this snapshot was taken, milliseconds since the Unix epoch.
    pub ts: u64,
}

/// One snapshot of the whole network surface: the guard, the drivers, the
/// router session, the roster and the one controlled device.
#[tauri::command]
#[cfg(windows)]
async fn network_snapshot(
    lan: tauri::State<'_, LanState>,
    router: tauri::State<'_, RouterState>,
    cfg: tauri::State<'_, State>,
) -> Result<NetworkSnapshot, String> {
    let guard = lan_guard();
    // Both flags answer the same question now: is the raw packet driver there?
    // The relay is the only carrier, and it runs on Npcap. The second field is
    // kept because the screen still reads it.
    let driver_ready = quotacards::lan_route::npcap_installed();
    let claim_ready = driver_ready;
    let mut devices = scan_with_router(&router, &cfg).await?;
    let active = lan_status(lan);
    // A claimed device's ARP entry now points at this PC, so the sweep drops it
    // (it never lists our own hardware address). The controlled row must stay
    // on screen: put it back from the session itself, with the saved router
    // name when there is one.
    if let Some(a) = active.as_ref() {
        let known = devices.iter().any(|d| {
            d.ip == a.ip || (!a.mac.is_empty() && d.mac.eq_ignore_ascii_case(&a.mac))
        });
        if !known {
            let names = quotacards::lan::router_names();
            let name = names
                .get(&a.mac.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default();
            devices.push(quotacards::lan::Device {
                ip: a.ip.clone(),
                mac: a.mac.clone(),
                name: if name.is_empty() {
                    String::new()
                } else {
                    name.clone()
                },
                self_pc: false,
                name_source: if name.is_empty() {
                    String::new()
                } else {
                    "router".into()
                },
                protected: false,
                protected_always: false,
                live: true,
            });
        }
    }
    let router_linked = {
        let guard = router.0.lock().await;
        guard.is_some()
    };
    // Our own claim makes the gateway's ARP entry point at this PC; that is the
    // shape the guard reports as an attack. While a session we own is active,
    // the claim is ours and the alert would be a lie.
    let mut guard = guard;
    if active.is_some() {
        guard.watching = false;
        guard.attacker_mac.clear();
        guard.attacker_ip.clear();
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok(NetworkSnapshot {
        guard,
        driver_ready,
        claim_ready,
        router_linked,
        devices,
        active,
        ts,
    })
}

/// Rebuild the running tunnel from the session record, so a claim or a release
/// changes what the engine carries. The routed device list is read from the
/// intent record inside the config builder, so the same call serves both.
#[cfg(windows)]
fn rebuild_engine_for_routed() -> Result<(), String> {
    let path = quotacards::vpn::engine_dir().join("session.json");
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let apps: Vec<String> = v
        .get("apps")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let voice = v.get("voice").and_then(|b| b.as_bool()).unwrap_or(false);
    let (wg_private, wg_addr, wg_pub, wg_params) = (
        s("wg_private"),
        s("wg_addr"),
        s("wg_server_pub"),
        s("wg_params"),
    );
    let wg = if wg_private.is_empty() {
        None
    } else {
        Some((
            wg_private.as_str(),
            wg_addr.as_str(),
            wg_pub.as_str(),
            wg_params.as_str(),
        ))
    };
    quotacards::vpn::write_tun_config(
        &s("uuid"),
        &s("host"),
        &s("sni"),
        &s("apps_mode"),
        &apps,
        voice,
        &s("transport"),
        &s("hy2_pass"),
        wg,
    )?;
    quotacards::vpn::stop_engine();
    quotacards::vpn::spawn_engine().map(|_| ())
}

/// One device routed through the tunnel with nothing installed on it.
#[derive(serde::Serialize, Clone)]
#[cfg(windows)]
struct RoutedDevice {
    ip: String,
    mac: String,
    gateway: String,
    interface: String,
}

#[cfg(windows)]
impl From<quotacards::lan_route::Intent> for RoutedDevice {
    fn from(i: quotacards::lan_route::Intent) -> RoutedDevice {
        RoutedDevice {
            ip: i.target_ip,
            mac: i.target_mac,
            gateway: i.gateway_ip,
            interface: i.interface,
        }
    }
}

/// Take one device over. Returns only once the capture is carrying it, so a
/// success here means its traffic is coming to this machine now.
#[tauri::command]
#[cfg(windows)]
async fn lan_route_device(
    ip: String,
    mac: String,
    state: tauri::State<'_, LanState>,
) -> Result<RoutedDevice, String> {
    let wire = tauri::async_runtime::spawn_blocking(quotacards::lan_ctrl::wire)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No network adapter leads to the router.")?;
    let previous = { state.0.lock().map_err(|_| "state".to_string())?.take() };
    if let Some(prev) = previous {
        prev.stop();
    }
    let session = quotacards::lan_relay::start_with(&ip, false, Some(mac))?;
    let learned = session.target_mac.clone();
    *state.0.lock().map_err(|_| "state".to_string())? = Some(session);
    Ok(RoutedDevice {
        ip,
        mac: learned,
        gateway: wire.gateway_ip,
        interface: wire.adapter,
    })
}

/// Hand it back to the router. Safe to call when nothing is routed.
#[tauri::command]
#[cfg(windows)]
async fn lan_unroute_device(state: tauri::State<'_, LanState>) -> Result<bool, String> {
    let previous = { state.0.lock().map_err(|_| "state".to_string())?.take() };
    if let Some(prev) = previous {
        prev.stop();
    }
    // release() restores the ARP entry, lifts the pin, turns forwarding off and
    // stops an engine this claim started, so nothing is rebuilt here: a rebuild
    // would put a whole-device tunnel back up after the device was let go.
    tauri::async_runtime::spawn_blocking(|| quotacards::lan_route::release().map(|_| true))
        .await
        .map_err(|e| e.to_string())?
}

/// Report every fact that decides whether routing a device can work.
#[cfg(windows)]
#[tauri::command]
async fn lan_route_diagnose(
    ip: Option<String>,
    mac: Option<String>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = match (ip, mac) {
            (Some(ip), Some(mac)) => {
                let parsed = ip.parse().map_err(|_| format!("not an address: {ip}"))?;
                Some((parsed, mac))
            }
            _ => None,
        };
        Ok::<Vec<String>, String>(quotacards::lan_route::diagnose(target))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Is one-click ARP claiming available? False means the router's DHCP has to
/// point devices at this machine (gateway mode), because there is no driver to
/// forge a claim frame with.
#[cfg(windows)]
#[tauri::command]
fn lan_claim_ready() -> bool {
    quotacards::lan_route::npcap_installed()
}

/// Download and install the packet driver on demand, then say whether it worked.
#[cfg(windows)]
#[tauri::command]
async fn lan_install_npcap() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(quotacards::lan_route::install_npcap)
        .await
        .map_err(|e| e.to_string())?
}

/// Which device is currently routed, if any, from the intention record.
#[tauri::command]
#[cfg(windows)]
fn lan_routed_device() -> Option<RoutedDevice> {
    quotacards::lan_route::read_intent().map(RoutedDevice::from)
}

/// Keep the selected space connected without the Spaces screen.
///
/// Radmin's service owns the network and the window is only a view. This is the
/// same ownership model with the app process as the owner: on launch, and every
/// few seconds after, it refreshes the roster, picks the first online member,
/// and starts the direct engine. The screen reads the status; it does not own
/// the session. A manual Stop sets the pause flag and the supervisor leaves it
/// alone until the user connects again or switches spaces.
#[cfg(windows)]
fn spawn_spaces_supervisor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Let the window, the config load and the first paint finish.
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        let mut last_space = String::new();
        let mut last_error = String::new();
        loop {
            let state = app.state::<State>();
            let direct = app.state::<spaces_direct::DirectState>();
            let active = {
                let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
                cfg.active_space.clone()
            };
            if active != last_space {
                if !last_space.is_empty() {
                    let _ = spaces_direct::stop_engine(&direct);
                }
                spaces_direct::set_paused(false);
                last_space = active.clone();
                last_error.clear();
            }
            if active.is_empty()
                || spaces_direct::paused()
                || spaces_direct::is_starting()
                || spaces_direct::running_for(&direct, &active)
            {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                continue;
            }
            if let Some(other) = spaces_direct::running_space(&direct) {
                if other != active {
                    let _ = spaces_direct::stop_engine(&direct);
                }
            }
            if let Err(e) = spaces::refresh(&*state, &active).await {
                if e != last_error {
                    quotacards::vpn::app_log(&format!("spaces supervisor: {e}"));
                    last_error = e;
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                continue;
            }
            let peer = {
                let cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
                cfg.spaces
                    .iter()
                    .find(|s| s.id == active)
                    .and_then(|s| s.members.iter().find(|m| !m.self_pc && m.online))
                    .map(|m| m.id.clone())
            };
            if let Some(peer_id) = peer {
                match spaces_direct::start(&*state, &direct, &active, &peer_id).await {
                    Ok(status) => {
                        last_error.clear();
                        quotacards::vpn::app_log(&format!(
                            "spaces supervisor: direct {} with {} ({})",
                            status.address, status.peer, status.adapter
                        ));
                    }
                    Err(e) => {
                        if e != last_error {
                            quotacards::vpn::app_log(&format!("spaces supervisor: {e}"));
                            last_error = e;
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
}

#[cfg(windows)]
fn handlers() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        setup_helper,
        get_state,
        platform,
        import_credential,
        card_status,
        forget_card,
        copy_card,
        set_card_sni,
        tunnel_start,
        tunnel_stop,
        tunnel_status,
        tunnel_health,
        tunnel_destinations,
        tunnel_traffic,
        tunnel_log,
        tunnel_copy_log,
        tunnel_probe,
        tunnel_apps,
        spaces::spaces_state,
        spaces::spaces_api,
        spaces::set_spaces_api,
        spaces::space_device_name,
        spaces::space_create,
        spaces::space_join,
        spaces::space_refresh,
        spaces::space_probe,
        spaces::space_activate,
        spaces::space_leave,
        spaces::space_delete,
        spaces::space_rotate,
        spaces::space_kick,
        spaces_direct::space_direct_start,
        spaces_direct::space_direct_stop,
        spaces_direct::space_direct_status,
        resolve_host,
        check_update,
        apply_update,
        set_update_channel,
        net_info,
        app_icons,
        speed_servers,
        pipeline_ready,
        lan_wire,
        lan_start,
        lan_stop,
        lan_limits,
        lan_block,
        lan_prepare,
        lan_status,
        lan_guard,
        lan_protect,
        lan_traffic,
        lan_monitor_active,
        network_snapshot,
        lan_monitor_stop,
        lan_monitor_start,
        router_link,
        open_page,
        router_autolink,
        router_status,
        router_clients,
        router_unlink,
        lan_known,
        lan_scan,
        copy_text,
        kill_switch_state,
        kill_switch_set,
        kill_switch_release,
        lan_route_device,
        lan_unroute_device,
        lan_routed_device,
        lan_claim_ready,
        lan_install_npcap,
        lan_route_diagnose,
        lan_devices,
        speed_cancel,
        speed_latency,
        speed_reach,
        speed_down,
        speed_up,
        speedtest_cli_ready,
        speedtest_cli,
        process_running,
        game_running,
        autostart_get,
        autostart_set,
        we_quota::we_quota_challenge,
        we_quota::we_quota_login,
        we_quota::we_quota_usage,
        we_quota::we_quota_logout
    ]
}

#[cfg(not(windows))]
fn handlers() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        setup_helper,
        get_state,
        platform,
        import_credential,
        card_status,
        forget_card,
        copy_card,
        set_card_sni,
        tunnel_start,
        tunnel_stop,
        tunnel_status,
        tunnel_health,
        tunnel_destinations,
        tunnel_traffic,
        tunnel_log,
        tunnel_copy_log,
        tunnel_probe,
        spaces::spaces_state,
        spaces::spaces_api,
        spaces::set_spaces_api,
        spaces::space_device_name,
        spaces::space_create,
        spaces::space_join,
        spaces::space_refresh,
        spaces::space_probe,
        spaces::space_activate,
        spaces::space_leave,
        spaces::space_delete,
        spaces::space_rotate,
        spaces::space_kick,
        resolve_host,
        check_update,
        apply_update,
        set_update_channel,
        net_info,
        app_icons,
        open_page,
        copy_text,
        kill_switch_state,
        kill_switch_set,
        kill_switch_release,
        autostart_get,
        autostart_set,
        we_quota::we_quota_challenge,
        we_quota::we_quota_login,
        we_quota::we_quota_usage,
        we_quota::we_quota_logout
    ]
}

pub fn run() {
    AppConfig::migrate_dirs();
    // The router password is never kept between runs. Delete the sealed file a
    // previous build wrote, so an upgrade cannot silently keep signing in.
    let _ = router_remember::clear();
    if std::env::args().any(|a| a == "--connect-bench") {
        let voice = std::env::args().any(|a| a == "--voice");
        match bench_connect(voice) {
            Ok(()) => {}
            Err(e) => {
                println!("BENCH_ERR {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    // Undo any redirection a previous run failed to clean up: a killed app
    // leaves the router believing this machine is the device it blocked.
    #[cfg(windows)]
    std::thread::spawn(|| {
        quotacards::lan_ctrl::repair();
        // A killed session can leave the gateway pinned. Remove that before
        // the page is usable; the entry is the real router, so this is cleanup
        // rather than a rescue.
        quotacards::lan_relay::repair_pin();
    });
    // Names arrive when devices join, so listen for them in the background.
    #[cfg(windows)]
    quotacards::dhcp_names::start_background();
    // Watch for anyone doing to this machine what this app does to others.
    #[cfg(windows)]
    quotacards::lan_ctrl::start_guard();

    // Headless self-test for the WE quota flow, and a diagnostic the owner
    // can run on any PC: QC_WE_NUMBER and QC_WE_PASS in the environment,
    // QuotaVPN.exe --we-probe. Prints units and PROBE_GREEN, exits nonzero
    // on any failure. Runs before the single-instance plugin on purpose.
    // Headless redirect, for proving the engine works without the UI.
    #[cfg(windows)]
    if let Some(i) = std::env::args().position(|a| a == "--lan-test") {
        let ip = std::env::args().nth(i + 1).unwrap_or_default();
        let secs: u64 = std::env::args()
            .nth(i + 2)
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);
        let log = |line: &str| {
            println!("{line}");
            let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
            let dir = std::path::Path::new(&base).join("quotavpn");
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("lan_test.log"))
            {
                use std::io::Write;
                let _ = writeln!(f, "{line}");
            }
        };
        match quotacards::lan_ctrl::wire() {
            Some(w) => log(&format!(
                "wire: {} | our {} {} | gateway {} {}",
                w.adapter, w.ip, w.mac, w.gateway_ip, w.gateway_mac
            )),
            None => log("wire: none"),
        }
        let block = std::env::args()
            .nth(i + 3)
            .map(|v| v != "0")
            .unwrap_or(false);
        let mac = std::env::args().nth(i + 4).filter(|m| !m.is_empty());
        let down: u32 = std::env::args()
            .nth(i + 5)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let up: u32 = std::env::args()
            .nth(i + 6)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        match quotacards::lan_relay::start_with(&ip, block, mac) {
            Ok(s) => {
                log(&format!(
                    "started on {} at {} (blocked={block}, down={down} KB/s, up={up} KB/s)",
                    s.target_ip, s.target_mac
                ));
                s.down_kbps.store(down, std::sync::atomic::Ordering::Relaxed);
                s.up_kbps.store(up, std::sync::atomic::Ordering::Relaxed);
                let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
                while std::time::Instant::now() < until {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    log(&format!(
                        "tick up={} down={} forwarded={} dropped={} answers={}",
                        s.bytes_up.load(std::sync::atomic::Ordering::Relaxed),
                        s.bytes_down.load(std::sync::atomic::Ordering::Relaxed),
                        s.forwarded.load(std::sync::atomic::Ordering::Relaxed),
                        s.dropped.load(std::sync::atomic::Ordering::Relaxed),
                        s.client_answers.load(std::sync::atomic::Ordering::Relaxed)
                    ));
                }
                let up_bytes = s.bytes_up.load(std::sync::atomic::Ordering::Relaxed);
                let down_bytes = s.bytes_down.load(std::sync::atomic::Ordering::Relaxed);
                let fwd = s.forwarded.load(std::sync::atomic::Ordering::Relaxed);
                let drop = s.dropped.load(std::sync::atomic::Ordering::Relaxed);
                let answers = s.client_answers.load(std::sync::atomic::Ordering::Relaxed);
                let alive = s.alive();
                s.stop();
                log(&format!(
                    "stopped: up {up_bytes} B, down {down_bytes} B, forwarded {fwd}, dropped {drop}, answers {answers}, was_alive {alive}"
                ));
            }
            Err(e) => log(&format!("start failed: {e}")),
        }
        return;
    }
    // Open the capture and hold it, without sending anything: the probe for
    // whether opening a handle disturbs this adapter.
    #[cfg(windows)]
    if let Some(i) = std::env::args().position(|a| a == "--lan-tap") {
        let secs: u64 = std::env::args()
            .nth(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let Some(w) = quotacards::lan_ctrl::wire() else {
            println!("tap: no wire");
            return;
        };
        let cap = quotacards::wpcap::with(|p| p.open_mode(&w.device, false, 20)).flatten();
        println!("tap: open={} for {secs}s", cap.is_some());
        std::thread::sleep(std::time::Duration::from_secs(secs));
        println!("tap: done");
        return;
    }
    #[cfg(windows)]
    // Print the roster the Network page would paint, through the same tidy pass.
    if std::env::args().any(|a| a == "--lan-roster") {
        let wire = quotacards::lan_ctrl::wire();
        let mut list = quotacards::lan::known();
        if let Some(w) = wire.as_ref() {
            if !list.iter().any(|d| d.ip == w.ip) {
                list.push(quotacards::lan::Device {
                    ip: w.ip.clone(),
                    mac: w.mac.clone(),
                    name: String::new(),
                    self_pc: true,
                    name_source: "self".into(),
                    protected: true,
                    protected_always: true,
                    live: true,
                });
            }
            list = quotacards::lan::tidy_roster(
                list,
                &w.ip,
                &w.mac,
                &w.gateway_ip,
                &w.gateway_mac,
            );
        }
        for d in &list {
            println!(
                "{} {} {} self={} live={}",
                d.ip, d.mac, d.name, d.self_pc, d.live
            );
        }
        return;
    }
    #[cfg(windows)]
    // Print the wire the app would use, without touching the network.
    if std::env::args().any(|a| a == "--lan-wire") {
        match quotacards::lan_ctrl::wire() {
            Some(w) => println!(
                "adapter={} ip={} mac={} gateway={} gw_mac={} device={}",
                w.adapter, w.ip, w.mac, w.gateway_ip, w.gateway_mac, w.device
            ),
            None => println!("wire: none"),
        }
        return;
    }
    // Parse a saved copy of the router's device list.
    if let Some(i) = std::env::args().position(|a| a == "--router-parse") {
        let path = std::env::args().nth(i + 1).unwrap_or_default();
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        let clients = router_huawei::parse_body(&body);
        for c in &clients {
            println!("{:<18} {:<18} {}", c.ip, c.mac, c.name);
        }
        println!("ROUTER_PARSE {} clients", clients.len());
        return;
    }
    // Ask one device its name, on the wire.
    #[cfg(windows)]
    if let Some(i) = std::env::args().position(|a| a == "--lan-ask") {
        let ip = std::env::args().nth(i + 1).unwrap_or_default();
        let mac = std::env::args().nth(i + 2).unwrap_or_default();
        let name = quotacards::lan_ask::mdns_ask(&ip, &mac, std::time::Duration::from_millis(1500));
        println!("{ip} {mac} -> {:?}", name);
        return;
    }
    // Listen for devices announcing their names.
    #[cfg(windows)]
    if let Some(i) = std::env::args().position(|a| a == "--lan-listen") {
        let secs: u64 = std::env::args()
            .nth(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        println!("listening {secs}s");
        let found = quotacards::dhcp_names::listen(secs);
        for (mac, name) in &found {
            println!("{mac} -> {name}");
        }
        println!("found {}", found.len());
        return;
    }
    // The elevated half of the LAN features. `WinDivertOpen` needs Administrator
    // and the window process deliberately runs unelevated, so the forward loop
    // lives in this pass and the app talks to it through the policy and state
    // files in the engine directory.
    #[cfg(windows)]
    if std::env::args().any(|a| a == "--lan-pipeline") {
        match quotacards::lan_forward::run() {
            Ok(()) => println!("lan pipeline ended"),
            Err(e) => {
                quotacards::vpn::app_log(&format!("lan pipeline: {e}"));
                println!("lan pipeline: {e}");
            }
        }
        return;
    }
    // Does the packet driver load, and what does it see?
    #[cfg(windows)]
    if std::env::args().any(|a| a == "--lan-driver") {
        let ok = quotacards::wpcap::ready();
        println!("driver_loaded={ok}");
        if ok {
            if let Some(devs) = quotacards::wpcap::with(|w| w.devices()) {
                for d in devs {
                    println!("{} | {}", d.name, d.description);
                }
            }
        }
        return;
    }
    // Headless credential tools. The credential is read from a file or stdin,
    // never from the command line: an argument is visible to every process on
    // the machine and lands in crash reports.
    #[cfg(windows)]
    if let Some(i) = std::env::args()
        .position(|a| a == "--qvreg-import" || a == "--qcreg-import")
    {
        let source = std::env::args().nth(i + 1).unwrap_or_default();
        let text = if source == "-" {
            use std::io::Read;
            let mut text = String::new();
            match std::io::stdin().read_to_string(&mut text) {
                Ok(_) => text,
                Err(e) => {
                    println!("IMPORT_ERR read stdin: {e}");
                    return;
                }
            }
        } else if !source.is_empty()
            && !source.starts_with("qv1:")
            && !source.starts_with("qc1:")
        {
            match std::fs::read_to_string(&source) {
                Ok(text) => text,
                Err(e) => {
                    println!("IMPORT_ERR read {source}: {e}");
                    return;
                }
            }
        } else {
            println!("IMPORT_ERR pass a file path or - for stdin, never the credential on the command line");
            return;
        };
        match credentials::import_into_config(&text, None) {
            Ok(uuid) => println!("IMPORT_OK {uuid}"),
            Err(e) => println!("IMPORT_ERR {e}"),
        }
        return;
    }
    #[cfg(windows)]
    if let Some(i) = std::env::args()
        .position(|a| a == "--qvreg-status" || a == "--qcreg-status")
    {
        let uuid = std::env::args()
            .nth(i + 1)
            .filter(|value| !value.is_empty())
            .or_else(|| credentials::uuids().into_iter().next());
        let Some(uuid) = uuid else {
            println!("STATUS_ERR no card credential imported");
            return;
        };
        println!("CARD {uuid}");
        match cardapi::call_blocking("status", &uuid) {
            Ok(v) => println!("STATUS_OK {v}"),
            Err(e) => println!("STATUS_ERR {e}"),
        }
        return;
    }
    #[cfg(windows)]
    if std::env::args().any(|a| a == "--dpapi-selftest") {
        match credentials::selftest() {
            Ok(()) => println!("DPAPI_OK"),
            Err(e) => {
                println!("DPAPI_ERR {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if let Some(i) = std::env::args()
        .position(|a| a == "--qvreg-forget" || a == "--qcreg-forget")
    {
        let uuid = std::env::args().nth(i + 1).unwrap_or_default();
        match quotacards::qvreg::canonical_uuid(&uuid) {
            Some(uuid) => {
                let _ = credentials::forget(&uuid);
                let mut cfg = AppConfig::load();
                cfg.cards.retain(|card| card.uuid != uuid);
                cfg.save();
                println!("FORGET_OK {uuid}");
            }
            None => println!("FORGET_ERR bad card id"),
        }
        return;
    }
    #[cfg(windows)]
    if let Some(i) = std::env::args().position(|a| a == "--stage") {
        let host = std::env::args().nth(i + 1).unwrap_or_default();
        let port: u16 = std::env::args()
            .nth(i + 2)
            .and_then(|v| v.parse().ok())
            .unwrap_or(443);
        let log = std::env::args()
            .nth(i + 3)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let stage = quotacards::vpn::stage_of(&host, port, &log);
        println!("STAGE {:?}", stage);
        println!("WORDS {}", quotacards::vpn::stage_words(stage));
        return;
    }
    if let Some(i) = std::env::args().position(|a| a == "--lan-monitor") {
        let secs: u64 = std::env::args()
            .nth(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        // Every address the neighbour table knows, so the watch has the same
        // roster the page would hand it.
        let out = std::process::Command::new("arp").arg("-a").output();
        let mut targets: Vec<(String, String)> = Vec::new();
        if let Ok(o) = out {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() >= 2 && f[0].starts_with("192.168.") && f[1].contains('-') {
                    targets.push((f[0].to_string(), f[1].replace('-', ":").to_lowercase()));
                }
            }
        }
        println!("MONITOR targets={}", targets.len());
        match quotacards::lan_ctrl::start_monitor(targets) {
            Ok(session) => {
                let started = std::time::Instant::now();
                while started.elapsed() < std::time::Duration::from_secs(secs) {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let b = session.bytes.lock().map(|m| m.len()).unwrap_or(0);
                    println!("MONITOR alive {}s buckets={b}", started.elapsed().as_secs());
                }
                session
                    .stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                println!("MONITOR done");
            }
            Err(e) => println!("MONITOR_ERR {e}"),
        }
        return;
    }
    if let Some(i) = std::env::args().position(|a| a == "--dhcp-serve") {
        let secs: u64 = std::env::args()
            .nth(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let mac = std::env::args().nth(i + 2).unwrap_or_default();
        let ip = std::env::args().nth(i + 3).unwrap_or_default();
        let mut routed = std::collections::HashMap::new();
        if let (Ok(addr), true) = (ip.parse::<std::net::Ipv4Addr>(), !mac.is_empty()) {
            if let Some(m) = quotacards::lan_route::mac_bytes(&mac) {
                routed.insert(m, addr);
            }
        }
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        let s2 = stop.clone();
        std::thread::spawn(move || {
            while std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            s2.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        match quotacards::dhcp::serve(routed, stop, |line| println!("{line}")) {
            Ok(()) => println!("DHCP_DONE"),
            Err(e) => println!("DHCP_ERR {e}"),
        }
        return;
    }
    if let Some(i) = std::env::args().position(|a| a == "--lan-claim") {
        let ip = std::env::args().nth(i + 1).unwrap_or_default();
        let mac = std::env::args().nth(i + 2).unwrap_or_default();
        if ip.parse::<std::net::Ipv4Addr>().is_err() {
            println!("CLAIM_ERR not an address: {ip}");
            return;
        }
        let known = if mac.is_empty() { None } else { Some(mac) };
        match quotacards::lan_relay::start_with(&ip, false, known) {
            Ok(s) => {
                println!("CLAIM_OK {ip} RELAY_OK at {}", s.target_mac);
                std::thread::sleep(std::time::Duration::from_secs(12));
                println!(
                    "RELAY forwarded={} dropped={} up={} down={}",
                    s.forwarded.load(std::sync::atomic::Ordering::Relaxed),
                    s.dropped.load(std::sync::atomic::Ordering::Relaxed),
                    s.bytes_up.load(std::sync::atomic::Ordering::Relaxed),
                    s.bytes_down.load(std::sync::atomic::Ordering::Relaxed)
                );
                s.stop();
            }
            Err(e) => println!("CLAIM_ERR {e}"),
        }
        return;
    }
    #[cfg(windows)]
    if std::env::args().any(|a| a == "--lan-probe") {
        let started = std::time::Instant::now();
        let devices = quotacards::lan::scan();
        for d in &devices {
            // The same label the app shows, including the port fingerprint.
            let kind = quotacards::fingerprint::kind(&d.ip);
            let label = quotacards::fingerprint::describe(&d.mac, &d.ip, &d.name, kind);
            println!(
                "{:<16} {:<18} {}  [{}]",
                d.ip,
                d.mac,
                label,
                kind.unwrap_or("-")
            );
        }
        println!(
            "LAN_PROBE {} devices in {} ms",
            devices.len(),
            started.elapsed().as_millis()
        );
        return;
    }
    if std::env::args().any(|a| a == "--we-probe") {
        we_quota::probe_main();
        return;
    }
    // The elevated pass: fetch the engine and wintun if this machine has never
    // run them, then register the helper task and leave. This is the only
    // moment the product asks Windows for administrator, once per machine.
    // Answers "does this account already have its helper task", so the
    // installer only asks Windows for administrator when it actually has to.
    // What the update feed would offer, without a window. Used to prove that
    // an install left behind for months jumps straight to the newest build
    // instead of stepping through every release in between.
    if std::env::args().any(|a| a == "--update-check") {
        let args: Vec<String> = std::env::args().collect();
        let flag = |name: &str| -> Option<String> {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let channel = flag("--channel").unwrap_or_else(|| "normal".to_string());
        let current = flag("--as-version").unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        let stable_feed =
            "https://github.com/YousefMohiey/QuotaVPN/releases/latest/download/latest.json"
                .to_string();
        let url = if channel == "experimental" {
            tauri::async_runtime::block_on(experimental_feed_url()).unwrap_or(stable_feed)
        } else {
            stable_feed
        };
        let offered = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("QuotaVPN")
            .build()
            .ok()
            .and_then(|c| {
                tauri::async_runtime::block_on(async {
                    c.get(&url).send().await.ok()?.text().await.ok()
                })
            })
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| {
                v.get("version")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string())
            });
        match offered {
            Some(v) => {
                println!(
                    "channel={channel} current={current} offered={v} newer={}",
                    newer(&v, &current)
                );
                println!("feed={url}");
            }
            None => println!("channel={channel} current={current} offered=none feed={url}"),
        }
        return;
    }
    if std::env::args().any(|a| a == "--check-helper") {
        // Both passes, so an upgrade from a build that had only the tunnel one
        // runs the setup again and registers the network pass.
        let ready = vpn::task_installed() && vpn::lan_task_installed();
        std::process::exit(if ready { 0 } else { 3 });
    }
    // Remove this account's helper task (called by the uninstaller while the
    // exe still exists, because the task needs the name this account owns).
    if std::env::args().any(|a| a == "--remove-helper") {
        vpn::remove_task();
        return;
    }
    // The helper task's own body: start the engine hidden and stay alive until
    // it is stopped. Runs as a GUI binary, so Windows draws no console window.
    // One shot, run elevated by the app: put the outbound block on or take it
    // off. This is the only way the block can be lifted without the engine.
    if std::env::args().any(|a| a == "--killswitch") {
        let args: Vec<String> = std::env::args().collect();
        let mode = args
            .iter()
            .position(|a| a == "--killswitch")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default();
        // This pass runs elevated and hidden, so its exit code is the only
        // report anyone gets. Non-zero when the state did not actually change.
        if let Err(e) = vpn::block_traffic(mode == "on") {
            eprintln!("killswitch: {e}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|a| a == "--run-engine") {
        vpn::run_engine_headless();
        return;
    }
    if std::env::args().any(|a| a == "--setup-helper") {
        // A standard user elevating with an administrator's credentials runs
        // this pass as the ADMIN, so the account that asked is passed in
        // explicitly; guessing would register the helper for the wrong person.
        let args: Vec<String> = std::env::args().collect();
        let owner = args
            .iter()
            .position(|a| a == "--as")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(vpn::account_name);
        // This pass runs hidden, so its console goes nowhere. Every step is
        // written to the engine log, because a silent failure here is
        // indistinguishable from a refused prompt in the window that asked.
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        vpn::app_log(&format!(
            "setup-helper: start owner={owner} elevated={} exe={exe}",
            vpn::is_elevated()
        ));
        // The task first. It takes a second, and it is what the window and the
        // installer wait for; the engine is a download and can finish after
        // they have moved on.
        match vpn::install_task(&owner) {
            Ok(()) => {
                vpn::app_log("setup-helper: task installed");
                println!("helper installed");
            }
            Err(e) => {
                vpn::app_log(&format!("setup-helper: task failed: {e}"));
                eprintln!("helper failed: {e}");
            }
        }
        // The second elevated pass: the LAN pipeline. Registered here so a
        // watch or a block never asks Windows for administrator.
        match vpn::install_lan_task(&owner) {
            Ok(()) => vpn::app_log("setup-helper: network task installed"),
            Err(e) => vpn::app_log(&format!("setup-helper: network task failed: {e}")),
        }
        // The packet driver is neither shipped nor fetched. Downloading a kernel
        // driver and loading it at runtime is the shape antivirus engines treat as
        // a trojan, and it was getting this app quarantined on install. The LAN
        // features now need the driver placed deliberately (see notes/driver.md);
        // without it they report that it is missing and everything else works.
        vpn::app_log("setup-helper: the packet driver is not installed by this app");
        match vpn::ensure_engine() {
            Ok(msg) => vpn::app_log(&format!("setup-helper: engine {msg}")),
            Err(e) => vpn::app_log(&format!("setup-helper: engine failed: {e}")),
        }
        return;
    }
    // A record on disk means a previous run owed a restore: a device may still believe
    // this machine is its gateway, so unwind that before the UI can ask anything. Windows
    // only, because the module it lives in does not exist anywhere else.
    #[cfg(windows)]
    if let Some(msg) = quotacards::lan_route::recover() {
        println!("lan_route: {msg}");
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
        .manage(Lifecycle(tokio::sync::Mutex::new(())))
        .manage(Dests(Mutex::new(vpn::DestTracker::new())))
        .manage(we_quota::WeQuota::new())
        .setup(|app| {
            // A block rule outlives a crash by design: it lives in Windows, not
            // in this process. Start-up is therefore the one moment a leftover
            // block can be found before the user is left staring at a machine
            // with no internet and no obvious cause. Runs inline, before any
            // state is managed, so nothing can read the switch state while a
            // stale block is still in place.
            //
            // Try in-process first: a build that happens to hold an
            // administrator token lifts the rule without a prompt. It usually
            // does not, and deleting a firewall rule needs one, so when the
            // rule survives that attempt, ask for elevation through the same
            // one-shot pass the release button uses. A crashed session must not
            // leave the machine offline until the user finds that button.
            //
            // It has to sit inside THIS closure: Tauri holds a single setup
            // slot, and a second .setup() silently replaces the whole closure
            // (see the memory-thread note further down).
            #[cfg(windows)]
            {
                if vpn::reconcile_kill_switch() == vpn::Reconcile::Release && vpn::traffic_blocked()
                {
                    let _ = release_block_elevated();
                }
            }
            #[cfg(windows)]
            {
                app.manage(LanState(std::sync::Mutex::new(None)));
                app.manage(MonitorState(std::sync::Mutex::new(None)));
                app.manage(RouterState(tokio::sync::Mutex::new(None)));
                app.manage(spaces_direct::DirectState(std::sync::Mutex::new(None)));
            }
            // Same OS config dir as the previous desktop app: cards carry
            // over, nothing to re-enter. (No chdir - that is phone-only.)
            let mut cfg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(initial_config))
                .unwrap_or_default();
            // Older test builds used QC_SPACES_API. Adopt it once so the app
            // keeps working after the variable is gone, and the address can be
            // changed from the UI instead.
            if cfg.spaces_api.is_empty() {
                if let Ok(env) = std::env::var("QC_SPACES_API") {
                    let clean = env.trim().trim_end_matches('/').to_string();
                    if !clean.is_empty() {
                        cfg.spaces_api = clean;
                        cfg.save();
                    }
                }
            }
            spaces::init_api(&cfg);
            *app.state::<State>().0.lock().unwrap() = cfg;
            // The space stays connected after the window hides or the Spaces
            // screen is left, the way Radmin's service keeps its network up.
            #[cfg(windows)]
            spawn_spaces_supervisor(app.handle().clone());
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
            // The Valorant watcher: an OS thread, never a webview timer.
            #[cfg(windows)]
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
        .invoke_handler(handlers())
        .build(tauri::generate_context!())
        .expect("QuotaVPN failed to start")
        .run(|app, event| {
            // Closing the app must hand the network back. Without this, a
            // blocked device stays cut off until its router entry expires.
            if let tauri::RunEvent::Exit = event {
                #[cfg(windows)]
                if let Some(state) = app.try_state::<LanState>() {
                    if let Ok(mut guard) = state.0.lock() {
                        if let Some(session) = guard.take() {
                            session.stop();
                        }
                    }
                }
                #[cfg(windows)]
                if let Some(state) = app.try_state::<spaces_direct::DirectState>() {
                    spaces_direct::stop_now(&state);
                }
            }
        });
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
            let rows =
                std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
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
        assert!(!super::process_running(
            "quotavpn-definitely-not-a-process-xyz.exe".into()
        ));
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

    #[test]
    fn prerelease_sorts_below_the_release() {
        assert!(super::newer("0.7.6-exp.1", "0.7.5"));
        assert!(super::newer("0.7.6", "0.7.6-exp.1"));
        assert!(!super::newer("0.7.6-exp.1", "0.7.6"));
        assert!(super::newer("0.7.6-exp.2", "0.7.6-exp.1"));
        assert!(!super::newer("0.7.6-exp.1", "0.7.6-exp.2"));
        assert!(super::newer("0.7.10", "0.7.9"));
        assert!(!super::newer("0.7.6", "0.7.6"));
    }
}
