//! The OpenRad backed LAN.
//!
//! One button starts the bundled OpenRad engine, points it at our
//! coordination server, registers this PC once, starts the service and joins
//! the shared network. The user never sees an OpenRad command.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SERVER: &str = "80.225.89.162";
const PORT: &str = "17301";
const NETWORK: &str = "QuotaVPN";

fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default()
}

fn exe() -> PathBuf {
    install_dir().join("openrad").join("openrad.exe")
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

/// The OpenRad engine looks for an adapter named exactly `OpenRad`.
fn ensure_adapter() -> Result<(), String> {
    let tap = install_dir().join("tap").join("amd64");
    let devcon = tap.join("devcon.exe");
    let inf = tap.join("OemVista.inf");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         if (Get-NetAdapter -Name 'OpenRad') {{ exit 0 }}; \
         & '{}' install '{}' TAP0901 | Out-Null; \
         Start-Sleep -Milliseconds 900; \
         $n = Get-NetAdapter | Where-Object {{ $_.InterfaceDescription -like '*TAP-Windows*' -and $_.Name -ne 'QuotaSpaceLAN' }} | Sort-Object ifIndex | Select-Object -Last 1; \
         if ($n) {{ Rename-NetAdapter -Name $n.Name -NewName 'OpenRad' }}",
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

pub fn connect_blocking() -> Result<Value, String> {
    ensure_adapter()?;
    // Changing the server is refused while the engine runs, and a previous
    // connect or a manual run can leave it up. Stop first, then configure.
    let _ = run_raw(&["stop"]);
    clear_stale_address();
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
    let _ = run_json(&["init", "--node-name", &name, "--json"]);
    run_raw(&["start"])?;
    let _ = run_json(&["join", NETWORK, "--json"]);
    run_json(&["status", "--json"])
}

/// A previous engine instance can leave its `26.x` address on the adapter.
/// OpenRad refuses to start while another interface holds the `26/8`, and its
/// own stale address counts.
fn clear_stale_address() {
    let script = "$ErrorActionPreference='SilentlyContinue'; \
         Get-NetIPAddress -InterfaceAlias 'OpenRad' -AddressFamily IPv4 | \
         Where-Object { $_.IPAddress -like '26.*' } | \
         Remove-NetIPAddress -Confirm:$false";
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

#[tauri::command]
pub async fn openrad_connect() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(connect_blocking)
        .await
        .map_err(|e| e.to_string())?
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

/// A connected roster for the LAN card. The engine also lists identities that
/// are no longer this PC (an old install with the same device name) and peers
/// that are offline; neither belongs in the live peer list. RTT is measured
/// only for connected peers, one small authenticated keepalive each.
#[tauri::command]
pub async fn openrad_peers() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let status = run_json(&["status", "--json"])?;
        let own_rid = status.get("rid").and_then(Value::as_u64);
        let peers = run_json(&["peers", "--json"])?;
        let mut live: BTreeMap<String, Value> = BTreeMap::new();
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
                let rtt_ms = run_json(&["ping", &rid.to_string(), "--json"])
                    .ok()
                    .and_then(|v| v.get("rtt_ms").and_then(Value::as_f64));
                let row = json!({
                    "rid": rid.to_string(),
                    "name": name,
                    "vip": vip,
                    "connected": true,
                    "transport": entry.get("transport").and_then(Value::as_str),
                    "rtt_ms": rtt_ms,
                });
                // A stale duplicate of a live address must not produce two rows.
                live.insert(vip, row);
            }
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
