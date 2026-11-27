//! The QuotaSpace LAN, carried by the bundled engine.
//!
//! One press starts the engine, points it at our coordination server,
//! registers this PC once, starts the service and joins the selected space's
//! own OpenRad network. Each space is a separate network, so its peers and
//! broadcast domain never mix with another space.

use quotacards::config::{AppConfig, LanSpace};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SERVER: &str = "80.225.89.162";
const PORT: &str = "17301";

fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default()
}

fn exe() -> PathBuf {
    install_dir().join("openrad").join("QuotaSpace.exe")
}

fn modulus() -> PathBuf {
    install_dir().join("openrad").join("modulus.bin")
}

fn run_raw(args: &[&str]) -> Result<String, String> {
    let path = exe();
    let output = Command::new(&path)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("the LAN engine could not start: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(detail.to_string());
    }
    Ok(stdout)
}

fn run_json(args: &[&str]) -> Result<Value, String> {
    let text = run_raw(args)?;
    let line = text
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .ok_or("the LAN engine returned an empty answer")?;
    let value: Value =
        serde_json::from_str(line.trim()).map_err(|e| format!("the LAN engine answer is not JSON: {e}"))?;
    if value.get("ok").and_then(Value::as_bool) == Some(false) {
        return Err(value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the LAN engine refused the command")
            .to_string());
    }
    Ok(value.get("data").cloned().unwrap_or(Value::Null))
}

/// The bundled engine looks for an adapter named exactly `QuotaSpace`.
fn ensure_adapter() -> Result<(), String> {
    let tap = install_dir().join("tap").join("amd64");
    let devcon = tap.join("devcon.exe");
    let inf = tap.join("OemVista.inf");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         if (Get-NetAdapter -Name 'QuotaSpace') {{ exit 0 }}; \
         if (Get-NetAdapter -Name 'OpenRad') {{ Rename-NetAdapter -Name 'OpenRad' -NewName 'QuotaSpace' }}; \
         if (Get-NetAdapter -Name 'QuotaSpace') {{ exit 0 }}; \
         & '{}' install '{}' TAP0901 | Out-Null; \
         Start-Sleep -Milliseconds 900; \
         $n = Get-NetAdapter | Where-Object {{ $_.InterfaceDescription -like '*TAP-Windows*' -and $_.Name -ne 'QuotaSpaceLAN' -and $_.Name -ne 'OpenRad' -and $_.Name -ne 'QuotaSpace' }} | Sort-Object ifIndex | Select-Object -Last 1; \
         if ($n) {{ Rename-NetAdapter -Name $n.Name -NewName 'QuotaSpace' }}; \
         if (-not (Get-NetAdapter -Name 'QuotaSpace')) {{ exit 1 }}",
        devcon.to_string_lossy().replace('\'', "''"),
        inf.to_string_lossy().replace('\'', "''")
    );
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("the LAN adapter could not be prepared: {e}"))?;
    if !output.status.success() {
        return Err("the LAN adapter could not be prepared; reinstall QuotaVPN".into());
    }
    if !install_dir().join("tap").join("amd64").exists() {
        return Err("the LAN driver payload is missing; reinstall QuotaVPN".into());
    }
    Ok(())
}

fn password_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "quotaspace-{}.txt",
        &uuid_like()
    ))
}

/// A temp-file name with no dependency on a random crate in this binary.
fn uuid_like() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{now:x}", std::process::id())
}

fn join_network(space: &LanSpace) -> Result<(), String> {
    if space.password.is_empty() {
        run_json(&["join", space.network.as_str(), "--json"])?;
        return Ok(());
    }
    let path = password_path();
    std::fs::write(&path, &space.password)
        .map_err(|e| format!("the LAN password file could not be written: {e}"))?;
    let file = path.to_string_lossy().to_string();
    let result = run_json(&["join", space.network.as_str(), "--password-file", &file, "--json"]);
    let _ = std::fs::remove_file(&path);
    result.map(|_| ())
}

fn create_network(space: &LanSpace) -> Result<(), String> {
    let path = password_path();
    std::fs::write(&path, &space.password)
        .map_err(|e| format!("the LAN password file could not be written: {e}"))?;
    let file = path.to_string_lossy().to_string();
    let result = run_json(&["create", space.network.as_str(), "--password-file", &file, "--json"]);
    let _ = std::fs::remove_file(&path);
    result.map(|_| ())
}

fn joined_roles() -> Vec<(String, String, Option<u64>)> {
    let Ok(data) = run_json(&["networks", "--json"]) else {
        return Vec::new();
    };
    data.as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let id = row.get("id").and_then(Value::as_str)?.to_string();
                    let name = row.get("name").and_then(Value::as_str)?.to_string();
                    let role = row.get("role").and_then(Value::as_u64);
                    Some((id, name, role))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn joined_networks() -> Vec<(String, String)> {
    joined_roles()
        .into_iter()
        .map(|(id, name, _)| (id, name))
        .collect()
}

/// Only one network may be attached at a time. OpenRad's forwarding table puts
/// every eligible peer of every joined network in the same broadcast domain,
/// so joining two spaces would leak a LAN game from one into the other.
fn leave_others(network: &str) {
    for (_, name) in joined_networks() {
        if name != network {
            let _ = run_json(&["leave", name.as_str(), "--json"]);
        }
    }
}

/// Keep the engine's diagnostics from growing without bound. The daemon is
/// stopped before this runs, so renaming an open log is safe.
fn trim_logs() {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let base = PathBuf::from(local);
    for path in [
        base.join("openrad").join("logs").join("cli-startup.log"),
        base.join("openrad").join("logs").join("cli-daemon-startup.log"),
        base.join("openrad").join("diagnostics").join("connection.jsonl"),
        base.join("quotavpn").join("engine").join("singbox.log"),
    ] {
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.len() <= 5 * 1024 * 1024 {
            continue;
        }
        let old = path.with_extension("old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&path, &old);
    }
}

/// Configure the server and start the engine without joining a network. The
/// caller then joins exactly one space network.
fn prepare_engine() -> Result<(), String> {
    ensure_adapter()?;
    let _ = run_raw(&["stop"]);
    trim_logs();
    clear_stale_address();
    clear_legacy_space_address();
    let modulus = modulus();
    if !modulus.exists() {
        return Err("the LAN server key is missing; reinstall QuotaVPN".into());
    }
    run_json(&[
        "server",
        "set",
        "--host",
        SERVER,
        "--port",
        PORT,
        "--modulus",
        &modulus.to_string_lossy(),
        "--json",
    ])?;
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "QuotaVPN-PC".into());
    // An identity already exists after the first run; that answer is fine.
    let _ = run_json(&["init", "--node-name", name.as_str(), "--json"]);
    run_raw(&["start"])?;
    Ok(())
}

/// Leave the current networks and refuse to continue if any other network is
/// still attached. OpenRad puts every joined network in one forwarding table,
/// so a failed leave with a successful join would mix two spaces. Stopping the
/// engine is safer than pretending they are isolated.
fn leave_check(network: &str) -> Result<(), String> {
    leave_others(network);
    // The leave command returns before the local network list always drops the
    // old name. Give it a moment; stopping too early aborts a healthy switch.
    for _ in 0..15 {
        if !joined_networks()
            .iter()
            .any(|(_, name)| name != network)
        {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = run_raw(&["stop"]);
    Err(
        "the previous space could not be left; the LAN was stopped to keep the spaces separate"
            .into(),
    )
}

/// Create a space that has never been provisioned, or join one that exists.
/// A private network's create call is the only way to put it on the server;
/// an existing one falls back to join with the same password.
fn join_or_create(space: &LanSpace) -> Result<(), String> {
    if !space.provisioned {
        if create_network(space).is_err() {
            join_network(space)?;
        }
    } else {
        join_network(space)?;
    }
    let joined = joined_networks();
    if !joined.iter().any(|(_, name)| name == &space.network) {
        return Err("the space did not attach to the LAN engine".into());
    }
    if joined.iter().any(|(_, name)| name != &space.network) {
        let _ = run_raw(&["stop"]);
        return Err("more than one space is attached; the LAN was stopped".into());
    }
    Ok(())
}

pub fn connect_blocking(space: &LanSpace) -> Result<Value, String> {
    prepare_engine()?;
    leave_check(&space.network)?;
    join_or_create(space)?;
    run_json(&["status", "--json"])
}

pub fn switch_blocking(space: &LanSpace) -> Result<Value, String> {
    let status = run_json(&["status", "--json"]).unwrap_or(Value::Null);
    let connected = status.get("phase").and_then(Value::as_str) == Some("connected");
    if !connected {
        return connect_blocking(space);
    }
    leave_check(&space.network)?;
    join_or_create(space)?;
    run_json(&["status", "--json"])
}

/// Resolve a space for a headless run. The full app always has the default
/// record from `initial_config`; this fallback covers a bare profile.
pub fn configured_space(id: Option<&str>) -> Result<LanSpace, String> {
    let cfg = AppConfig::load();
    let fallback;
    let spaces = if cfg.lan_spaces.is_empty() {
        fallback = vec![LanSpace::default_space()];
        &fallback
    } else {
        &cfg.lan_spaces
    };
    let space = match id {
        Some(id) => spaces
            .iter()
            .find(|space| space.id == id)
            .cloned()
            .ok_or_else(|| "that QuotaSpace space is not configured".to_string()),
        None => spaces
            .first()
            .cloned()
            .ok_or_else(|| "no QuotaSpace space is configured".to_string()),
    }?;
    unseal_space(space)
}

fn view(space: &LanSpace) -> Value {
    json!({
        "id": space.id,
        "name": space.name,
        "network": space.network,
        "owner": space.owner,
        "default": space.is_default(),
        "invite": space.invite(),
    })
}

/// A previous engine instance can leave its `26.x` address on the adapter.
/// The engine refuses to start while another interface holds the `26/8`, and
/// its own stale address counts. The old name is cleared too, so an adapter
/// that has not been renamed yet cannot keep the address.
fn clear_stale_address() {
    let script = "$ErrorActionPreference='SilentlyContinue'; \
         foreach ($name in @('QuotaSpace','OpenRad')) { \
           Get-NetIPAddress -InterfaceAlias $name -AddressFamily IPv4 | \
           Where-Object { $_.IPAddress -like '26.*' } | \
           Remove-NetIPAddress -Confirm:$false \
         }";
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// The first layer-3 test space can leave a second private address on
/// `QuotaSpaceLAN`. The shipping LAN does not use that adapter, so clear it
/// when the LAN takes over and once at launch instead of showing two
/// addresses at once. The adapter is disabled as well: Windows hands an
/// enabled but disconnected TAP adapter a 169.254 link-local address, which
/// still reads as a second IP. This never touches a real space session.
pub(crate) fn clear_legacy_space_address() {
    let script = "$ErrorActionPreference='SilentlyContinue'; \
         Get-NetIPAddress -InterfaceAlias 'QuotaSpaceLAN' -AddressFamily IPv4 | \
         Where-Object { $_.IPAddress -like '10.77.*' -or $_.IPAddress -like '169.254.*' } | \
         Remove-NetIPAddress -Confirm:$false; \
         Disable-NetAdapter -Name 'QuotaSpaceLAN' -Confirm:$false";
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

fn unseal_space(mut space: LanSpace) -> Result<LanSpace, String> {
    if space.password.is_empty() && !space.password_sealed.is_empty() {
        space.password = crate::credentials::open_text(&space.password_sealed)?;
    }
    Ok(space)
}

fn seal_space(space: &mut LanSpace) -> Result<(), String> {
    if !space.password.is_empty() {
        space.password_sealed = crate::credentials::seal_text(&space.password)?;
        space.password.clear();
    }
    Ok(())
}

fn space_from_state(state: &crate::State, id: Option<&str>) -> Result<LanSpace, String> {
    let cfg = state.0.lock().unwrap();
    let space = match id {
        Some(id) => cfg
            .lan_spaces
            .iter()
            .find(|space| space.id == id)
            .cloned()
            .ok_or_else(|| "that QuotaSpace space is not configured".to_string()),
        None => cfg
            .lan_spaces
            .first()
            .cloned()
            .ok_or_else(|| "no QuotaSpace space is configured".to_string()),
    }?;
    unseal_space(space)
}

/// The server's role field is 2 for the admin and 1 for a member. An owner
/// who leaves a space with other members hands admin to one of them, so the
/// local crown and the delete action have to follow the server, not the old
/// record.
fn sync_space_role(state: &crate::State, space: &LanSpace) {
    // The baseline space has no server admin when it is public; once it is
    // private the creator is admin. Keep its owner flag as the app set it.
    if space.is_default() {
        return;
    }
    let role = run_json(&["networks", "--json"]).ok().and_then(|data| {
        data.as_array()?.iter().find_map(|row| {
            if row.get("name").and_then(Value::as_str) != Some(space.network.as_str()) {
                return None;
            }
            row.get("role").and_then(Value::as_u64)
        })
    });
    let Some(role) = role else { return };
    let owner = role == 2;
    let mut cfg = state.0.lock().unwrap();
    if let Some(row) = cfg.lan_spaces.iter_mut().find(|row| row.id == space.id) {
        if row.owner != owner {
            row.owner = owner;
            cfg.save();
        }
    }
}

fn mark_provisioned(state: &crate::State, space: &LanSpace) {
    let mut cfg = state.0.lock().unwrap();
    if let Some(row) = cfg.lan_spaces.iter_mut().find(|row| row.id == space.id) {
        if !row.provisioned {
            row.provisioned = true;
            cfg.save();
        }
    }
}

#[tauri::command]
pub async fn openrad_spaces(
    state: tauri::State<'_, crate::State>,
) -> Result<Vec<Value>, String> {
    let joined = tauri::async_runtime::spawn_blocking(joined_roles)
        .await
        .unwrap_or_default();
    let mut cfg = state.0.lock().unwrap();
    let mut changed = false;
    for (_, name, role) in &joined {
        if let Some(row) = cfg.lan_spaces.iter_mut().find(|space| &space.network == name) {
            if !row.provisioned {
                row.provisioned = true;
                changed = true;
            }
            if !row.is_default() {
                if let Some(role) = role {
                    let owner = *role == 2;
                    if row.owner != owner {
                        row.owner = owner;
                        changed = true;
                    }
                }
            }
        }
    }
    if changed {
        cfg.save();
    }
    Ok(cfg
        .lan_spaces
        .iter()
        .map(|space| {
            let plain = unseal_space(space.clone()).unwrap_or_else(|_| space.clone());
            view(&plain)
        })
        .collect())
}

#[tauri::command]
pub async fn openrad_connect(
    state: tauri::State<'_, crate::State>,
    space_id: Option<String>,
) -> Result<Value, String> {
    let space = space_from_state(&state, space_id.as_deref())?;
    let target = space.clone();
    let result = tauri::async_runtime::spawn_blocking(move || connect_blocking(&space))
        .await
        .map_err(|e| e.to_string())?;
    if result.is_ok() {
        mark_provisioned(&state, &target);
        sync_space_role(&state, &target);
    }
    result
}

#[tauri::command]
pub async fn openrad_switch(
    state: tauri::State<'_, crate::State>,
    space_id: String,
) -> Result<Value, String> {
    let space = space_from_state(&state, Some(&space_id))?;
    let target = space.clone();
    let result = tauri::async_runtime::spawn_blocking(move || switch_blocking(&space))
        .await
        .map_err(|e| e.to_string())?;
    if result.is_ok() {
        mark_provisioned(&state, &target);
        sync_space_role(&state, &target);
    }
    result
}

#[tauri::command]
pub async fn openrad_create_space(
    state: tauri::State<'_, crate::State>,
    name: String,
) -> Result<Value, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("give the space a name".into());
    }
    if name.chars().count() > 40 {
        return Err("the space name is too long".into());
    }
    let mut space = LanSpace::new(&name);
    let created = space.clone();
    let mut saved = space.clone();
    seal_space(&mut saved)?;
    saved.provisioned = true;
    space.provisioned = true;
    // Start the engine without joining the baseline first, then create the
    // requested network. A failed create leaves no local record and no orphan.
    tauri::async_runtime::spawn_blocking(move || {
        let status = run_json(&["status", "--json"]).unwrap_or(Value::Null);
        if status.get("phase").and_then(Value::as_str) != Some("connected") {
            prepare_engine()?;
        }
        create_network(&created)
    })
    .await
    .map_err(|e| e.to_string())??;
    // The network exists and create has already joined it. Save the record
    // before the switch cleanup, so a later leave failure cannot lose the
    // password or the invite.
    {
        let mut cfg = state.0.lock().unwrap();
        match cfg.lan_spaces.iter_mut().find(|row| row.id == saved.id) {
            Some(row) => *row = saved.clone(),
            None => cfg.lan_spaces.push(saved.clone()),
        }
        cfg.save();
    }
    let switch = space.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        leave_check(&switch.network)?;
        join_or_create(&switch)?;
        run_json(&["status", "--json"])
    })
    .await
    .map_err(|e| e.to_string())?;
    // The record is saved. Return it even if the cleanup failed so the user
    // can see the space and retry from the card instead of creating a second.
    let _ = result;
    Ok(view(&space))
}

#[tauri::command]
pub async fn openrad_join_space(
    state: tauri::State<'_, crate::State>,
    invite: String,
) -> Result<Value, String> {
    let parsed = LanSpace::from_invite(&invite)?;
    let existing = {
        let cfg = state.0.lock().unwrap();
        cfg.lan_spaces.iter().find(|space| space.id == parsed.id).cloned()
    };
    let mut space = match existing {
        Some(mut space) => {
            // The code is the authority for the password; the local name and
            // ownership stay as this PC already had them.
            space.password = parsed.password;
            space
        }
        None => parsed,
    };
    space.provisioned = true;
    let mut saved = space.clone();
    seal_space(&mut saved)?;
    saved.provisioned = true;
    let join = space.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = run_json(&["status", "--json"]).unwrap_or(Value::Null);
        if status.get("phase").and_then(Value::as_str) != Some("connected") {
            prepare_engine()?;
        }
        join_network(&join)
    })
    .await
    .map_err(|e| e.to_string())??;
    // The join succeeded. Save it before any cleanup so the space survives a
    // failed leave or status read.
    {
        let mut cfg = state.0.lock().unwrap();
        match cfg.lan_spaces.iter_mut().find(|row| row.id == saved.id) {
            Some(row) => *row = saved.clone(),
            None => cfg.lan_spaces.push(saved.clone()),
        }
        cfg.save();
    }
    let switch = space.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        leave_check(&switch.network)?;
        join_or_create(&switch)?;
        run_json(&["status", "--json"])
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = result;
    Ok(view(&space))
}

#[tauri::command]
pub async fn openrad_delete_space(
    state: tauri::State<'_, crate::State>,
    id: String,
) -> Result<(), String> {
    let space = {
        let cfg = state.0.lock().unwrap();
        cfg.lan_spaces
            .iter()
            .find(|space| space.id == id)
            .cloned()
            .ok_or_else(|| "that QuotaSpace space is not configured".to_string())?
    };
    let space = unseal_space(space)?;
    if space.is_default() {
        return Err("the default space cannot be removed".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if space.owner {
            if !space.provisioned {
                // The network was never created on the server; forgetting the
                // local record is the whole delete.
                return Ok(());
            }
            let status = run_json(&["status", "--json"]).unwrap_or(Value::Null);
            if status.get("phase").and_then(Value::as_str) != Some("connected") {
                prepare_engine()?;
            }
            leave_check(&space.network)?;
            if !joined_networks()
                .iter()
                .any(|(_, name)| name == &space.network)
            {
                join_or_create(&space)?;
            }
            // `delete` by name is not reliable for a private network; the id
            // from `networks` is the address the server expects.
            let target = joined_networks()
                .iter()
                .find(|(_, name)| name == &space.network)
                .map(|(network_id, _)| network_id.clone())
                .ok_or("the space is not attached to the LAN engine")?;
            run_json(&["delete", target.as_str(), "--yes", "--json"])?;
        } else {
            let status = run_json(&["status", "--json"]).unwrap_or(Value::Null);
            if status.get("phase").and_then(Value::as_str) == Some("connected") {
                run_json(&["leave", space.network.as_str(), "--json"])?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        let mut cfg = state.0.lock().unwrap();
        cfg.lan_spaces.retain(|row| row.id != id);
        cfg.save();
    }
    result
}

#[tauri::command]
pub async fn openrad_disconnect() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        run_raw(&["stop"])?;
        clear_stale_address();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn openrad_status() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| run_json(&["status", "--json"]))
        .await
        .map_err(|e| e.to_string())?
}

/// A connected roster for the selected space. The engine also lists identities
/// that are no longer this PC and peers that are offline; neither belongs on
/// the card. The network id filter keeps a stale peer from another space out
/// even if the engine still has it joined for a moment during a switch. RTT is
/// measured only for connected peers, one small authenticated keepalive each.
#[tauri::command]
pub async fn openrad_peers() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let status = run_json(&["status", "--json"])?;
        let own_rid = status.get("rid").and_then(Value::as_u64);
        let joined: Vec<String> = status
            .pointer("/snapshot/networks")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        row.get("network_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let peers = run_json(&["peers", "--json"])?;
        if joined.is_empty() {
            // No joined network means no valid roster. Returning stale peers
            // here would put the previous space's PCs on the card during a
            // switch, before the new network is up.
            return Ok(Value::Array(Vec::new()));
        }
        let mut candidates: Vec<(u64, String, String, Option<String>)> = Vec::new();
        if let Some(map) = peers.as_object() {
            for (key, entry) in map {
                let rid = entry
                    .pointer("/peer/rid")
                    .and_then(Value::as_u64)
                    .or_else(|| key.parse::<u64>().ok())
                    .unwrap_or(0);
                if rid == 0 || Some(rid) == own_rid {
                    continue;
                }
                let peer_networks: Vec<&str> = entry
                    .pointer("/peer/network_ids")
                    .and_then(Value::as_array)
                    .map(|rows| rows.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if !joined.is_empty()
                    && !peer_networks
                        .iter()
                        .any(|id| joined.iter().any(|own| own == id))
                {
                    continue;
                }
                let name = entry
                    .pointer("/peer/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let vip = entry
                    .pointer("/peer/vip")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let peer_status = entry
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                // Only a live channel belongs on the card. Offline identities,
                // including a previous install of this same PC, are not peers.
                if peer_status != "Connected" {
                    continue;
                }
                let transport = entry
                    .get("transport")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                candidates.push((rid, name, vip, transport));
            }
        }
        // Ping in small parallel waves. One shot per peer was fine for two
        // friends, but a room of twenty would hold the roster poll for a
        // minute on the 3 second ping timeout.
        let mut rtts: Vec<Option<f64>> = Vec::with_capacity(candidates.len());
        for chunk in candidates.chunks(8) {
            let wave: Vec<Option<f64>> = std::thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|(rid, _, _, _)| {
                        let rid = *rid;
                        scope.spawn(move || {
                            run_json(&["ping", &rid.to_string(), "--json"])
                                .ok()
                                .and_then(|v| v.get("rtt_ms").and_then(Value::as_f64))
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| handle.join().unwrap_or(None))
                    .collect()
            });
            rtts.extend(wave);
        }
        let mut live: BTreeMap<String, Value> = BTreeMap::new();
        for ((rid, name, vip, transport), rtt_ms) in candidates.into_iter().zip(rtts) {
            let row = json!({
                "rid": rid.to_string(),
                "name": name,
                "vip": vip,
                "connected": true,
                "transport": transport,
                "rtt_ms": rtt_ms,
            });
            // A stale duplicate of a live address must not produce two rows.
            live.insert(vip, row);
        }
        let mut rows: Vec<Value> = live.into_values().collect();
        rows.sort_by(|a, b| {
            let live_a = a.get("connected").and_then(Value::as_bool).unwrap_or(false);
            let live_b = b.get("connected").and_then(Value::as_bool).unwrap_or(false);
            live_b.cmp(&live_a).then_with(|| {
                let name_a = a.get("name").and_then(Value::as_str).unwrap_or_default();
                let name_b = b.get("name").and_then(Value::as_str).unwrap_or_default();
                name_a.cmp(name_b)
            })
        });
        Ok(Value::Array(rows))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_space_password_opens_again() {
        let mut space = LanSpace::new("Seal Test");
        let plain = space.password.clone();
        seal_space(&mut space).expect("seal");
        assert!(space.password.is_empty());
        assert!(!space.password_sealed.is_empty());
        let opened = unseal_space(space).expect("open");
        assert_eq!(opened.password, plain);
    }
}
