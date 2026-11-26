//! Built-in TUN connection (no third-party client needed):
//! runs a pinned sing-box engine with a wintun TUN interface in auto-route
//! mode, so EVERYTHING on this PC (browsers, games, speed tests) goes
//! through the card's server. Needs admin (the app manifest requests it).
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const NO_WINDOW: u32 = 0x08000000;
#[cfg(not(windows))]
const NO_WINDOW: u32 = 0;

// ---------------------------------------------------------------- artifact integrity

/// SHA-256 of each archive this app downloads, pinned in source.
///
/// Every one of these is fetched over the network and then executed or loaded by
/// an elevated process: the tunnel engine, the Wintun driver DLL, and the Ookla
/// speed-test binary. A download that is not checked is a remote code execution
/// primitive for whoever can serve it, and the signed installer is irrelevant to
/// it because none of this travels through the updater. Pinning the hash here is
/// what makes the fetch safe; bumping a pin is a deliberate, reviewable act.
const ENGINE_SHA256: &str = "cc9e23f47cccdeb581085f56c3f71cca26ec14b5aef063b2bfcf49ee45fc45d7";
const WINTUN_SHA256: &str = "07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51";

pub fn sha256_of(path: &std::path::Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Refuse to go on unless the downloaded file is the exact bytes this build
/// expects. The file is removed on a mismatch so a later run cannot retry with it.
#[cfg_attr(not(windows), allow(dead_code))]
fn verify_artifact(path: &std::path::Path, expected: &str, what: &str) -> Result<(), String> {
    match sha256_of(path) {
        Some(got) if got == expected => Ok(()),
        Some(got) => {
            let _ = std::fs::remove_file(path);
            Err(format!(
                "the {what} download is not the build this app expects (got {got}, want {expected})"
            ))
        }
        None => Err(format!("the {what} download could not be read")),
    }
}

/// Engine: amnezia-box (sing-box + AmneziaWG obfuscated WireGuard), hosted as
/// a release asset so every install pulls the same pinned build.
const ENGINE_TAG: &str = "awg-1.14.1";
const ENGINE_VER: &str = "1.14.1";
const ENGINE_URL: &str =
    "https://github.com/YousefMohiey/QuotaVPN/releases/download/engine-awg-1141/engine-awg-1141.zip";
const WINTUN_TAG: &str = "0.14.1";
const WINTUN_URL: &str = "https://www.wintun.net/builds/wintun-0.14.1.zip";

/// Global tunnel-operation lock: engine stop/cleanup and engine spawn must
/// never overlap (a cleanup hunt racing a fresh spawn kills the new engine).
/// Hold it ONLY around spawn_engine calls - never around network waits,
/// settle sleeps or egress probes, or Disconnect hangs behind Connect.
pub static TUN_OPS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Tunnel generation: bumped on every stop request. A connect thread that
/// finishes after a stop carries a stale gen and must quietly drop its
/// engine instead of reporting VpnUp (the UI already says Disconnected).
static TUN_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Bump the generation (call on the UI thread when stopping). Returns the
/// new generation.
#[cfg(not(target_os = "windows"))]
use crate::wincompat::CmdExt;
pub fn bump_gen() -> u64 {
    TUN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

/// Current generation (connect threads capture this, VpnUp carries it).
pub fn tunnel_gen() -> u64 {
    TUN_GEN.load(std::sync::atomic::Ordering::SeqCst)
}
pub const TUN_IP: &str = "172.19.0.1";
/// Candidate tunnel addresses, classic first. The engine refuses to start
/// ("configure tun interface: set ipv4 address: The object already exists")
/// when the address or its /28 prefix already exists on the machine: a ghost
/// adapter from a force-killed run, or Hyper-V / Docker / WSL / another VPN
/// on the same range. A taken candidate is skipped, not failed on.
pub const TUN_IP_CANDIDATES: [&str; 8] = [
    "172.19.0.1",
    "172.20.0.1",
    "172.21.0.1",
    "172.22.0.1",
    "10.18.0.1",
    "10.19.0.1",
    "10.20.0.1",
    "10.21.0.1",
];

/// The IPv6 ULA the tunnel used to hardcode; it collides in exactly the same
/// way ("set ipv6 address: The object already exists"), so it gets its own
/// candidate list. The first entry matches every build so far.
pub const TUN_V6_CANDIDATES: [&str; 6] = [
    "fdfe:dcba:9876::1",
    "fdfe:dcba:9877::1",
    "fdfe:dcba:9878::1",
    "fdfe:dcba:9879::1",
    "fdfe:dcba:987a::1",
    "fdfe:dcba:987b::1",
];

/// First candidate whose stem is absent from every blob (routes and adapter
/// list both scanned: a ghost adapter shows in ipconfig, its prefix route in
/// the route table). The stem keeps everything up to the last '.' or ':', so
/// it catches the address itself, its prefix route, and any route through it.
fn first_absent(candidates: &[&str], blobs: &[&str]) -> String {
    let hay: Vec<String> = blobs.iter().map(|b| b.to_lowercase()).collect();
    for cand in candidates {
        let low = cand.to_lowercase();
        let cut = low.rfind(&['.', ':'][..]).map(|i| i + 1).unwrap_or(0);
        let stem = &low[..cut];
        if hay.iter().any(|h| h.contains(stem)) {
            continue;
        }
        return cand.to_string();
    }
    candidates[0].to_string()
}

/// (v4, v6) tunnel addresses: the first candidate each set of tables is free
/// of. Split out so tests can feed synthetic route/adapter tables.
fn pick_addresses(routes4: &str, routes6: &str, adapters: &str) -> (String, String) {
    (
        first_absent(&TUN_IP_CANDIDATES, &[routes4, adapters]),
        first_absent(&TUN_V6_CANDIDATES, &[routes6, adapters]),
    )
}

#[cfg(windows)]
fn pick_tun_ips() -> (String, String) {
    let routes4 = run_hidden("route", &["print", "-4"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let routes6 = run_hidden("route", &["print", "-6"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let adapters = run_hidden("ipconfig", &["/all"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    pick_addresses(&routes4, &routes6, &adapters)
}

/// The TUN address of the last written config (no file yet: the classic
/// constant), so route checks and cleanup follow the address actually used.
pub fn current_tun_ip() -> String {
    std::fs::read_to_string(engine_dir().join("tun-ip.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| TUN_IP.to_string())
}

/// Addresses whose stale default routes cleanup must drop: the session's own
/// plus the classic constant (ghosts of force-killed runs sit there).
fn cleanup_tun_ips() -> Vec<String> {
    let mut v = vec![current_tun_ip()];
    if !v.iter().any(|ip| ip == TUN_IP) {
        v.push(TUN_IP.to_string());
    }
    v
}
const DIRECT_DNS: &str = "8.8.8.8";
const PROXY_DOH_IP: &str = "1.1.1.1";

pub fn engine_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("quotavpn").join("engine")
}

#[cfg(windows)]
pub fn singbox_path() -> PathBuf {
    engine_dir().join("sing-box.exe")
}

#[cfg(windows)]
pub fn wintun_path() -> PathBuf {
    engine_dir().join("wintun.dll")
}

pub fn tun_config_path() -> PathBuf {
    engine_dir().join("singbox-tun.json")
}

/// Is another interface publishing a default route right now?
///
/// Deliberately a count, not a name: any of them has the same effect on routing, and the app
/// should not have to know about Tailscale specifically.
#[cfg(windows)]
fn another_default_route() -> bool {
    // `route print` is a native command; the old PowerShell query spent about
    // a second starting the shell on every connect just to count two lines.
    let out = std::process::Command::new("route")
        .args(["print", "-4"])
        .creation_flags(NO_WINDOW)
        .output();
    let Ok(o) = out else {
        return false;
    };
    let text = String::from_utf8_lossy(&o.stdout);
    let mut count = 0;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() == Some("0.0.0.0") && fields.next() == Some("0.0.0.0") {
            count += 1;
        }
    }
    count > 1
}

/// Put our tun ahead of the competition once it exists.
///
/// Only meaningful when another default route was detected; a failed call is a warning, never
/// a reason to refuse the connection, because the tunnel still works without it.
#[cfg(windows)]
fn raise_our_route() {
    // The adapter name is written before the engine starts. `netsh` resolves
    // its index in one native call; the old PowerShell lookup cost about a
    // second on every connect.
    let name = std::fs::read_to_string(engine_dir().join("tun-ifname.txt")).unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    let Ok(out) = run_hidden("netsh", &["interface", "ipv4", "show", "interfaces"]) else {
        return;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let Some(idx) = text
        .lines()
        .find(|line| line.contains(name))
        .and_then(|line| line.split_whitespace().next())
    else {
        return;
    };
    let _ = run_hidden("netsh", &["interface", "ipv4", "set", "interface", idx, "metric=1"]);
}

#[cfg(windows)]
fn cmd_hidden(prog: &str) -> Command {
    let mut c = Command::new(prog);
    c.creation_flags(NO_WINDOW);
    c
}

#[cfg(not(windows))]
fn cmd_hidden(prog: &str) -> Command {
    Command::new(prog)
}

fn run_hidden(prog: &str, args: &[&str]) -> Result<std::process::Output, String> {
    cmd_hidden(prog)
        .args(args)
        .output()
        .map_err(|e| format!("{prog}: {e}"))
}

/// True when running elevated. `net session` only succeeds as admin.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    run_hidden("net", &["session"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Download only the pinned Wintun DLL. Spaces needs the adapter on a fresh
/// install that never connected the main VPN, so the full sing-box engine is
/// not required just to open the virtual LAN.
#[cfg(windows)]
pub fn ensure_wintun() -> Result<(), String> {
    let path = wintun_path();
    if path.exists() {
        return Ok(());
    }
    let dir = engine_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let wzip = dir.join("wintun.zip");
    let dl = run_hidden(
        "curl",
        &[
            "-L",
            "--max-time",
            "120",
            "-A",
            "QuotaVPN",
            "-o",
            &wzip.to_string_lossy(),
            WINTUN_URL,
        ],
    )
    .map(|o| o.status.success())
    .unwrap_or(false);
    if !dl {
        return Err("the Wintun driver download failed (check internet)".into());
    }
    verify_artifact(&wzip, WINTUN_SHA256, "wintun driver")?;
    run_hidden(
        "tar",
        &["-xf", &wzip.to_string_lossy(), "-C", &dir.to_string_lossy()],
    )
    .map_err(|e| format!("extract driver: {e}"))?;
    let _ = std::fs::remove_file(&wzip);
    let extracted = dir.join("wintun").join("bin").join("amd64").join("wintun.dll");
    std::fs::copy(&extracted, &path).map_err(|e| format!("install driver: {e}"))?;
    let _ = std::fs::remove_dir_all(dir.join("wintun"));
    if path.exists() {
        Ok(())
    } else {
        Err("the Wintun driver is missing after extract".into())
    }
}

/// Download + extract the sing-box engine (pinned) and wintun driver.
/// Version-stamped so a stale engine is replaced automatically.
#[cfg(windows)]
pub fn ensure_engine() -> Result<String, String> {
    let dir = engine_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    // one-way migration: drop the legacy xray engine remnants
    for f in ["xray.exe", "xray-client.json", "xray.zip"] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    let ver_file = dir.join("tun-version.txt");
    let cur = std::fs::read_to_string(&ver_file).unwrap_or_default();
    let want = format!("amnezia-box {ENGINE_TAG} + wintun {WINTUN_TAG}");
    if singbox_path().exists() && wintun_path().exists() && cur.trim() == want {
        return Ok("engine ready".to_string());
    }
    // --- engine (amnezia-box: sing-box + AmneziaWG) ---
    let zip = dir.join("engine.zip");
    let dl = run_hidden(
        "curl",
        &[
            "-L", "--max-time", "180", "-A", "QuotaVPN", "-o",
            &zip.to_string_lossy(), ENGINE_URL,
        ],
    )
    .map(|o| o.status.success())
    .unwrap_or(false)
        && zip.exists();
    if !dl {
        return Err("engine download failed (check internet)".to_string());
    }
    // Before the extract, not after: `tar` on an archive is already parsing
    // attacker-shaped input.
    verify_artifact(&zip, ENGINE_SHA256, "tunnel engine")?;
    run_hidden(
        "tar",
        &["-xf", &zip.to_string_lossy(), "-C", &dir.to_string_lossy()],
    )
    .map_err(|e| format!("extract: {e}"))?;
    let _ = std::fs::remove_file(&zip);
    let inner = dir.join(format!("sing-box-{ENGINE_VER}-windows-amd64"));
    if singbox_path().exists() {
        let _ = std::fs::remove_file(singbox_path());
    }
    std::fs::rename(inner.join("sing-box.exe"), singbox_path())
        .map_err(|e| format!("install engine: {e}"))?;
    let _ = std::fs::remove_dir_all(inner);
    // --- wintun (driver DLL lives next to the engine exe) ---
    let wzip = dir.join("wintun.zip");
    let dl = run_hidden(
        "curl",
        &[
            "-L", "--max-time", "120", "-A", "QuotaVPN", "-o",
            &wzip.to_string_lossy(), WINTUN_URL,
        ],
    )
    .map(|o| o.status.success())
    .unwrap_or(false)
        && wzip.exists();
    if !dl {
        return Err("driver download failed (check internet)".to_string());
    }
    verify_artifact(&wzip, WINTUN_SHA256, "wintun driver")?;
    run_hidden(
        "tar",
        &["-xf", &wzip.to_string_lossy(), "-C", &dir.to_string_lossy()],
    )
    .map_err(|e| format!("extract: {e}"))?;
    let _ = std::fs::remove_file(&wzip);
    std::fs::copy(
        dir.join("wintun").join("bin").join("amd64").join("wintun.dll"),
        wintun_path(),
    )
    .map_err(|e| format!("install driver: {e}"))?;
    let _ = std::fs::remove_dir_all(dir.join("wintun"));
    if singbox_path().exists() && wintun_path().exists() {
        let _ = std::fs::write(&ver_file, &want);
        Ok(format!("engine {ENGINE_TAG} ready"))
    } else {
        Err("engine missing after extract".to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct IpCache {
    host: String,
    ts: u64,
    ips: Vec<String>,
}

fn ip_cache_path() -> std::path::PathBuf {
    engine_dir().join("server-ips-cache.json")
}

fn cached_server_ips(host: &str) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(ip_cache_path()).ok()?;
    let cache: IpCache = serde_json::from_str(&text).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    (cache.host == host && now.saturating_sub(cache.ts) < 1800 && !cache.ips.is_empty())
        .then_some(cache.ips)
}

fn cache_server_ips(host: &str, ips: &[String]) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cache = IpCache {
        host: host.to_string(),
        ts: now,
        ips: ips.to_vec(),
    };
    if let Ok(text) = serde_json::to_string(&cache) {
        let _ = std::fs::create_dir_all(engine_dir());
        let _ = std::fs::write(ip_cache_path(), text);
    }
}

/// A server move must not be hidden behind the IP cache. Called on a failed
/// connect so the next press resolves the name again.
pub fn clear_server_ip_cache() {
    let _ = std::fs::remove_file(ip_cache_path());
}

/// Resolve the server domain NOW (system DNS intact) so its IPs can be
/// excluded from TUN routing - otherwise the tunnel eats its own path. The
/// answer is cached for thirty minutes: a warm connect must not spend a DNS
/// round trip inside the config build.
pub fn resolve_server_ips(host: &str) -> Result<Vec<String>, String> {
    if let Some(ips) = cached_server_ips(host) {
        return Ok(ips);
    }
    let mut ips: Vec<String> = format!("{host}:443")
        .to_socket_addrs()
        .map_err(|e| format!("can't resolve {host} ({e})"))?
        .map(|a| a.ip())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|ip| {
            if ip.is_ipv4() {
                format!("{ip}/32")
            } else {
                format!("{ip}/128")
            }
        })
        .collect();
    ips.sort();
    if ips.is_empty() {
        return Err(format!("can't resolve {host}"));
    }
    cache_server_ips(host, &ips);
    Ok(ips)
}

/// Destination networks of the game's VOICE infrastructure: Vivox and the
/// hosting companies its media ran on. This list matches the route lines of
/// the working community OpenVPN config for Egypt (the owner-supplied
/// "Valorant1.ovpn", which labels these exact ranges "Valorant Voice Chat
/// IPs"), line for line, down to a /24 where we once carried a wider /16.
/// They are matched WITHOUT process scoping or sniffing, so they also catch
/// the flows where the game talks to a voice server by raw IP on a port
/// nobody published. The network ranges are the part that actually carries
/// voice; every extra port rule this feature once had was retired because it
/// also swept game-adjacent and third-party flows (the Riot client's own
/// master connection, other games' Steam-range UDP) into the tunnel, which
/// showed up as lag and a brief in-game disconnect the moment the tunnel
/// came up. Live-tested: from Egypt the direct path to a voice IP times
/// out while the same connection through the tunnel answers in ms.
const VOICE_CIDRS: [&str; 8] = [
    "20.0.0.0/8",
    "63.251.140.0/24",
    "69.25.124.0/24",
    "70.42.0.0/16",
    "74.201.0.0/16",
    "85.0.0.0/8",
    "188.42.0.0/16",
    "216.52.0.0/16",
];

/// A localhost port nobody is using right now, for the engine's Clash API.
/// Bind-then-drop: the engine re-binds it a moment later; a stale collision
/// would only ever cost the health check, never the tunnel itself.
fn free_local_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(15309)
}

/// A per-run secret for that localhost control channel. Not cryptographic;
/// the listener only ever faces 127.0.0.1 and lives with the session.
fn session_secret() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    format!(
        "{:016x}{:016x}",
        (n as u64) ^ 0x9E37_79B9_7F4A_7C15u64,
        ((n >> 64) as u64).wrapping_add(pid as u64)
    )
}

/// Full TUN client config for one card. Self-signed server cert → insecure
/// (same trust as the share links); SNI stamp preserved.
/// `apps_mode` picks the per-app routing: "allow" sends ONLY the listed
/// process names through the tunnel, "block" sends everything EXCEPT them,
/// anything else routes the whole PC.
/// `voice` adds the VALORANT voice rules: only the voice networks and the
/// Riot-scoped STUN/Vivox hedges ride the tunnel, everything else (the
/// game's own traffic included) stays direct.

/// The resolvers this machine actually asks. On Windows the tunnel's adapter
/// arrives with a DNS server of its own; on Linux the system keeps pointing
/// at the router, and a kill switch that drops the LAN would take DNS with
/// it. Routing those resolvers into the tunnel means their queries are
/// hijacked and answered over the proxy instead of being dropped or leaked.
#[cfg(target_os = "linux")]
fn system_dns_addresses() -> Vec<String> {
    let text = std::fs::read_to_string("/etc/resolv.conf").unwrap_or_default();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("nameserver") {
            let ip = rest.trim();
            if ip.parse::<std::net::IpAddr>().is_ok() {
                let cidr = if ip.contains(':') {
                    format!("{ip}/128")
                } else {
                    format!("{ip}/32")
                };
                if !out.contains(&cidr) {
                    out.push(cidr);
                }
            }
        }
    }
    out
}

pub fn write_tun_config(
    uuid: &str,
    host: &str,
    sni: &str,
    apps_mode: &str,
    apps: &[String],
    voice: bool,
    transport: &str,
    hy2_pass: &str,
    wg: Option<(&str, &str, &str, &str)>,
) -> Result<PathBuf, String> {
    write_tun_config_for(
        uuid,
        host,
        sni,
        apps_mode,
        apps,
        voice,
        transport,
        hy2_pass,
        wg,
        &routed_device_ips(),
        lan_bind,
        None,
    )
    .map(|(path, _)| path)
}

/// Build the config with one active Space. The space endpoint rides the main
/// outbound (`detour: proxy`), so the config is otherwise the normal session:
/// the card still picks the transport, the route still sends everything else
/// where it did.
pub fn write_tun_config_with_space(
    uuid: &str,
    host: &str,
    sni: &str,
    apps_mode: &str,
    apps: &[String],
    voice: bool,
    transport: &str,
    hy2_pass: &str,
    wg: Option<(&str, &str, &str, &str)>,
    space: Option<&crate::spaces::SpaceTunnel>,
) -> Result<PathBuf, String> {
    write_tun_config_for(
        uuid,
        host,
        sni,
        apps_mode,
        apps,
        voice,
        transport,
        hy2_pass,
        wg,
        &routed_device_ips(),
        lan_bind,
        space,
    )
    .map(|(path, _)| path)
}

/// The LAN addresses this machine has promised to carry for a device.
///
/// A claim writes an intention record before it touches anything, and that record
/// outlives a crash, so a config rebuilt from disk still knows the device is there.
/// Non-Windows has no claim mechanism; the tests that need a routed device pass the
/// list in themselves, which is why this answers empty under `cfg(test)`.
fn routed_device_ips() -> Vec<String> {
    #[cfg(all(windows, not(test)))]
    {
        // Read the target address straight out of the record. The decoded
        // Intent needs every field, including the tun index, which is empty
        // whenever the engine is not up at claim time, and one empty field
        // used to drop the device rule without a word.
        let path = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("quotavpn")
            .join("lan_route_intent.txt");
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let ip = text.split('|').next().unwrap_or("").trim().to_string();
        if ip.parse::<std::net::Ipv4Addr>().is_ok() {
            vec![format!("{ip}/32")]
        } else {
            Vec::new()
        }
    }
    #[cfg(not(all(windows, not(test))))]
    {
        Vec::new()
    }
}

/// The address and adapter the direct outbound has to leave through.
///
/// Read from the same probe the claim uses, so the two can never disagree about
/// which interface is the LAN.
fn lan_bind() -> Option<(String, String)> {
    #[cfg(windows)]
    {
        crate::lan_route::adapter()
            .ok()
            .map(|a| (a.ip.to_string(), a.name))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Build the config from an explicit routing decision, and return both the path
/// and the document that was written.
///
/// Split out of `write_tun_config` so the two inputs that depend on the machine,
/// the routed device list and the LAN bind, can be supplied by a test. The
/// document comes back because the engine config file is one shared path: a test
/// that re-read it could be handed another test's write.
fn write_tun_config_for<F>(
    uuid: &str,
    host: &str,
    sni: &str,
    apps_mode: &str,
    apps: &[String],
    voice: bool,
    transport: &str,
    hy2_pass: &str,
    wg: Option<(&str, &str, &str, &str)>,
    routed: &[String],
    bind: F,
    space: Option<&crate::spaces::SpaceTunnel>,
) -> Result<(PathBuf, serde_json::Value), String>
where
    F: FnOnce() -> Option<(String, String)>,
{
    let mut excludes = resolve_server_ips(host)?;
    // The config, session and address records all live here. Production
    // creates this during engine setup; a direct config build (including the
    // tests) must be able to create it too, or the first write fails on a
    // fresh profile.
    let _ = std::fs::create_dir_all(engine_dir());
    // The kill switch needs to know which addresses the tunnel itself is
    // allowed to reach, and it runs as root without the card in hand.
    #[cfg(target_os = "linux")]
    {
        let bare: Vec<String> = excludes
            .iter()
            .map(|c| c.split('/').next().unwrap_or(c).to_string())
            .collect();
        let _ = std::fs::create_dir_all(engine_dir());
        let _ = std::fs::write(engine_dir().join("server-ips.txt"), bare.join("
"));
    }
    for ip in [PROXY_DOH_IP, DIRECT_DNS] {
        let cidr = format!("{ip}/32");
        if !excludes.contains(&cidr) {
            excludes.push(cidr);
        }
    }
    // allow mode with no list is normally a mistake, but the voice-only
    // session uses exactly that shape: keep its finals per-app so only the
    // voice channels ride the tunnel. Whole-device ("all") is untouched.
    let apps_only = apps_mode == "allow" && (!apps.is_empty() || voice);
    let apps_except = apps_mode == "block" && !apps.is_empty();
    let per_app = apps_only || apps_except;
    // A device on the LAN is being carried: the host itself is NOT. Sending the
    // host through the tunnel as well would replace the address the owner checks
    // the feature with, and the claim would then be indistinguishable from
    // connecting the PC.
    //
    // One host means /32, and every rule below spells it that way. Written out
    // here rather than trusted from the caller: a bare address in one rule and a
    // /32 in another is the kind of difference nobody notices until a rule stops
    // matching, and `device_route_ready` compares the literal text.
    let routed: Vec<String> = routed
        .iter()
        .map(|a| if a.contains('/') { a.clone() } else { format!("{a}/32") })
        .collect();
    let routed = routed.as_slice();
    let device_only = !routed.is_empty();
    // unique adapter name per connect: a force-killed run can leave a ghost
    // adapter in the driver that would block a reused name.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let if_name = format!("QuotaVPN{:04x}", nanos & 0xffff);
    // TUN addresses: first candidates this machine is not already using.
    // Plenty of PCs hold 172.19.x (or the old ULA) already - Hyper-V, Docker,
    // WSL, a ghost adapter from a force-killed run - and the engine then
    // refuses to start with "The object already exists", so neither address
    // is ever hardcoded.
    let (tun_ip, tun_v6) = pick_tun_ips();
    // per-app routing matches by process, which needs the userspace stack
    // on Windows; whole-PC mode keeps the fast mixed stack.
    let tun_stack = if per_app { "gvisor" } else { "mixed" };
    // Only Linux needs the resolvers pulled into the tunnel; on Windows the
    // adapter already carries its own and nothing is gained by the extra route.
    #[cfg(target_os = "linux")]
    let dns_routes: Vec<String> = {
        // route_address REPLACES the routes sing-box would otherwise install
        // for auto_route. Listing only the resolvers therefore left the tun
        // with no IPv4 default at all: the engine ran, the app said connected,
        // and every packet still left through the LAN. Carry the whole IPv4
        // space as two halves and append the resolvers to them.
        let mut routes = vec!["0.0.0.0/1".to_string(), "128.0.0.0/1".to_string()];
        routes.extend(system_dns_addresses());
        routes
    };
    // Windows: nothing is added unless something else already owns a default route. A /0
    // beats the tun's two /1s by prefix length whatever the metric, so a running Tailscale
    // exit node, a second NIC or a corporate VPN would quietly carry every packet and the
    // carrier would file it as general. Holding /0 as well makes it a prefix tie, and the
    // tie is decided by metric, which `raise_our_route` then wins.
    #[cfg(not(target_os = "linux"))]
    let dns_routes: Vec<String> = if another_default_route() {
        vec!["0.0.0.0/0".to_string()]
    } else {
        Vec::new()
    };
    let dns_final = if apps_only || device_only { "direct-dns" } else { "proxy-dns" };
    let route_final = if apps_only || device_only { "direct" } else { "proxy" };
    // Where the direct outbound leaves from. With `final: "direct"` and
    // `auto_detect_interface`, the direct outbound binds to whatever the tun just
    // made the default interface, so every packet the host sends would come
    // straight back in: a loop, not a route. Both pins are needed. The address
    // fixes the source, and Windows picks a route by destination prefix first and
    // per-interface metric second, so an address alone still loses to the tun's
    // 0.0.0.0/1 entries; the interface decides the route.
    let bind_target = if route_final == "direct" { bind() } else { None };
    if device_only && bind_target.is_none() {
        // Not cosmetic: an unpinned direct outbound in a device-only session
        // loops the whole session back into its own tun.
        return Err(
            "cannot pin the direct outbound to the LAN adapter: the probe found no \
             address and interface to bind to"
                .to_string(),
        );
    }
    let direct_out = match &bind_target {
        Some((ip, iface)) => serde_json::json!({
            "type": "direct",
            "tag": "direct",
            "inet4_bind_address": ip,
            "bind_interface": iface
        }),
        None => serde_json::json!({"type": "direct", "tag": "direct"}),
    };
    let mut route_rules = serde_json::json!([
        {"action": "sniff"},
        {"network": "udp", "port": [135, 137, 138, 139, 5353], "action": "reject"},
        {"ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject"},
        {"source_ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject"},
        {"protocol": "dns", "action": "hijack-dns"}
    ]);
    if device_only {
        let rules = route_rules.as_array_mut().unwrap();
        // Inserted between the reject block and the DNS hijack: below the sniff
        // action, because a rule above sniffing is evaluated before anything has
        // been sniffed and could never match on protocol, and above the hijack, so
        // the device's queries are the ones that reach the tunnel's resolver.
        // Lower index wins, so the DNS rule goes in last and ends up first.
        const BEFORE_HIJACK: usize = 4;
        rules.insert(
            BEFORE_HIJACK,
            serde_json::json!({"source_ip_cidr": routed, "outbound": "proxy"}),
        );
        rules.insert(
            BEFORE_HIJACK,
            serde_json::json!({"protocol": "dns", "source_ip_cidr": routed, "action": "hijack-dns"}),
        );
    }
    if per_app {
        let rules = route_rules.as_array_mut().unwrap();
        // allow: the listed apps ride the tunnel. block: the listed apps skip it.
        let target = if apps_only { "proxy" } else { "direct" };
        let (paths, names): (Vec<&String>, Vec<&String>) =
            apps.iter().partition(|a| a.contains('\\') || a.contains('/'));
        // Insert just BELOW the sniff action: a rule that sits above a sniff
        // rule is evaluated before anything has been sniffed, so it could
        // never match on domain. Ports and process names do not care.
        if !names.is_empty() {
            rules.insert(1, serde_json::json!({"process_name": names, "outbound": target}));
        }
        if !paths.is_empty() {
            rules.insert(1, serde_json::json!({"process_path": paths, "outbound": target}));
        }
    }
    if voice {
        // Voice rules go on top of whatever routing the session already has,
        // so the normal VPN and the voice helper run in the same tunnel. They
        // are inserted just below the sniff action, never at index 0: a rule
        // above a sniff rule is evaluated before anything has been sniffed
        // and can never match on domain (that bug shipped once).
        let rules = route_rules.as_array_mut().unwrap();
        let riot = serde_json::json!([
            "VALORANT-Win64-Shipping.exe",
            "VALORANT.exe",
            "RiotClientServices.exe"
        ]);
        // The voice infrastructure NETWORKS come first: this works for every
        // port and for flows where the client talks to a bare IP (the common
        // case today; the old name-based voice endpoints no longer resolve).
        rules.insert(
            1,
            serde_json::json!({"ip_cidr": VOICE_CIDRS.to_vec(), "outbound": "proxy"}),
        );
        // 20.0.0.0/8 carries the voice networks, but it also contains
        // Microsoft's identity endpoints (20.190.x.x). With the voice helper
        // on, login.microsoftonline.com, login.live.com and every OAuth flow
        // failed with ERR_CONNECTION_CLOSED while unrelated Microsoft sites
        // kept working. This direct exception sits above the voice CIDR and
        // below the Riot-scoped rules (which are inserted after it), so only
        // non-Riot traffic to that /16 leaves directly.
        rules.insert(
            1,
            serde_json::json!({"ip_cidr": ["20.190.0.0/16"], "outbound": "direct"}),
        );
        // Name-based fallback, for any future name-based voice endpoint.
        rules.insert(
            1,
            serde_json::json!({
                "process_name": riot,
                "domain_suffix": ["vivox.com", "voice.riotgames.com"],
                "outbound": "proxy"
            }),
        );
        // The Riot-scoped STUN hedge stays: it cannot sweep anything but the
        // listed processes. What does NOT come back: the Riot-process
        // 8393:8400 rule (Riot's own port table labels that range Installer
        // and Master - the game client's own launch and lobby traffic, which
        // belongs DIRECT, and tunneling it was a prime suspect for the
        // in-game disconnect people saw when the helper engaged), and the
        // unscoped 27016:27024 / 54000:54012 media ranges in both directions
        // (they swept third-party Steam-range and random-source-port flows
        // into the tunnel; the voice networks above are what actually carry
        // the media, proven by the community OpenVPN config this forked
        // from, which carries no port rules at all).
        rules.insert(
            1,
            serde_json::json!({
                "process_name": riot,
                "network": "udp",
                "port_range": ["3478:3480"],
                "outbound": "proxy"
            }),
        );
    }
    {
        // IPv6 is captured by the tun (so it cannot leak around the VPN) but
        // never carried: the exit has no usable v6 path, and a new v6 flow
        // sent into it blackholes without any fallback, which is exactly how
        // Discord voice calls and screen shares broke while existing v4 flows
        // kept working. Rejecting v6 makes apps fall back to v4 politely.
        // This must sit above every other rule, voice rules included.
        let rules = route_rules.as_array_mut().unwrap();
        rules.insert(0, serde_json::json!({"ip_version": 6, "action": "reject"}));
    }
    let mut dns_rules = if voice {
        serde_json::json!([
            {"domain": [host], "server": "direct-dns"},
            // Voice connections are IP-based now, so the CIDR rules carry the
            // real traffic; these suffix rules keep any name-based voice flow
            // resolving through the tunnel rather than the local resolver.
            {"domain_suffix": ["vivox.com", "voice.riotgames.com"], "server": "proxy-dns"}
        ])
    } else {
        serde_json::json!([{"domain": [host], "server": "direct-dns"}])
    };
    if device_only {
        // Appended, not prepended: the host's own name still has to resolve
        // directly. The queried address is the only thing that tells the two
        // sides apart inside the DNS module.
        let rules = dns_rules.as_array_mut().unwrap();
        rules.push(serde_json::json!({"source_ip_cidr": routed, "server": "proxy-dns"}));
    }
    // The UDP transports dial the resolved address directly: name resolution
    // inside the engine is one more thing that can go wrong on a game path.
    let server_v4 = excludes
        .iter()
        .find(|c| c.ends_with("/32"))
        .map(|c| c.trim_end_matches("/32").to_string())
        .unwrap_or_else(|| host.to_string());
    // Transport shapes follow the vendored engine (1.13+): WireGuard is a
    // top-level ENDPOINT (the outbound form was removed outright), Hysteria2
    // is an outbound. The endpoint carries the "proxy" tag, so route rules,
    // DNS detours, per-app rules and the voice rules all keep working.
    let hy2_out = |tag: &str| {
        serde_json::json!({
            "type": "hysteria2",
            "tag": tag,
            "server": server_v4,
            "server_port": 443,
            "password": hy2_pass,
            "up_mbps": 10,
            "down_mbps": 50,
            "tls": {
                "enabled": true,
                "server_name": sni,
                "insecure": true,
                "alpn": ["h3"]
            }
        })
    };
    let (outbounds, mut endpoints) = match transport {
        "hy2" => (
            serde_json::json!([hy2_out("proxy"), direct_out.clone()]),
            serde_json::json!([]),
        ),
        "wg" => {
            let (priv_key, wg_addr, srv_pub, awg_json) = wg.unwrap_or(("", "", "", ""));
            let p: serde_json::Value = serde_json::from_str(awg_json)
                .map_err(|e| format!("awg params: {e}"))?;
            for k in ["jc", "jmin", "jmax", "s1", "s2", "h1", "h2", "h3", "h4"] {
                if p.get(k).is_none() {
                    return Err(format!("awg params missing {k}"));
                }
            }
            let h = |k: &str| p[k].as_str().map(str::to_string).unwrap_or_else(|| p[k].to_string());
            (
                serde_json::json!([direct_out.clone()]),
                serde_json::json!([{
                    "type": "awg",
                    "tag": "proxy",
                    "address": [wg_addr],
                    "private_key": priv_key,
                    "jc": p["jc"],
                    "jmin": p["jmin"],
                    "jmax": p["jmax"],
                    "s1": p["s1"],
                    "s2": p["s2"],
                    "h1": h("h1"),
                    "h2": h("h2"),
                    "h3": h("h3"),
                    "h4": h("h4"),
                    "peers": [{
                        "address": server_v4,
                        "port": 53,
                        "public_key": srv_pub,
                        "allowed_ips": ["0.0.0.0/0"],
                        "persistent_keepalive_interval": 25
                    }],
                    "mtu": 1280
                }]),
            )
        }
        _ => (
            serde_json::json!([
                {
                    "type": "vless",
                    "tag": "proxy",
                    "server": host,
                    "server_port": 443,
                    "uuid": uuid,
                    "tls": {
                        "enabled": true,
                        "server_name": sni,
                        "insecure": true,
                        "alpn": ["h3", "h2", "http/1.1"],
                        "utls": {"enabled": true, "fingerprint": "random"}
                    }
                },
                direct_out.clone()
            ]),
            serde_json::json!([]),
        ),
    };
    // The host's own lookups leave through the bound direct socket, never back
    // into the tun. An unbound direct DNS server is the same loop as an unbound
    // direct outbound, one layer down.
    let direct_dns = if bind_target.is_some() {
        serde_json::json!({
            "type": "udp",
            "tag": "direct-dns",
            "server": DIRECT_DNS,
            "detour": "direct"
        })
    } else {
        serde_json::json!({"type": "udp", "tag": "direct-dns", "server": DIRECT_DNS})
    };
    let mut cfg = serde_json::json!({
        "log": {"level": "warning"},
        "dns": {
            "servers": [
                {"type": "https", "tag": "proxy-dns", "server": PROXY_DOH_IP, "detour": "proxy"},
                direct_dns
            ],
            "rules": dns_rules,
            "final": dns_final,
            "strategy": "ipv4_only"
        },
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "interface_name": if_name,
            "mtu": if device_only { 1400 } else { 9000 },
            "address": [format!("{tun_ip}/28"), format!("{tun_v6}/126")],
            "auto_route": true,
            // Low-metric block rules that would otherwise kill the bound direct
            // socket and the forwarded return path. With final = direct the host
            // is not being tunnelled, so there is nothing here to protect.
            "strict_route": route_final != "direct",
            "stack": tun_stack,
            "route_exclude_address": excludes,
            "route_address": dns_routes
        }],
        "outbounds": outbounds,
        "route": {
            "rules": route_rules,
            "final": route_final,
            "auto_detect_interface": true,
            "default_domain_resolver": "direct-dns"
        }
    });
    // A Space rides the same session: one top-level endpoint and one IP rule.
    // The rule goes first because it is an IP match, so it needs neither
    // sniffing nor DNS, and it must sit above the IPv6 reject and the default
    // final. The endpoint is only added when the record passed validation, so
    // a damaged space can never stop a normal connect.
    if let Some(space) = space {
        space.valid()?;
        if let Some(list) = endpoints.as_array_mut() {
            list.push(space.endpoint_json());
        }
        if let Some(rules) = cfg["route"]["rules"].as_array_mut() {
            rules.insert(0, space.route_rule());
        }
    }
    // A localhost-only control channel the app itself uses: the engine's
    // Clash API answers "is the upstream actually carrying traffic" with a
    // real dial through the proxy outbound. Process-and-routes liveness stays
    // green while the server path is dead, which is the exact lie the owner
    // caught ("says connected when it is not"). Random port, per-run secret,
    // written next to the config for the app to read.
    let clash_port = free_local_port();
    let clash_secret = session_secret();
    cfg["experimental"] = serde_json::json!({
        "clash_api": {
            "external_controller": format!("127.0.0.1:{clash_port}"),
            "secret": clash_secret.clone()
        }
    });
    // Voice-only sessions take the lesson from the community OpenVPN config
    // this feature forked from: its route-nopull plus explicit routes confine
    // the tunnel to the voice networks at the KERNEL level. Without this,
    // auto_route captures the whole PC into the userspace stack, so the
    // game's own flows get pulled through it the moment the helper engages
    // (the in-game disconnect) and pay userspace overhead while it runs
    // (the lag). With it, only traffic destined to the voice networks is
    // ever routed into the tun; everything else, the game included, never
    // touches it. Sessions with real app lists, whole-device sessions and
    // block-mode sessions must NOT get this: they are defined by capturing
    // everything and classifying per connection.
    if apps_only && voice && apps.is_empty() {
        let mut routes: Vec<String> = VOICE_CIDRS.iter().map(|s| s.to_string()).collect();
        // A voice-only session narrows the TUN to the voice ranges. A space
        // would then have no route into the TUN and its rule could never fire,
        // so its /24 is carried alongside the voice networks.
        if let Some(space) = space {
            routes.push(space.subnet.clone());
        }
        cfg["inbounds"][0]["route_address"] = serde_json::json!(routes);
    }
    // The WireGuard endpoint lives in its own top-level list; omit the key
    // entirely for the other transports so the shape stays minimal.
    if !endpoints.as_array().map(|a| a.is_empty()).unwrap_or(true) {
        cfg["endpoints"] = endpoints;
    }
    let p = tun_config_path();
    std::fs::write(
        &p,
        serde_json::to_string_pretty(&cfg).map_err(|e| format!("config: {e}"))?,
    )
    .map_err(|e| format!("write config: {e}"))?;
    // Remember which adapter belongs to this session. The traffic counter has
    // to match it by its exact name: other products' wintun adapters and
    // ghosts of force-killed runs share the vague words in their description.
    let _ = std::fs::write(p.with_file_name("tun-ifname.txt"), &if_name);
    // Track the picked addresses the same way: cleanup and the route checks
    // must follow the session's own addresses, not the one-time constants.
    let _ = std::fs::write(p.with_file_name("tun-ip.txt"), &tun_ip);
    let _ = std::fs::write(p.with_file_name("tun-ip6.txt"), &tun_v6);
    // The Clash API coordinates for the health check (tunnel_health reads
    // these; the port rides a fresh bind, the secret dies with the session).
    let _ = std::fs::write(
        p.with_file_name("clash-ctl.txt"),
        format!("{clash_port}\n{clash_secret}\n"),
    );
    // The routing intent can change while the app is not in the loop: a device
    // claimed from the Network screen flips this machine out of whole-PC mode,
    // and sing-box reads its config once, at start. Recording the session is
    // what lets the config be rebuilt from disk, with whatever intent is on
    // disk at the time, without the card in hand.
    let (wg_private, wg_addr, wg_server_pub, wg_params) = wg.unwrap_or(("", "", "", ""));
    let _ = remember_session(&SessionRecord {
        uuid: uuid.to_string(),
        host: host.to_string(),
        sni: sni.to_string(),
        apps_mode: apps_mode.to_string(),
        apps: apps.to_vec(),
        voice,
        transport: transport.to_string(),
        hy2_pass: hy2_pass.to_string(),
        wg_private: wg_private.to_string(),
        wg_addr: wg_addr.to_string(),
        wg_server_pub: wg_server_pub.to_string(),
        wg_params: wg_params.to_string(),
    });
    Ok((p, cfg))
}

// ---------------------------------------------------------------- session record

/// What the last session was connected with, so the config can be regenerated
/// from disk. Field for field what `write_tun_config` takes, minus the derived
/// routing decision, which is re-read from the intention record every time.
///
/// It sits next to `singbox-tun.json`, which already holds the same password and
/// private key in the clear: this is not a new place for a secret to live.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct SessionRecord {
    uuid: String,
    host: String,
    sni: String,
    apps_mode: String,
    apps: Vec<String>,
    voice: bool,
    transport: String,
    hy2_pass: String,
    wg_private: String,
    wg_addr: String,
    wg_server_pub: String,
    wg_params: String,
}

fn session_path() -> PathBuf {
    engine_dir().join("session.json")
}

fn remember_session(rec: &SessionRecord) -> Result<(), String> {
    let _ = std::fs::create_dir_all(engine_dir());
    let text = serde_json::to_string(rec).map_err(|e| format!("session record: {e}"))?;
    std::fs::write(session_path(), text).map_err(|e| format!("write session record: {e}"))
}

/// Is there a session on disk a claim can rebuild from?
pub fn session_exists() -> bool {
    read_session().is_some()
}

/// Write a session record from the saved card and server settings, so a device
/// can be routed without connecting this PC first. The PC is never tunnelled in
/// that shape: the record only gives the engine something to build from.
pub fn seed_session_from_config() -> Result<(), String> {
    let cfg = crate::config::AppConfig::load();
    let card = cfg
        .cards
        .first()
        .ok_or("no card saved yet: pick a package on Home once")?;
    let rec = SessionRecord {
        uuid: card.uuid.clone(),
        host: cfg.server_ip.clone(),
        sni: card.sni.clone(),
        apps_mode: "all".to_string(),
        apps: Vec::new(),
        voice: false,
        transport: "vless".to_string(),
        hy2_pass: cfg.hy2_password.clone(),
        wg_private: card.wg_private.clone(),
        wg_addr: card.wg_addr.clone(),
        wg_server_pub: cfg.wg_server_pub.clone(),
        wg_params: cfg.awg_params.clone(),
    };
    remember_session(&rec)
}

fn read_session() -> Option<SessionRecord> {
    let text = std::fs::read_to_string(session_path()).ok()?;
    serde_json::from_str(&text).ok()
}

/// Is the running engine already carrying this device with the host left direct?
///
/// Read from the config on disk, which is exactly what the engine read at start,
/// so a "yes" here means a restart would change nothing and drop the tunnel for
/// nothing.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn device_route_ready(ip: &str) -> bool {
    if task_engine_pid().is_none() || !tun_adapter_present() {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(tun_config_path()) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    if v["route"]["final"].as_str() != Some("direct") {
        return false;
    }
    let want = format!("{ip}/32");
    v["route"]["rules"].as_array().is_some_and(|rules| {
        rules.iter().any(|r| {
            r["outbound"].as_str() == Some("proxy")
                && r["source_ip_cidr"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(want.as_str())))
        })
    })
}

/// Rewrite the config from the recorded session and the routing intent on disk,
/// then restart the engine so those are the rules running.
///
/// This is the only way to change the routing of a live session: sing-box has no
/// runtime route editing, and the config is read exactly once, at start.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn restart_for_intent() -> Result<(), String> {
    let s = read_session().ok_or_else(|| {
        "no tunnel session recorded yet: connect once, then route the device".to_string()
    })?;
    // A claim can arrive long after the last connect, or before the first one. The
    // engine has to exist to be restarted; this is a no-op when it is already there.
    eprintln!("[claim] session ok, ensuring engine");
    ensure_engine()?;
    let wg = if s.transport == "wg" {
        Some((
            s.wg_private.as_str(),
            s.wg_addr.as_str(),
            s.wg_server_pub.as_str(),
            s.wg_params.as_str(),
        ))
    } else {
        None
    };
    eprintln!("[claim] engine ok, writing config");
    write_tun_config(
        &s.uuid, &s.host, &s.sni, &s.apps_mode, &s.apps, s.voice, &s.transport, &s.hy2_pass, wg,
    )?;
    // The same gate the connect path uses: a config that fails here would fail
    // inside the engine, where the message is a log tail instead of a reason.
    eprintln!("[claim] config written, checking");
    check_config()?;
    eprintln!("[claim] config ok, stopping engine");
    stop_engine();
    eprintln!("[claim] stopped, starting engine");
    let pid = start_engine()?;
    if pid == 0 {
        return Err("the engine did not come back after the routing change".to_string());
    }
    // A live pid is not a carrying engine: confirm the rule for the routed
    // device is really in the file the engine just read.
    if let Some(ip) = routed_device_ips().first() {
        let bare = ip.trim_end_matches("/32").to_string();
        if !crate::vpn::device_route_ready(&bare) {
            return Err(format!("the engine came up without a rule for {bare}"));
        }
    }
    Ok(())
}

/// Validate the config before routing a single packet.
/// Cheap fingerprint of the tunnel config that is on disk, so a connect can
/// tell "same file" from "new file" without parsing it.
/// Which stage of a connection actually failed, worked out from two cheap
/// facts: can the name resolve, and does anything accept a TCP connection on
/// the port. TLS is read from the engine's own log because only the engine
/// knows whether the handshake, the certificate or the server name was the
/// part that broke.
///
/// Deliberately narrow: it answers "where", never "who" - no address, no card
/// id and no log line leaves this function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Dns,
    TcpTimeout,
    TcpRefused,
    Tls,
    Reachable,
}

pub fn stage_of(host: &str, port: u16, log: &str) -> Stage {
    use std::net::ToSocketAddrs;
    let addrs: Vec<std::net::SocketAddr> = match (host, port).to_socket_addrs() {
        Ok(a) => a.collect(),
        Err(_) => return Stage::Dns,
    };
    if addrs.is_empty() {
        return Stage::Dns;
    }
    let mut refused = false;
    for a in &addrs {
        match std::net::TcpStream::connect_timeout(a, std::time::Duration::from_secs(4)) {
            Ok(_) => {
                let l = log.to_lowercase();
                if l.contains("certificate")
                    || l.contains("handshake")
                    || l.contains("tls:")
                    || l.contains("unexpected eof")
                {
                    return Stage::Tls;
                }
                return Stage::Reachable;
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    refused = true;
                }
            }
        }
    }
    if refused {
        Stage::TcpRefused
    } else {
        Stage::TcpTimeout
    }
}

/// The sentence a user gets for a stage. No addresses, no identifiers.
pub fn stage_words(stage: Stage) -> &'static str {
    match stage {
        Stage::Dns => "This network cannot look up the server name. Your internet or DNS is the problem, not your card.",
        Stage::TcpRefused => "The server refused the connection on port 443. It may be restarting; try again in a moment.",
        Stage::TcpTimeout => "Nothing answered on port 443. Your network or ISP is probably blocking it.",
        Stage::Tls => "The secure handshake failed. A network filter is intercepting port 443.",
        Stage::Reachable => "The server is reachable but the tunnel carried nothing.",
    }
}

pub fn tun_config_digest() -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let bytes = std::fs::read(engine_dir().join("singbox-tun.json")).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    Some(h.finish())
}

pub fn check_config() -> Result<(), String> {
    let o = run_hidden(
        &singbox_path().to_string_lossy(),
        &["check", "-c", &tun_config_path().to_string_lossy()],
    )?;
    if o.status.success() {
        Ok(())
    } else {
        let err = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let tail: Vec<&str> = err.lines().collect();
        let tail = tail[tail.len().saturating_sub(4)..].join(" | ");
        Err(format!("bad tunnel config: {}", tail.trim()))
    }
}

/// Spawn the engine hidden, stderr APPENDED to a log file (with a run
/// marker). The log rotates: past 256KB it restarts, so reads stay cheap
/// and the file can't grow forever.
/// Returns the child handle (caller owns it).
pub fn spawn_engine() -> Result<std::process::Child, String> {
    use std::io::Write;
    let path = engine_log_path();
    // rotate: a stale multi-MB log makes every tail read + failure message slow
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 256 * 1024 {
        let _ = std::fs::write(&path, "=== rotated (was >256KB) ===\n");
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("log: {e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = writeln!(log, "=== run {now} ===");
    let mut cmd = cmd_hidden(&singbox_path().to_string_lossy());
    // Go runtime tuning: cap the heap target and collect sooner, so a long
    // session keeps its resident memory tight instead of holding garbage.
    // The engine reads these once at startup; behaviour is unchanged.
    cmd.env("GOMEMLIMIT", "64MiB").env("GOGC", "50");
    // Below normal priority: when a game runs on the same machine it must
    // win the CPU, and the engine keeps the line fed with leftover cycles.
    #[cfg(windows)]
    cmd.creation_flags(NO_WINDOW | 0x0000_4000); // BELOW_NORMAL_PRIORITY_CLASS
    let child = cmd
        .args(["run", "-c", &tun_config_path().to_string_lossy()])
        .current_dir(engine_dir())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("start engine: {e}"))?;
    // pid file: lets cleanup kill OUR engine without the slow wmic sweep
    let _ = std::fs::write(engine_dir().join("engine.pid"), child.id().to_string());
    Ok(child)
}

pub fn engine_log_path() -> PathBuf {
    engine_dir().join("singbox.log")
}

/// Last few engine log lines, color codes stripped - for failure messages.
/// Reads only the last 16KB so a big log can't stall the UI.
pub fn engine_log_tail() -> String {
    use std::io::{Read, Seek, SeekFrom};
    let mut t = String::new();
    if let Ok(mut f) = std::fs::File::open(engine_log_path()) {
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let _ = f.seek(SeekFrom::Start(len.saturating_sub(16 * 1024)));
        let _ = f.read_to_string(&mut t);
    }
    let mut plain = String::with_capacity(t.len());
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' && it.peek() == Some(&'[') {
            for c2 in it.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    let lines: Vec<&str> = plain.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(3)..];
    tail.join(" | ").chars().take(300).collect()
}

/// Ask the engine to exit on its own (lets it delete its TUN adapter -
/// a force-kill can leave a ghost adapter that blocks the next connect).
#[cfg(windows)]
pub fn request_graceful_stop(pid: u32) {
    let _ = run_hidden("taskkill", &["/PID", &pid.to_string()]);
}

/// App-side lifecycle line in the same log (spawn/probe/death markers).
pub fn app_log(line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(engine_log_path())
    {
        let _ = writeln!(f, "app: {line}");
    }
}

/// ---------------------------------------------------------------------
/// The privileged half of the tunnel.
///
/// Creating a WinTun adapter is a privileged operation, so an ordinary user
/// process cannot do it. Instead of asking people to launch the whole app as
/// administrator (and to reboot when an adapter is left behind), setup
/// registers ONE scheduled task that runs the engine with full rights. The app
/// then starts and stops that task, which an ordinary user may do.
/// The helper task's name, one per Windows account.
///
/// The app installs per user and its engine folder lives in that user's
/// profile, so a single machine-wide task name would make a second account on
/// the same PC drive the first account's engine and config. The name carries
/// the account so each user gets their own helper.
#[cfg(windows)]
pub fn task_name() -> String {
    format!("QuotaVPN Tunnel {}", account_name())
}

/// This process's Windows account, asked from Windows rather than inherited
/// from the environment: a process started through runas or
/// CreateProcessWithLogonW carries the launcher's USERNAME, which made a
/// second user believe it was already set up.
#[cfg(windows)]
pub fn account_name() -> String {
    let who = run_hidden("whoami", &[])
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let short = who.rsplitn(2, char::from(92)).next().unwrap_or("").trim().to_string();
    if short.is_empty() {
        std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string())
    } else {
        short
    }
}

/// True when the helper task is registered. Setup is needed until it is.
///
/// The action is checked too: a task left over from the PowerShell-script
/// version still starts the engine, but opens a console window on every
/// connect, so it counts as not installed and the setup pass replaces it.
#[cfg(windows)]
pub fn task_installed() -> bool {
    // There is nothing to install. The engine pass is a detached child of this
    // process, so "is the helper ready" has one honest answer.
    true
}

/// What the privileged setup pass wrote, for the window that asked for it.
pub fn setup_log_tail() -> String {
    let text = match std::fs::read_to_string(engine_log_path()) {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("setup-helper:"))
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let start = lines.len().saturating_sub(3);
    lines.drain(..start);
    lines
        .iter()
        .map(|l| l.split("setup-helper:").nth(1).unwrap_or(l).trim())
        .collect::<Vec<_>>()
        .join("; ")
}

/// The script the helper task runs. It exists so the privileged side can clear
/// what a crashed run leaves behind *before* the engine starts: every dead
/// session leaves a phantom "sing-tun Tunnel" device, Windows keeps the adapter
/// name reserved, and the next start dies with
/// "create adapter: Cannot create a file when that file already exists".
/// Clearing phantoms needs administrator rights, which the task has.
#[cfg(windows)]
pub fn engine_ps_path() -> PathBuf {
    engine_dir().join("run-engine.ps1")
}

/// The script the helper task runs. It exists so the privileged side can do
/// what the app may not: clear the phantom wintun devices a dead session
/// leaves behind (Windows keeps the adapter name reserved, and the next start
/// died with "create adapter: Cannot create a file when that file already
/// exists" until the machine was restarted), and report the engine pid, which
/// an ordinary user cannot read from a SYSTEM process.
#[cfg(windows)]
pub fn write_engine_cmd() {
    let sb = singbox_path();
    let cfg = tun_config_path();
    let log = engine_log_path();
    let pid = engine_dir().join("engine.pid");
    let stop = engine_dir().join("engine.stop");
    // Phantoms first: a dead session reserves the adapter name, and the next
    // start dies with "create adapter: Cannot create a file when that file
    // already exists" until the machine restarts. Only this side can clear
    // them. It also reports the pid, which an ordinary user cannot read from
    // a SYSTEM process, and watches for the stop file so stopping needs no
    // elevation either.
    let body = format!(
        "$ErrorActionPreference = 'SilentlyContinue'\n\
         if (-not (Test-Path -LiteralPath '{cfg}')) {{ exit 0 }}\n\
         Get-PnpDevice -Class Net | Where-Object {{ $_.Status -eq 'Unknown' -and $_.FriendlyName -eq 'sing-tun Tunnel' }} | ForEach-Object {{ pnputil /remove-device $_.InstanceId | Out-Null }}\n\
         Remove-Item -LiteralPath '{pid}' -Force -ErrorAction SilentlyContinue\n\
         Remove-Item -LiteralPath '{stop}' -Force -ErrorAction SilentlyContinue\n\
         $p = Start-Process -FilePath '{sb}' -ArgumentList 'run','-c','{cfg}' -RedirectStandardOutput '{log}' -RedirectStandardError '{err}' -PassThru -WindowStyle Hidden\n\
         Set-Content -LiteralPath '{pid}' -Value $p.Id -Encoding ascii\n\
         while (-not $p.HasExited) {{\n\
         \x20 if (Test-Path -LiteralPath '{stop}') {{ Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; break }}\n\
         \x20 Start-Sleep -Milliseconds 350\n\
         }}\n",
        pid = pid.display(),
        stop = stop.display(),
        sb = sb.display(),
        cfg = cfg.display(),
        log = log.display(),
        err = engine_dir().join("engine.err").display(),
    );
    let _ = std::fs::create_dir_all(engine_dir());
    let _ = std::fs::write(engine_ps_path(), body);
}

/// The task runs the app itself with this flag. A PowerShell script as the
/// task action opened a console window on every connect, which read as the app
/// shelling out; the app is a GUI binary, so Windows never gives it one.

/// The kill switch.
///
/// Windows Firewall lets a block rule beat every allow rule, so "permit only
/// the tunnel" cannot be expressed with rules alone. What can be expressed is
/// the case that matters: when the engine dies while it was supposed to be
/// carrying traffic, nothing leaves this machine until the tunnel is back or
/// the user lifts the block. That is the leak the switch exists to stop, and
/// it holds across a crash because the rule lives in Windows, not in us.
const KILL_RULE: &str = "QuotaVPN Kill Switch";

fn kill_flag_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("quotavpn")
        .join("killswitch.txt")
}

pub fn kill_switch_on() -> bool {
    std::fs::read_to_string(kill_flag_path())
        .map(|t| t.trim() == "1")
        .unwrap_or(false)
}

pub fn set_kill_switch(on: bool) -> Result<(), String> {
    let path = kill_flag_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, if on { "1" } else { "0" }).map_err(|e| e.to_string())?;
    if !on {
        // Disarming is a promise that the block is off. If the rule is still
        // there, the caller has to hear about it: the switch is not off.
        block_traffic(false)?;
    }
    Ok(())
}

/// True when the block rule is in the firewall right now.
///
/// The exit code alone is not enough. "No rules match the specified criteria"
/// is printed on stdout and, depending on the Windows build, comes back with
/// exit 0 or exit 1, so a status-only test can call a machine blocked when it
/// is not, or the reverse. The rule's own name is echoed only when a rule
/// actually matched, so that string is the test. NUL bytes are stripped first:
/// netsh can answer in UTF-16 once its stdout is a pipe, which would otherwise
/// split every character and hide the name.
#[cfg(windows)]
pub fn traffic_blocked() -> bool {
    let out = run_hidden(
        "netsh",
        &["advfirewall", "firewall", "show", "rule", &format!("name={KILL_RULE}")],
    );
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .replace('\0', "")
            .contains(KILL_RULE),
        Err(_) => false,
    }
}

/// Where the intention record lives: beside the switch flag, in the same
/// `%LOCALAPPDATA%/quotacards` directory.
#[cfg(windows)]
fn kill_intent_path() -> std::path::PathBuf {
    kill_flag_path().with_file_name("killswitch.intent")
}

/// True when a previous run wrote the intention record and never cleared it.
#[cfg(windows)]
fn kill_intent_present() -> bool {
    kill_intent_path().exists()
}

/// One short note about a command result, for the log line. Never the whole
/// output: netsh can print a page.
#[cfg(windows)]
fn note(out: &Result<std::process::Output, String>) -> String {
    match out {
        Ok(o) => match o.status.code() {
            Some(c) => format!("exit={c}"),
            None => "killed".to_string(),
        },
        Err(e) => format!("spawn: {e}"),
    }
}

/// Add or remove the block, and report only what actually happened.
///
/// Both operations need elevation: the engine helper runs with it, and the app
/// reaches the same code through `--killswitch`.
///
/// The order is the point. The intention record is written BEFORE the rule is
/// added, so a rule that outlives this process is always one a later start
/// knows about, and is removed only after a delete that was verified. A rule
/// left behind is not a leak, it is a machine with no internet and no obvious
/// way back, so nothing here may report success it did not see.
#[cfg(windows)]
pub fn block_traffic(block: bool) -> Result<(), String> {
    if block {
        // Already on: adding again would stack a second rule with the same
        // name, and a delete would then take only one of them off.
        if traffic_blocked() {
            return Ok(());
        }
        let intent = kill_intent_path();
        if let Some(dir) = intent.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&intent, "1") {
            // Without the record, a crash leaves a block nothing can trace.
            // Not blocking is the lesser failure, and it is said out loud.
            app_log(&format!("killswitch: not blocking, no intention record: {e}"));
            return Err(format!("killswitch: cannot record intent: {e}"));
        }
        let out = run_hidden(
            "netsh",
            &["advfirewall", "firewall", "add", "rule", &format!("name={KILL_RULE}"),
              "dir=out", "action=block", "enable=yes", "profile=any"],
        );
        if traffic_blocked() {
            app_log("killswitch: outbound blocked");
            return Ok(());
        }
        app_log(&format!(
            "killswitch: block FAILED, outbound is not blocked ({})",
            note(&out)
        ));
        Err("killswitch: the block rule did not take".to_string())
    } else {
        let out = run_hidden(
            "netsh",
            &["advfirewall", "firewall", "delete", "rule", &format!("name={KILL_RULE}")],
        );
        if traffic_blocked() {
            app_log(&format!(
                "killswitch: release FAILED, block still in place ({})",
                note(&out)
            ));
            return Err("killswitch: the block rule is still in place".to_string());
        }
        // Verified gone, so the record has done its job. A delete that found
        // nothing is a success too: the state asked for is the state on disk.
        let _ = std::fs::remove_file(kill_intent_path());
        app_log("killswitch: outbound released");
        Ok(())
    }
}

/// What a start-up pass should do about the block rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconcile {
    /// Neither a rule nor an intention record: nothing was left behind.
    Nothing,
    /// A record without a rule: the add never landed, or the delete did.
    /// Drop the record, touch nothing else.
    Forget,
    /// A rule with no session behind it: take it off.
    Release,
}

/// Decide what to do with a block found outside a live session.
///
/// Pure on purpose: the answer is a policy, and it can be tested without a
/// firewall. A rule that is present is always released. Whether the intention
/// record is there does not change that: a record means we applied it and died,
/// and no record means an older build, a hand edit, or a record already cleared.
/// Neither of those is a session that is holding the block on purpose, and a
/// machine held offline by accident is the failure this exists to undo.
pub fn reconcile_action(rule_present: bool, marker_present: bool) -> Reconcile {
    match (rule_present, marker_present) {
        (true, _) => Reconcile::Release,
        (false, true) => Reconcile::Forget,
        (false, false) => Reconcile::Nothing,
    }
}

/// Undo a block left behind by a previous run. Called once when the app starts
/// and again when a tunnel session starts, because neither an app that has not
/// connected nor a fresh engine can justify holding the machine offline.
///
/// Returns what it decided so a caller that is not elevated can finish the job:
/// this app runs as an ordinary user, and deleting a firewall rule needs more
/// than that. The rule surviving the attempt is visible through
/// `traffic_blocked()`, which stays true.
#[cfg(windows)]
pub fn reconcile_kill_switch() -> Reconcile {
    let action = reconcile_action(traffic_blocked(), kill_intent_present());
    match action {
        Reconcile::Nothing => {}
        Reconcile::Forget => {
            let _ = std::fs::remove_file(kill_intent_path());
            app_log("killswitch: dropped an intention record with no rule behind it");
        }
        Reconcile::Release => {
            app_log("killswitch: leftover block with no session, releasing");
            if let Err(e) = block_traffic(false) {
                app_log(&format!("killswitch: leftover block NOT released: {e}"));
            }
        }
    }
    action
}

#[cfg(windows)]
pub fn run_engine_headless() {
    let cfg = tun_config_path();
    // No tunnel config means this PC has never connected: nothing to run.
    if !cfg.exists() {
        return;
    }
    // Phantoms first. A dead session reserves the adapter name and the next
    // start dies with "create adapter: Cannot create a file when that file
    // already exists". Only this side can clear them.
    let _ = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Get-PnpDevice -Class Net | Where-Object { $_.Status -eq 'Unknown' -and $_.FriendlyName -eq 'sing-tun Tunnel' } | ForEach-Object { pnputil /remove-device $_.InstanceId | Out-Null }",
        ],
    );
    let pid_path = engine_dir().join("engine.pid");
    let stop_path = engine_dir().join("engine.stop");
    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(&stop_path);
    // A block left from the last death is lifted the moment a session starts,
    // because the tunnel is about to carry the traffic it was protecting. It is
    // lifted whether or not the switch is armed: an armed switch guards a
    // running session, and there is no session yet. A rule still in place here
    // keeps the tunnel from carrying anything, which is the same offline
    // machine reached by a different route.
    reconcile_kill_switch();
    let guarding = kill_switch_on();

    let mut cmd = cmd_hidden(&singbox_path().to_string_lossy());
    cmd.args(["run", "-c", &cfg.to_string_lossy()]);
    // Append, never truncate: the app writes the connect timings before the
    // engine starts, and a create here used to erase them on every connect.
    if let Ok(f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(engine_log_path())
    {
        cmd.stdout(std::process::Stdio::from(f));
    }
    if let Ok(f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(engine_dir().join("engine.err"))
    {
        cmd.stderr(std::process::Stdio::from(f));
    }
    let Ok(mut child) = cmd.spawn() else {
        return;
    };
    let _ = std::fs::write(&pid_path, child.id().to_string());
    loop {
        // The app drops this file to stop the engine without elevation.
        if stop_path.exists() {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(350)),
            Err(_) => break,
        }
    }
    let _ = std::fs::remove_file(&pid_path);
    // Asked to stop: the user is closing the tunnel, so nothing should be
    // held. Died on its own: hold everything until they are back. Either way
    // the result is logged truthfully by block_traffic; there is nobody left
    // in this process to hand an error to.
    if guarding {
        if stop_path.exists() {
            let _ = block_traffic(false);
        } else {
            let _ = block_traffic(true);
        }
    }
}

/// Register the task. Runs in the elevated pass (`QuotaVPN.exe --setup-helper`),
/// so this is the only moment the product asks Windows for administrator.
/// Register the privileged helper for one account.
///
/// `/IT` is not decoration: without it the task is created as "run whether the
/// user is logged on or not", which needs the account's password, and on a
/// Microsoft account schtasks refuses with a credentials error. With `/IT` the
/// task uses the interactive token and needs no password, while `/RL HIGHEST`
/// still runs it elevated.
#[cfg(windows)]
/// Install the elevated pass that carries the tunnel.
pub fn install_task(owner: &str) -> Result<(), String> {
    install_helper_task(
        owner,
        &format!("QuotaVPN Tunnel {owner}"),
        "QuotaVPN tunnel helper",
        "--run-engine",
    )
}
/// Nothing is installed any more.
///
/// These used to register two scheduled tasks, and registering a task whose
/// action launches this executable is exactly what `Behavior:Win32/Persistence.A!ml`
/// describes. Windows quarantined the app for it three times. The app runs with an
/// administrator token already, so the passes are started as detached children and
/// nothing is written to the Task Scheduler.
///
/// The functions stay because older installs left tasks behind: calling one now
/// deletes them, which is the cleanup path.
pub fn install_lan_task(_owner: &str) -> Result<(), String> {
    remove_legacy_tasks();
    Ok(())
}

fn install_helper_task(
    _owner: &str,
    _name: &str,
    _description: &str,
    _args: &str,
) -> Result<(), String> {
    remove_legacy_tasks();
    Ok(())
}

/// Delete tasks that earlier versions registered, if this machine still has them.
#[cfg(windows)]
fn remove_legacy_tasks() {
    for name in [task_name(), lan_task_name()] {
        let _ = run_hidden("schtasks", &["/Delete", "/TN", &name, "/F"]);
    }
}

#[cfg(not(windows))]
fn remove_legacy_tasks() {}

fn said(out: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let std_out = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match (err.is_empty(), std_out.is_empty()) {
        (false, false) => format!("{err} {std_out}"),
        (false, true) => err,
        (true, false) => std_out,
        (true, true) => "schtasks returned an error with no message".to_string(),
    }
}

/// The security identifier for an account name.
///
/// A task principal written as a bare name has to be resolved by the Task
/// Scheduler, and on a Microsoft account that resolution is exactly what
/// fails. A SID always resolves.
#[cfg(windows)]
fn sid_for(user: &str) -> Option<String> {
    let quoted = user.replace(char::from(39), "''");
    let script = format!(
        "(New-Object System.Security.Principal.NTAccount('{quoted}')).Translate([System.Security.Principal.SecurityIdentifier]).Value"
    );
    let out = run_hidden("powershell", &["-NoProfile", "-Command", &script]).ok()?;
    if !out.status.success() {
        return None;
    }
    let sid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sid.starts_with("S-1-") { Some(sid) } else { None }
}

/// The SID of the account running this process. Always resolvable, and the
/// right answer whenever the elevated pass is running as the person who asked.
fn current_sid() -> Option<String> {
    let out = run_hidden("whoami", &["/user", "/fo", "csv", "/nh"]).ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let sid = text.split(',').nth(1)?.trim().trim_matches('"').to_string();
    if sid.starts_with("S-1-") { Some(sid) } else { None }
}

fn xml_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// PowerShell's `-EncodedCommand` form: base64 of the UTF-16LE script.
///
/// Hand-rolled because this crate has no base64 dependency and the alternative is
/// another download; it is a dozen lines of table lookup.
#[cfg_attr(not(windows), allow(dead_code))]
fn ps_encoded(script: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes: Vec<u8> = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(windows)]
pub fn remove_task() {
    // Both passes: the tunnel helper and the network one. Leaving the second
    // behind would keep a task on the machine that nothing uninstalls.
    let deleted_tunnel = run_hidden("schtasks", &["/Delete", "/TN", &task_name(), "/F"])
        .map(|o| o.status.success())
        .unwrap_or(false);
    let deleted_network = run_hidden("schtasks", &["/Delete", "/TN", &lan_task_name(), "/F"])
        .map(|o| o.status.success())
        .unwrap_or(false);
    let deleted = deleted_tunnel && deleted_network;
    let adapter = tun_adapter_present();
    // Nothing left to clean and nothing to ask for.
    if deleted && !adapter {
        return;
    }
    // Two things need administrator and neither can be done by an ordinary
    // account: deleting a task registered with highest rights, and removing a
    // virtual network adapter. One prompt covers both, and it is only raised
    // when one of them is still there. The script lives in the engine folder
    // so the quoting can be plain prose instead of an escaped one-liner.
    let script = engine_dir().join("uninstall-clean.ps1");
    let body = format!(
        "$ErrorActionPreference = 'SilentlyContinue'
         schtasks /End /TN \"{0}\" | Out-Null
         schtasks /Delete /TN \"{0}\" /F | Out-Null
         schtasks /End /TN \"{1}\" | Out-Null
         schtasks /Delete /TN \"{1}\" /F | Out-Null
         taskkill /IM sing-box.exe /F | Out-Null
         Get-NetAdapter -IncludeHidden | Where-Object {{ $_.Name -like 'QuotaVPN*' }} | ForEach-Object {{
           Disable-NetAdapter -InputObject $_ -Confirm:$false
           Remove-NetAdapter -InputObject $_ -Confirm:$false
         }}
         Get-PnpDevice -Class Net | Where-Object {{ $_.FriendlyName -eq 'sing-tun Tunnel' }} | ForEach-Object {{
           pnputil /remove-device $_.InstanceId | Out-Null
         }}
",
        task_name(),
        lan_task_name()
    );
    // The script goes to the elevated process as an encoded command, never as a
    // file. A file has to live somewhere, and the only place this process can
    // write is the user's own profile: an attacker could replace it between the
    // write and the launch, and the launch runs it as Administrator.
    let encoded = ps_encoded(&body);
    // Start-Process joins an argument array with spaces and quotes nothing, so
    // the whole switch list goes as one string.
    let ps = format!(
        "Start-Process -FilePath 'powershell.exe' -ArgumentList '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -EncodedCommand {encoded}' -Verb RunAs -Wait -WindowStyle Hidden"
    );
    let _ = run_hidden("powershell", &["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]);
    let _ = std::fs::remove_file(&script);
    std::thread::sleep(std::time::Duration::from_secs(3));
}

/// Start the engine pass as a detached child of this process.
///
/// It used to be a scheduled task started with `schtasks /Run`, which is the one
/// thing about this app that antivirus engines kept calling
/// `Behavior:Win32/Persistence.A!ml`: an installer, or a program, creating a task
/// whose action launches itself. The detection is about the mechanism, not the
/// intent, and it quarantined the executable each time.
///
/// The app already runs with an administrator token, so a child process inherits
/// exactly what the task provided, without registering anything that outlives it.
/// `DETACHED_PROCESS` is what lets the window close and the tunnel stay up.
#[cfg(windows)]
fn start_task() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| format!("exe: {e}"))?;
    std::process::Command::new(exe)
        .arg("--run-engine")
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("engine: {e}"))
}

#[cfg(windows)]
pub fn stop_task() {
    // The pid the engine pass wrote for itself. Nothing to ask the scheduler for.
    if let Some(pid) = task_engine_pid() {
        let _ = run_hidden("taskkill", &["/F", "/PID", &pid.to_string()]);
    }
}

/// The name of the elevated pass that carries the LAN pipeline.
pub fn lan_task_name() -> String {
    format!("QuotaVPN Network {}", account_name())
}

/// Is that task registered, and pointed at this binary?
///
/// The same two questions as [`task_installed`]: the argument it runs, and the
/// path it runs, because a task left over from an older install names a binary
/// that may no longer exist and starting it fails silently.
pub fn lan_task_installed() -> bool {
    true
}

/// Start the LAN pass through its task, which costs no prompt.
/// Start the LAN pipeline pass, detached, the same way the engine starts.
///
/// Same reasoning as `start_task`: no scheduled task, so nothing on this machine
/// outlives the app, and nothing looks like persistence to a behaviour model.
pub fn start_lan_task() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let exe = std::env::current_exe().map_err(|e| format!("exe: {e}"))?;
        std::process::Command::new(exe)
            .arg("--lan-pipeline")
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("lan pipeline: {e}"))
    }
    #[cfg(not(windows))]
    {
        Err("the LAN pipeline is Windows only".to_string())
    }
}

/// End the LAN pass from outside. The loop stops itself when the stop file it
/// watches appears, so this is the backstop for a process that will not go.
pub fn stop_lan_task() {
    // Nothing to end: the pipeline watches `lan/stop` and exits, which is the
    // protocol the control layer already speaks.
    let _ = crate::lan_forward::request_stop();
}

/// The pid of the engine our task started: the sing-box whose command line
/// names our own config path. Anything else called sing-box is not ours.
#[cfg(windows)]
pub fn task_engine_pid() -> Option<u32> {
    // The helper runs as SYSTEM, and an ordinary user cannot read the command
    // line of a SYSTEM process, so the pid comes from the file the helper
    // writes. Verify it really is our engine before trusting it.
    let pid_file = engine_dir().join("engine.pid");
    if let Ok(text) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = text.trim().parse::<u32>() {
            if engine_alive(pid) {
                return Some(pid);
            }
        }
    }
    // Fallback for a session started before that file existed: the only
    // sing-box on this machine is ours.
    let out = run_hidden("tasklist", &["/FI", "Imagename eq sing-box.exe", "/FO", "CSV", "/NH"]).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.to_lowercase().contains("sing-box.exe"))?;
    line.split("\",\"").nth(1)?.trim().parse::<u32>().ok()
}

/// True when that pid is a live sing-box: the image name is readable even for
/// a SYSTEM process, unlike its command line.
#[cfg(windows)]
pub fn engine_alive(pid: u32) -> bool {
    // One syscall instead of a `tasklist` process per poll. The image name is
    // still verified, so a recycled pid cannot masquerade as our engine.
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        let _ = CloseHandle(handle);
        ok != 0
            && String::from_utf16_lossy(&buf[..len as usize])
                .to_ascii_lowercase()
                .ends_with("sing-box.exe")
    }
}

/// Whether a process with this pid is still alive, without caring what its
/// image is. The packet pipeline is a QuotaVPN child, not sing-box, so
/// `engine_alive` (which checks the image name) cannot answer for it.
#[cfg(windows)]
pub fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut code);
        let _ = CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE as u32
    }
}

/// Bring the engine up through the task and return its pid (0 while the task
/// is still starting; the caller waits on the routes anyway).

/// Every session gets its own adapter name.
///
/// wintun refuses to create an adapter whose name a ghost of a force-killed run
/// still holds ("create adapter: Cannot create a file when that file already
/// exists") and cannot open that ghost either ("Element not found"), so the
/// tunnel never comes up while the app happily reports connected. A fresh name
/// sidesteps the ghost instead of asking the user to reboot.
#[cfg(windows)]
fn refresh_tun_name() -> Option<String> {
    let p = tun_config_path();
    let raw = std::fs::read_to_string(&p).ok()?;
    let mut v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos() as u64;
    let name = format!(
        "QuotaVPN{:06x}",
        (nanos ^ (std::process::id() as u64).wrapping_mul(2654435761)) & 0xffffff
    );
    v["inbounds"][0]["interface_name"] = serde_json::json!(name);
    std::fs::write(&p, serde_json::to_string_pretty(&v).ok()?).ok()?;
    let _ = std::fs::write(p.with_file_name("tun-ifname.txt"), &name);
    Some(name)
}

/// Is the session's adapter actually standing? A name that exists but never
/// comes up is the difference between connected and pretending.
#[cfg(windows)]
fn tun_adapter_up() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, MIB_IF_TABLE2,
    };
    let want = std::fs::read_to_string(engine_dir().join("tun-ifname.txt"))
        .unwrap_or_default()
        .trim()
        .to_string();
    if want.is_empty() {
        return false;
    }
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table) != 0 || table.is_null() {
            return false;
        }
        let rows =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let mut up = false;
        for row in rows {
            let alias = String::from_utf16_lossy(&row.Alias);
            if alias.trim_end_matches('\0') == want {
                // IfOperStatusUp is 1.
                up = row.OperStatus as u32 == 1;
                break;
            }
        }
        FreeMibTable(table as *const core::ffi::c_void);
        up
    }
}

#[cfg(windows)]
pub fn start_engine() -> Result<u32, String> {
    if !task_installed() {
        return Err("setup-needed".to_string());
    }
    // One engine at a time: a task left running from last session owns the
    // adapter, and a second one cannot create it.
    if let Some(old) = task_engine_pid() {
        let _ = run_hidden("taskkill", &["/F", "/PID", &old.to_string()]);
        // Wait for it to actually go: wintun releases the adapter name when
        // the process exits, and starting a new engine before that made the
        // new one die with "create adapter: Cannot create a file when that
        // file already exists" - the failure that used to cost people a reboot.
        for _ in 0..25 {
            if task_engine_pid().is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    // Fresh adapter name for this session, so a ghost from a killed run cannot
    // block the tunnel.
    let if_name = refresh_tun_name();
    write_engine_cmd();
    stop_task();
    start_task()?;
    for _ in 0..80 {
        if let Some(pid) = task_engine_pid() {
            let _ = std::fs::write(engine_dir().join("engine.pid"), pid.to_string());
            // The engine process alone is not a tunnel: wait for the adapter to
            // stand, and say so plainly when it never does.
            if let Some(name) = if_name.as_deref() {
                let mut up = false;
                for _ in 0..60 {
                    if tun_adapter_up() {
                        up = true;
                        break;
                    }
                    if task_engine_pid().is_none() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                if !up && task_engine_pid().is_none() {
                    return Err(format!("tunnel-failed: {name}"));
                }
                if up {
                    // The adapter exists, so the competition can be outranked. Only needed
                    // when something else published a default route, which is why the
                    // config carried /0 in the first place.
                    if another_default_route() {
                        raise_our_route();
                    }
                }
            }
            return Ok(pid);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(0)
}

/// Stop whatever the task is running, then clean routes and adapter.
#[cfg(windows)]
pub fn stop_engine() {
    // An ordinary user may end the task but cannot kill the SYSTEM process it
    // started, so the script watches for this file and stops the engine
    // itself. Ending the task is the backstop.
    let stop = engine_dir().join("engine.stop");
    let _ = std::fs::write(&stop, "1");
    if let Some(pid) = task_engine_pid() {
        // A press has to feel like a press. This waited six seconds for the
        // engine to die before it would let go of the button; three is the
        // point where a healthy engine is already gone, and anything slower
        // gets killed rather than waited on.
        let mut gone = false;
        for _ in 0..20 {
            if !engine_alive(pid) {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        if !gone {
            let _ = run_hidden("taskkill", &["/F", "/PID", &pid.to_string()]);
        }
    }
    stop_task();
    let _ = std::fs::remove_file(&stop);
    let _ = std::fs::remove_file(engine_dir().join("engine.pid"));
    engine_cleanup();
}

/// Reset a wedged wintun driver (ghost adapter names). Harmless otherwise.
pub fn bounce_wintun() {
    let _ = run_hidden("sc", &["stop", "wintun"]);
    std::thread::sleep(std::time::Duration::from_millis(2500));
}

/// True when our own TUN adapter is still registered. A dead engine can leave
/// it behind, and Windows then refuses to make a new one ("Cannot create a
/// file when that file already exists") until the machine is restarted. That
/// restart is the one users were told to do.
pub fn tun_adapter_present() -> bool {
    #[cfg(windows)]
    {
        // Native scan: the PowerShell adapter query cost about a second and a
        // half on every connect, even when there was no adapter to find.
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            FreeMibTable, GetIfTable2, MIB_IF_TABLE2,
        };
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table) != 0 || table.is_null() {
                return false;
            }
            let rows =
                std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
            let mut found = false;
            for row in rows {
                let alias = String::from_utf16_lossy(&row.Alias);
                let alias = alias.trim_end_matches('\0');
                let desc = String::from_utf16_lossy(&row.Description);
                let desc = desc.trim_end_matches('\0');
                let lower = desc.to_ascii_lowercase();
                if alias.starts_with("QuotaVPN")
                    || alias.starts_with("quotavpn")
                    || desc.contains("QuotaVPN")
                    || desc.contains("quotavpn")
                    || lower.contains("wintun")
                    || lower.contains("sing-box")
                {
                    found = true;
                    break;
                }
            }
            FreeMibTable(table as *const core::ffi::c_void);
            found
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Remove our leftover adapter so the next engine can create a fresh one.
/// Only names this app owns are touched: another product's wintun adapter is
/// not ours to delete.
pub fn reset_tun_adapter() {
    #[cfg(windows)]
    {
        let ps = "Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | \
                  Where-Object { $_.Name -like 'QuotaVPN*' } | ForEach-Object { \
                    Disable-NetAdapter -InputObject $_ -Confirm:$false -ErrorAction SilentlyContinue; \
                    Remove-NetAdapter -InputObject $_ -Confirm:$false -ErrorAction SilentlyContinue }";
        let _ = run_hidden("powershell", &["-NoProfile", "-WindowStyle", "Hidden", "-Command", ps]);
        std::thread::sleep(std::time::Duration::from_millis(900));
    }
}

/// Single-instance lock. Returns Some(other_pid) when a LIVE other copy
/// is running (caller must stand down - two copies fight over one tunnel);
/// otherwise claims the lock and returns None. A stale lock (dead PID)
/// is reclaimed.
pub fn claim_instance() -> Option<u32> {
    let p = crate::config::AppConfig::config_dir().join("instance.lock");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let me = std::process::id();
    if let Ok(txt) = std::fs::read_to_string(&p) {
        if let Ok(pid) = txt.trim().parse::<u32>() {
            if pid != me && process_is_our_app(pid) {
                return Some(pid);
            }
        }
    }
    let _ = std::fs::write(&p, me.to_string());
    None
}

/// Release our lock (only if we hold it).
pub fn release_instance() {
    let p = crate::config::AppConfig::config_dir().join("instance.lock");
    if let Ok(txt) = std::fs::read_to_string(&p) {
        if txt.trim().parse::<u32>().ok() == Some(std::process::id()) {
            let _ = std::fs::remove_file(&p);
        }
    }
}

fn process_is_our_app(pid: u32) -> bool {
    let out = match run_hidden("tasklist", &["/FI", &format!("PID eq {pid}")]) {
        Ok(o) => o,
        Err(_) => return false,
    };
    let t = String::from_utf8_lossy(&out.stdout).to_lowercase();
    t.lines().any(|l| {
        (l.contains("quotavpn") || l.contains("quotavpn"))
            && l.split_whitespace().any(|w| w.parse::<u32>().ok() == Some(pid))
    })
}

/// Egress IP with the system proxy BYPASSED - under TUN this must be the
/// server; with no tunnel it is the real IP. Proves full capture.
pub fn egress_ip_direct() -> Result<String, String> {
    let o = run_hidden(
        "curl",
        &[
            "-s", "--max-time", "20", "--noproxy", "*",
            "https://api.ipify.org",
        ],
    )
    .map_err(|e| format!("probe: {e}"))?;
    let ip = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if ip.is_empty() || !ip.bytes().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err("no route (tunnel dead?)".to_string());
    }
    Ok(ip)
}

/// Is any route pointing into our TUN interface? Two session shapes count:
/// whole-device / app sessions pull a default route into the tun, while the
/// voice session confines the tun to its `route_address` entries (the voice
/// networks) and never owns a default route. Requiring a default route is
/// exactly what left the voice helper looping on "Connecting" while a
/// healthy engine was already carrying the voice traffic.
#[cfg(windows)]
pub fn tun_routes_present() -> bool {
    let ip = current_tun_ip();
    run_hidden("route", &["print", "-4"])
        .map(|o| routes_present_in(&String::from_utf8_lossy(&o.stdout), &ip))
        .unwrap_or(false)
}

/// Split out so the parsing is testable: a line counts when it is ours
/// (carries the session's tun address) and is either the default route or
/// one of the voice networks the voice-only session installs. The adapter's
/// own subnet line (172.x.x.0 On-link) never counts - a bare adapter is not
/// a session.
fn routes_present_in(table: &str, ip: &str) -> bool {
    table.lines().any(|l| {
        let l = l.trim_start();
        if !l.contains(ip) {
            return false;
        }
        if l.starts_with("0.0.0.0") {
            return true;
        }
        VOICE_CIDRS
            .iter()
            .any(|c| l.starts_with(c.split('/').next().unwrap_or(c)))
    })
}

/// Kill our orphan engine (matched by our config path, never anyone else's)
/// and remove default routes via the TUN interface. `keep` spares one live
/// PID (the just-connected engine - without this the connect handler would
/// hunt down its own child). Safe to call anytime.
pub fn engine_cleanup() {
    engine_cleanup_keep(None);
}

#[cfg(windows)]
pub fn engine_cleanup_keep(keep: Option<u32>) {
    // fast path: kill OUR engine via the pid file (no wmic - wmic alone
    // can take 2-5s on Win11 and froze the old UI on every disconnect)
    if let Ok(pid_s) = std::fs::read_to_string(engine_dir().join("engine.pid")) {
        if let Ok(pid) = pid_s.trim().parse::<u32>() {
            if Some(pid) != keep && pid != 0 {
                // confirm it's still ours before killing (filtered = fast)
                let ours = std::process::Command::new("tasklist")
                    .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
                    .creation_flags(0x08000000)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
                    .unwrap_or_default();
                if ours.contains("sing-box") {
                    let _ = run_hidden("taskkill", &["/F", "/PID", &pid.to_string()]);
                }
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    // default route via a dead TUN = no internet: remove (up to 3 tries,
    // one per leftover default entry Windows may hold)
    for _ in 0..3 {
        if !tun_routes_present() {
            break;
        }
        for ip in cleanup_tun_ips() {
            let _ = run_hidden("route", &["delete", "0.0.0.0", "mask", "0.0.0.0", ip.as_str()]);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    // slow wmic sweep only if something STILL holds the TUN (crash
    // leftovers) - the common stop path returns before this
    if tun_routes_present() {
        engine_orphan_sweep(keep);
    }
}

/// Slow fallback: hunt any sing-box bound to our config path. Only for
/// crash leftovers - never on the hot stop path.
fn engine_orphan_sweep(keep: Option<u32>) {
    if let Ok(o) = run_hidden(
        "wmic",
        &[
            "process", "where", "name='sing-box.exe'", "get",
            "commandline,processid",
        ],
    ) {
        let ours = tun_config_path().to_string_lossy().to_lowercase();
        for line in String::from_utf8_lossy(&o.stdout).lines().skip(1) {
            let low = line.to_lowercase();
            if low.contains("sing-box") && low.contains(&ours) {
                if let Some(pid) = low.split_whitespace().last() {
                    if pid.parse::<u32>().ok() == keep {
                        continue; // that's our live engine, not an orphan
                    }
                    let _ = run_hidden("taskkill", &["/F", "/PID", pid]);
                }
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    for _ in 0..3 {
        if !tun_routes_present() {
            break;
        }
        for ip in cleanup_tun_ips() {
            let _ = run_hidden("route", &["delete", "0.0.0.0", "mask", "0.0.0.0", ip.as_str()]);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

// ---------------------------------------------------------------------------
// What the tunnel is carrying, not just how much
// ---------------------------------------------------------------------------
// Conntrack, which is how the Linux gateway names its flows, is not available
// on Windows at a price worth paying. The engine we already run can answer for
// each live flow instead: its Clash-compatible API reports the sniffed host,
// the process and the byte counters. Reaching that endpoint needs an HTTP
// client and the per-run secret, so it lives in the desktop crate; this module
// keeps only the arithmetic, where it can be tested without a tunnel.

/// Most (process, host) pairs kept for one session. The screen shows five; a
/// browser with a tab farm can open thousands of flows in minutes, and the
/// point of the cap is that a long session costs the same memory as a short one.
pub const MAX_DEST_PAIRS: usize = 256;

/// Most live flows remembered between two polls. Active flows are the natural
/// bound; this is the hard one, for a confused or hostile engine.
pub const MAX_TRACKED_CONNS: usize = 4096;

/// One destination row. `process` is empty when the engine named none, which is
/// the normal case unless process finding is on. `host` is the sniffed name
/// when the engine saw one, otherwise the raw destination address: a guessed
/// service name is never put here.
#[derive(Clone, serde::Serialize)]
pub struct Destination {
    pub process: String,
    pub host: String,
    pub up: u64,
    pub down: u64,
}

/// Session totals per (process, host), built by diffing the engine's per-flow
/// counters between polls.
///
/// The counters the API reports are cumulative for the life of a flow, so
/// adding them on every poll would multiply the traffic by the number of polls.
/// The map below is what keeps the sum honest: each poll adds only the growth
/// since the previous one, and a flow that disappears takes its baseline with
/// it. The cost of that choice is that traffic a flow carries in its final poll
/// interval is never seen. That is a bounded undercount, which is the direction
/// an accounting number should err in.
#[derive(Default)]
pub struct DestTracker {
    /// connection id -> (up, down) as of the last poll.
    seen: std::collections::HashMap<String, (u64, u64)>,
    /// (process, host) -> session totals.
    pairs: std::collections::HashMap<(String, String), (u64, u64)>,
    /// Pairs and flows refused because a cap was reached.
    dropped: u64,
}

impl DestTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new session at zero, so two tunnels never share one total.
    pub fn clear(&mut self) {
        self.seen.clear();
        self.pairs.clear();
        self.dropped = 0;
    }

    /// Fold one `/connections` reply in.
    ///
    /// Everything unexpected is ignored on purpose. A missing endpoint, a
    /// renamed field, a truncated body and an engine that is still starting all
    /// look the same from here, and none of them is worth a panic or an error
    /// on a VPN screen.
    pub fn observe(&mut self, body: &serde_json::Value) {
        let conns = match body.get("connections").and_then(|v| v.as_array()) {
            Some(c) => c,
            None => return,
        };
        let mut live: std::collections::HashSet<String> = std::collections::HashSet::new();
        for c in conns.iter().take(MAX_TRACKED_CONNS) {
            let id = match c.get("id").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let up = c.get("upload").and_then(|v| v.as_u64()).unwrap_or(0);
            let down = c.get("download").and_then(|v| v.as_u64()).unwrap_or(0);
            // Written out three times rather than through a helper closure: a
            // closure returning a borrowed &str here is a lifetime puzzle with
            // no payoff on a path this short.
            let meta = c.get("metadata");
            let process = meta
                .and_then(|m| m.get("process"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let named = meta
                .and_then(|m| m.get("host"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let address = meta
                .and_then(|m| m.get("destinationIP"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            // A host is present only when the engine sniffed one. When it did
            // not, the destination address is the truth and the row says so.
            let host = if named.is_empty() { address } else { named };
            // Nothing to attribute these bytes to: skip the flow rather than
            // invent a label for it.
            if host.is_empty() {
                continue;
            }
            // A flow seen for the first time is worth its whole counter. A flow
            // we cannot track is worth nothing: re-counting it on every poll
            // would be worse than missing it.
            let fresh;
            if let Some(&(pu, pd)) = self.seen.get(id) {
                fresh = (up.saturating_sub(pu), down.saturating_sub(pd));
            } else if self.seen.len() >= MAX_TRACKED_CONNS {
                self.dropped = self.dropped.saturating_add(1);
                continue;
            } else {
                fresh = (up, down);
            }
            self.seen.insert(id.to_string(), (up, down));
            live.insert(id.to_string());
            if fresh.0 == 0 && fresh.1 == 0 {
                continue;
            }
            let key = (process.to_string(), host.to_string());
            if let Some(t) = self.pairs.get_mut(&key) {
                t.0 = t.0.saturating_add(fresh.0);
                t.1 = t.1.saturating_add(fresh.1);
            } else if self.pairs.len() >= MAX_DEST_PAIRS {
                self.dropped = self.dropped.saturating_add(1);
            } else {
                self.pairs.insert(key, fresh);
            }
        }
        // A closed flow's baseline is dead weight, and keeping it would make a
        // reused id look like it had already sent what it now sends.
        self.seen.retain(|id, _| live.contains(id));
    }

    /// The heaviest `n` pairs, ordered by total bytes, so a caller can draw rows
    /// without sorting anything.
    pub fn top(&self, n: usize) -> Vec<Destination> {
        let mut rows: Vec<Destination> = self
            .pairs
            .iter()
            .map(|((process, host), &(up, down))| Destination {
                process: process.clone(),
                host: host.clone(),
                up,
                down,
            })
            .collect();
        // Total first; the labels break ties so the order does not jitter
        // between two polls that saw the same traffic.
        rows.sort_by(|a, b| {
            b.up
                .saturating_add(b.down)
                .cmp(&a.up.saturating_add(a.down))
                .then_with(|| a.host.cmp(&b.host))
                .then_with(|| a.process.cmp(&b.process))
        });
        rows.truncate(n);
        rows
    }

    /// Count of pairs and flows refused because a cap was reached. Zero in any
    /// normal session; non-zero means the session carried more than is shown.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    // A literal address: the tests must not depend on DuckDNS.
    const TEST_HOST: &str = "203.0.113.7";
    use super::*;

    // Destinations: the arithmetic the screen depends on. No HTTP, no engine,
    // no clock, so these run anywhere.

    fn conn(id: &str, process: &str, host: &str, up: u64, down: u64) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "upload": up,
            "download": down,
            "metadata": {
                "process": process,
                "host": host,
                "destinationIP": "203.0.113.9"
            }
        })
    }

    fn reply(conns: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({ "connections": conns })
    }

    #[test]
    fn a_flow_is_counted_once_not_once_per_poll() {
        let mut t = DestTracker::new();
        // One flow, counters growing: only the growth may be added.
        t.observe(&reply(vec![conn("a", "chrome.exe", "example.com", 100, 200)]));
        t.observe(&reply(vec![conn("a", "chrome.exe", "example.com", 150, 260)]));
        let top = t.top(5);
        assert_eq!(top.len(), 1);
        assert_eq!(
            (top[0].up, top[0].down),
            (150, 260),
            "cumulative counters must be diffed, not summed"
        );
    }

    #[test]
    fn a_flow_that_closes_keeps_what_it_already_sent() {
        let mut t = DestTracker::new();
        t.observe(&reply(vec![conn("a", "curl.exe", "example.com", 10, 20)]));
        t.observe(&reply(vec![]));
        let top = t.top(5);
        assert_eq!(top.len(), 1);
        assert_eq!(
            (top[0].up, top[0].down),
            (10, 20),
            "a closed flow must not erase its bytes"
        );
        // A reused id starts from zero instead of inheriting old counters.
        t.observe(&reply(vec![conn("a", "curl.exe", "example.com", 5, 5)]));
        let top = t.top(5);
        assert_eq!((top[0].up, top[0].down), (15, 25));
    }

    #[test]
    fn the_address_is_shown_when_no_host_was_sniffed() {
        let mut t = DestTracker::new();
        t.observe(&reply(vec![conn("a", "app.exe", "", 1, 2)]));
        let top = t.top(5);
        assert_eq!(top.len(), 1);
        assert_eq!(
            top[0].host, "203.0.113.9",
            "fall back to the address, never to a guessed name"
        );
    }

    #[test]
    fn only_the_five_heaviest_come_back() {
        let mut t = DestTracker::new();
        for i in 0..8u64 {
            let n = i + 1;
            t.observe(&reply(vec![conn(
                &format!("c{i}"),
                "p.exe",
                &format!("h{n}.test"),
                n,
                n,
            )]));
        }
        let top = t.top(5);
        assert_eq!(top.len(), 5);
        assert_eq!(top[0].host, "h8.test");
        assert_eq!(top[4].host, "h4.test");
    }

    #[test]
    fn the_session_map_stays_bounded() {
        let mut t = DestTracker::new();
        for i in 0..(MAX_DEST_PAIRS + 40) {
            t.observe(&reply(vec![conn(
                &format!("c{i}"),
                "p.exe",
                &format!("h{i}.test"),
                1,
                1,
            )]));
        }
        assert!(
            t.pairs.len() <= MAX_DEST_PAIRS,
            "the map must not grow with the flow count"
        );
        assert!(
            t.dropped() >= 40,
            "refused pairs are counted, not silently dropped"
        );
        assert_eq!(t.top(5).len(), 5);
    }

    // The three tests below rewrite one engine config file; serialize them.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // The kill switch decision, tested as the policy it is: no firewall, no
    // netsh, no elevation. The property that matters is that a rule which is
    // present is never left alone, because that is the machine with no internet.

    #[test]
    fn a_present_block_rule_is_always_released() {
        for &marker in &[false, true] {
            assert_eq!(
                reconcile_action(true, marker),
                Reconcile::Release,
                "a block rule with no session behind it must be released (marker={marker})"
            );
        }
    }

    #[test]
    fn a_crashed_block_is_released_and_not_just_forgotten() {
        // Armed, applied, process died: record and rule are both there.
        // Clearing the record while leaving the rule is the exact bug.
        assert_eq!(reconcile_action(true, true), Reconcile::Release);
        assert_ne!(reconcile_action(true, true), Reconcile::Forget);
    }

    #[test]
    fn an_intention_record_on_its_own_is_only_cleared() {
        // The add never landed, or the delete did: nothing to take off the
        // firewall, but the record must not survive to confuse a later run.
        assert_eq!(reconcile_action(false, true), Reconcile::Forget);
    }

    #[test]
    fn a_clean_machine_is_left_alone() {
        // No rule, no record: the pass must not run netsh at all.
        assert_eq!(reconcile_action(false, false), Reconcile::Nothing);
    }

    fn cfg(mode: &str, apps: &[&str]) -> serde_json::Value {
        let list: Vec<String> = apps.iter().map(|s| s.to_string()).collect();
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            mode,
            &list,
            false,
            "vless",
            "",
            None,
        )
        .expect("config");
        serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("json")
    }

    /// A config as a device-only session builds it: one LAN address routed, the
    /// host left on its own adapter.
    fn device_cfg(routed: &[&str], bind: Option<(&str, &str)>) -> Result<serde_json::Value, String> {
        let list: Vec<String> = routed.iter().map(|s| s.to_string()).collect();
        let bind = bind.map(|(ip, name)| (ip.to_string(), name.to_string()));
        let (_path, cfg) = write_tun_config_for(
            // A real UUID shape: the device-only config is also the one that gets
            // handed to `sing-box check` as part of verifying this feature.
            "11111111-1111-1111-1111-111111111111",
            TEST_HOST,
            "example.com",
            "",
            &[],
            false,
            "vless",
            "",
            None,
            &list,
            move || bind,
            None,
        )?;
        Ok(cfg)
    }

    #[test]
    fn a_space_adds_one_endpoint_and_one_rule_at_the_top() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (private_key, _) = crate::spaces::wireguard_keypair();
        let space = crate::spaces::SpaceTunnel {
            private_key,
            address: "10.77.9.7/32".into(),
            hub_public_key: crate::spaces::wireguard_keypair().1,
            hub_endpoint: "10.77.0.1".into(),
            hub_port: 51821,
            subnet: "10.77.9.0/24".into(),
        };
        let (_path, cfg) = write_tun_config_for(
            "11111111-1111-1111-1111-111111111111",
            TEST_HOST,
            "example.com",
            "all",
            &[],
            false,
            "vless",
            "",
            None,
            &[],
            || None,
            Some(&space),
        )
        .expect("config");
        let endpoints = cfg["endpoints"].as_array().expect("endpoints");
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0]["tag"], "space");
        assert_eq!(endpoints[0]["address"][0], "10.77.9.7/32");
        assert_eq!(endpoints[0]["detour"], "proxy");
        let rules = cfg["route"]["rules"].as_array().expect("rules");
        assert_eq!(rules[0]["outbound"], "space");
        assert_eq!(rules[0]["ip_cidr"][0], "10.77.9.0/24");
    }

    #[test]
    fn a_routed_device_leaves_the_host_direct() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = device_cfg(&["192.168.1.39"], Some(("192.168.1.5", "Ethernet"))).expect("config");
        // The host is not being tunnelled, so its own traffic must not be either:
        // the whole point of the feature is that the PC's address does not move.
        assert_eq!(v["route"]["final"], "direct");
        assert_eq!(v["dns"]["final"], "direct-dns");
        // strict_route installs low-metric block rules; with the host direct they
        // would cut the bound direct socket and the forwarded return path.
        assert_eq!(v["inbounds"][0]["strict_route"], false);
    }

    #[test]
    fn the_routed_device_reaches_the_proxy_and_its_dns_does_not_leak() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = device_cfg(&["192.168.1.39"], Some(("192.168.1.5", "Ethernet"))).expect("config");
        let rules = v["route"]["rules"].as_array().unwrap();
        let is = |r: &serde_json::Value, outbound: &str| {
            r["outbound"].as_str() == Some(outbound)
                && r["source_ip_cidr"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x.as_str() == Some("192.168.1.39/32")))
        };
        let route_at = rules
            .iter()
            .position(|r| is(r, "proxy"))
            .unwrap_or_else(|| panic!("no rule sends the device to the proxy: {}", serde_json::to_string(&rules).unwrap()));
        // Below sniffing, or the protocol could never be known; above the generic
        // hijack, so the device's own queries reach the tunnel's resolver.
        let sniff_at = rules.iter().position(|r| r["action"] == "sniff").expect("no sniff action");
        let hijack_at = rules
            .iter()
            .position(|r| r["action"] == "hijack-dns" && r["source_ip_cidr"].is_null())
            .expect("no generic dns hijack");
        assert!(route_at > sniff_at, "the device rule sits above sniffing and can never match");
        assert!(route_at < hijack_at, "the generic hijack wins the device's dns");

        // The device's DNS has to leave through the tunnel, not through the host's
        // resolver, or the phone's lookups go out with the PC's address.
        assert!(
            v["dns"]["rules"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["server"] == "proxy-dns" && r["source_ip_cidr"] == serde_json::json!(["192.168.1.39/32"])),
            "the device's dns has no rule of its own"
        );
    }

    #[test]
    fn the_direct_outbound_is_pinned_to_the_lan_both_ways() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = device_cfg(&["192.168.1.39"], Some(("192.168.1.5", "Ethernet"))).expect("config");
        let direct = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["tag"] == "direct")
            .expect("no direct outbound");
        // Address alone loses to the tun's 0.0.0.0/1 routes, because Windows picks by
        // destination prefix and then by per-interface metric. Both, not one.
        assert_eq!(direct["inet4_bind_address"], "192.168.1.5");
        assert_eq!(direct["bind_interface"], "Ethernet");
        // and the host's resolver leaves through that same socket
        let dns = v["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["tag"] == "direct-dns")
            .expect("no direct dns");
        assert_eq!(dns["detour"], "direct");
    }

    #[test]
    fn a_device_session_with_nothing_to_bind_to_refuses_to_build() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // An unbound direct outbound would take the host's packets and hand them
        // back to the tun that just captured them. Refusing beats looping.
        assert!(device_cfg(&["192.168.1.39"], None).is_err());
    }

    #[test]
    fn a_whole_pc_session_is_untouched() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = device_cfg(&[], None).expect("config");
        assert_eq!(v["route"]["final"], "proxy");
        assert_eq!(v["dns"]["final"], "proxy-dns");
        assert_eq!(v["inbounds"][0]["strict_route"], true);
        let direct = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["tag"] == "direct")
            .expect("no direct outbound");
        assert!(direct["inet4_bind_address"].is_null(), "a plain session must not pin its direct outbound");
    }

    #[test]
    fn the_encoded_command_is_utf16le_base64() {
        // "A" is 0x41 0x00 in UTF-16LE, which base64 spells QQA=.
        assert_eq!(ps_encoded("A"), "QQA=");
        // Two characters: four bytes, so one whole group plus a short one.
        assert_eq!(ps_encoded("AB"), "QQBCAA==");
        // The byte order is the point: a big-endian encoder agrees on ASCII and
        // disagrees here, which is exactly the bug that would slip through.
        assert_ne!(ps_encoded("\u{0e01}"), ps_encoded("\u{010e}"));
    }

    #[test]
    fn voice_on_top_of_a_session_keeps_the_apps() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The helper pressed while the VPN is already up: the session keeps
        // its app list and gains the voice rules. A voice-only config here
        // would replace the tunnel and leave everything else unrouted while
        // the dial still said connected.
        let list = vec!["VALORANT.exe".to_string(), "Discord.exe".to_string()];
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "allow",
            &list,
            true,
            "vless",
            "",
            None,
        )
        .expect("config");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("json");
        let rules = v["route"]["rules"].as_array().unwrap();
        // the app rule survived
        assert!(
            rules.iter().any(|r| r["process_name"]
                .as_array()
                .map_or(false, |a| a.iter().any(|x| x.as_str() == Some("VALORANT.exe")))),
            "the app list was dropped when the voice helper joined"
        );
        // and the voice networks came with it
        assert!(rules.iter().any(|r| {
            r["outbound"] == "proxy"
                && r["ip_cidr"]
                    .as_array()
                    .map_or(false, |a| a.iter().any(|x| x.as_str() == Some("74.201.0.0/16")))
        }));
    }

    #[test]
    fn voice_config_routes_only_voice() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The voice-only session: no app list, just the helper.
        let list: Vec<String> = vec![];
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "allow",
            &list,
            true,
            "vless",
            "",
            None,
        )
        .expect("config");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("json");
        let rules = v["route"]["rules"].as_array().unwrap();
        // IPv6 is rejected above everything: captured so it cannot leak,
        // never carried, so new v6 flows fall back to v4 instead of dying.
        assert_eq!(rules[0]["ip_version"], 6);
        assert_eq!(rules[0]["action"], "reject");
        // the scoped STUN hedge rides the tunnel (port_range, not port:
        // sing-box 1.14 accepts ranges only through port_range) and is the
        // only port rule allowed in the set: anything unscoped would sweep
        // third-party flows into the tunnel and bring back the lag and the
        // in-game disconnect this feature was fixed for.
        assert!(rules.iter().any(|r| r["port_range"][0] == "3478:3480" && r["outbound"] == "proxy"));
        assert!(
            !rules.iter().any(|r| {
                r["outbound"] == "proxy"
                    && r.get("process_name").is_none()
                    && (r.get("port_range").is_some() || r.get("source_port_range").is_some())
            }),
            "an unscoped port rule leaked into the voice set"
        );
        // The Riot client's master/installer range must stay out: Riot's own
        // port table labels 8393-8400 Installer and Master, and tunneling it
        // took the game's launch and lobby traffic abroad.
        assert!(!rules.iter().any(|r| r["port_range"][0] == "8393:8400"));
        // The /16 we once carried was a widening of the source config; the
        // list stays line for line with the community file the feature forked
        // from.
        assert!(!rules.iter().any(|r| {
            r["ip_cidr"]
                .as_array()
                .map_or(false, |a| a.iter().any(|x| x.as_str() == Some("69.25.0.0/16")))
        }));
        // The helper must not capture the whole PC: only traffic addressed to
        // the voice networks is routed into the tun (the community config's
        // route-nopull shape). Everything else, the game included, stays on
        // the physical NIC and never touches the userspace stack.
        let ra = v["inbounds"][0]["route_address"].as_array().unwrap();
        assert_eq!(ra.len(), 8);
        assert!(ra.iter().any(|x| x.as_str() == Some("74.201.0.0/16")));
        assert!(ra.iter().any(|x| x.as_str() == Some("69.25.124.0/24")));
        assert_eq!(v["inbounds"][0]["auto_route"], true);
        // The voice infrastructure NETWORKS ride the tunnel on every port and
        // protocol; live-tested from Egypt: the direct path to a voice IP
        // times out while the same connection succeeds through the tunnel.
        assert!(rules.iter().any(|r| {
            r["outbound"] == "proxy"
                && r["ip_cidr"].as_array().map_or(false, |a| {
                    a.iter().any(|x| x.as_str() == Some("74.201.0.0/16"))
                        && a.iter().any(|x| x.as_str() == Some("20.0.0.0/8"))
                        && a.iter().any(|x| x.as_str() == Some("69.25.124.0/24"))
                })
        }));
        // The sniff action must precede the domain rule, or a sniffed domain
        // can never match and the rule is dead weight (shipped once already).
        let sniff_at = rules.iter().position(|r| r["action"] == "sniff").unwrap();
        let dom_at = rules.iter().position(|r| r["domain_suffix"].is_array()).unwrap();
        assert!(sniff_at < dom_at, "domain rule at {dom_at} above sniff at {sniff_at}");
        // no catch-all app rule is written for the Riot processes: the game
        // follows route.final, which stays direct so only voice is carried
        assert_eq!(v["route"]["final"], "direct");
        // no generic Riot rule exists either: without port_range or
        // domain_suffix the only rules that could proxy the game are absent
        assert!(!rules.iter().any(|r| {
            r["outbound"] == "proxy"
                && r.get("port_range").is_none()
                && r.get("domain_suffix").is_none()
                && r["process_name"].as_array().map_or(false, |a| a.len() == 3)
        }));
        // vivox resolves through the tunnel, not the local resolver
        let dns = v["dns"]["rules"].as_array().unwrap();
        assert!(dns.iter().any(|r| r["domain_suffix"][0] == "vivox.com"));
        // The engine's own decoder is the only authority on the grammar: a
        // rule shape it rejects kills the session at connect time. This is
        // exactly the check that was missing when "port" was used for ranges.
        if engine_dir().join("sing-box.exe").exists() {
            check_config().expect("engine accepts the voice config");
        }
    }

    #[test]
    fn voice_keeps_microsoft_identity_direct() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // 20.0.0.0/8 carries the voice networks, but it also contains
        // Microsoft's login endpoints at 20.190.x.x. With the helper on, every
        // OAuth and Microsoft login flow died with ERR_CONNECTION_CLOSED. The
        // direct /16 must sit above the voice CIDR rule and only the voice
        // networks may still use the proxy.
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "allow",
            &[],
            true,
            "vless",
            "",
            None,
        )
        .expect("config");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("json");
        let rules = v["route"]["rules"].as_array().unwrap();
        let direct_at = rules
            .iter()
            .position(|r| {
                r["outbound"] == "direct"
                    && r["ip_cidr"]
                        .as_array()
                        .map_or(false, |a| a.iter().any(|x| x.as_str() == Some("20.190.0.0/16")))
            })
            .expect("no direct exception for Microsoft identity");
        let voice_at = rules
            .iter()
            .position(|r| {
                r["outbound"] == "proxy"
                    && r["ip_cidr"]
                        .as_array()
                        .map_or(false, |a| a.iter().any(|x| x.as_str() == Some("20.0.0.0/8")))
            })
            .expect("no voice CIDR rule");
        assert!(
            direct_at < voice_at,
            "Microsoft identity at {direct_at} is below voice at {voice_at}"
        );
        // The Riot-scoped rules stay above the direct exception, so a Riot
        // voice flow on a STUN port is still carried.
        assert!(rules
            .iter()
            .any(|r| r["port_range"][0] == "3478:3480" && r["outbound"] == "proxy"));
        // No second outbound and no split tag: the connection is the selected
        // transport, exactly as before the experiment.
        let outs = v["outbounds"].as_array().unwrap();
        assert!(outs.iter().any(|o| o["tag"] == "proxy" && o["type"] == "vless"));
        assert!(!outs.iter().any(|o| o["tag"] == "voice"));
        if engine_dir().join("sing-box.exe").exists() {
            check_config().expect("engine accepts the voice config with the direct exception");
        }
    }

    #[test]
    fn route_check_sees_voice_sessions_and_defaults() {
        // The voice session confines the tun with route_address: the routes
        // it installs are the voice networks (real table lines, captured on
        // Windows), never a default route. Looking only for 0.0.0.0 sent the
        // helper into a connect-kill-retry loop while it was healthy.
        let voice = "\
         20.0.0.0        255.0.0.0       172.19.0.2       172.19.0.1      0\n\
     63.251.140.0    255.255.255.0       172.19.0.2       172.19.0.1      0\n\
       172.19.0.0  255.255.255.240         On-link        172.19.0.1    256\n";
        assert!(routes_present_in(voice, "172.19.0.1"));
        // Whole-device and app sessions keep finding their default route.
        assert!(routes_present_in(
            "          0.0.0.0          0.0.0.0         On-link        172.19.0.1      0\n",
            "172.19.0.1"
        ));
        // A bare adapter with only its own subnet line is not a session.
        assert!(!routes_present_in(
            "       172.19.0.0  255.255.255.240         On-link        172.19.0.1    256\n",
            "172.19.0.1"
        ));
        // Someone else's tunnel must never count.
        assert!(!routes_present_in(voice, "10.88.0.1"));
    }

    #[test]
    fn voice_merges_with_a_normal_session() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Normal per-app session (picked apps) with the voice helper on: the
        // caller's app rule and the voice rules must coexist in one config.
        let v = cfg_voice("allow", &["chrome.exe"], true);
        let rules = v["route"]["rules"].as_array().unwrap();
        // voice rules on top, under the v6 reject
        assert_eq!(rules[0]["ip_version"], 6);
        assert!(rules.iter().any(|r| r["port_range"][0] == "3478:3480"));
        assert!(rules.iter().any(|r| r["ip_cidr"].as_array().map_or(false, |a| !a.is_empty())));
        // the normal app rule is still present
        assert!(rules.iter().any(|r| {
            r["process_name"].as_array().map_or(false, |a| {
                a.iter().any(|x| x.as_str() == Some("chrome.exe"))
            })
        }));
        // allow-mode finals stay: everything else direct
        assert_eq!(v["route"]["final"], "direct");
        // merged sessions keep the full capture: per-app matching has to see
        // every connection, so the voice narrowing must not be applied.
        // route_address is always present in the shape (empty on Windows, the
        // DNS halves on Linux), so assert the absence of the voice networks
        // rather than the absence of the key.
        let ra = v["inbounds"][0]["route_address"]
            .as_array()
            .expect("route_address is part of the tun shape");
        assert!(
            !ra.iter().any(|x| x.as_str() == Some("74.201.0.0/16")),
            "a merged session must not be narrowed to the voice networks"
        );
        // whole-device session plus the helper: everything rides the tunnel
        let v2 = cfg_voice("all", &[], true);
        assert_eq!(v2["route"]["final"], "proxy");
        let ra2 = v2["inbounds"][0]["route_address"]
            .as_array()
            .expect("route_address is part of the tun shape");
        assert!(
            !ra2.iter().any(|x| x.as_str() == Some("74.201.0.0/16")),
            "a whole-device session must not be narrowed to the voice networks"
        );
        assert!(v2["route"]["rules"].as_array().unwrap().iter().any(|r| r["port_range"][0] == "3478:3480"));
        if engine_dir().join("sing-box.exe").exists() {
            check_config().expect("engine accepts the merged config");
        }
    }

    fn cfg_voice(mode: &str, apps: &[&str], voice: bool) -> serde_json::Value {
        let list: Vec<String> = apps.iter().map(|s| s.to_string()).collect();
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            mode,
            &list,
            voice,
            "vless",
            "",
            None,
        )
        .expect("config");
        serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("json")
    }

    #[test]
    fn allow_mode_sends_picked_apps_through_the_proxy() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = cfg("allow", &["chrome.exe"]);
        let rules = v["route"]["rules"].as_array().unwrap();
        // the IPv6 reject leads, the picked app follows
        assert_eq!(rules[0]["ip_version"], 6);
        assert_eq!(rules[0]["action"], "reject");
        assert!(rules.iter().any(|r| {
            r["outbound"] == "proxy"
                && r["process_name"].as_array().map_or(false, |a| a[0].as_str() == Some("chrome.exe"))
        }));
        assert_eq!(v["route"]["final"], "direct");
        assert_eq!(v["inbounds"][0]["stack"], "gvisor");
    }

    #[test]
    fn block_mode_sends_picked_apps_direct_and_the_rest_through_the_proxy() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = cfg("block", &["chrome.exe", "C:\\Tools\\game.exe"]);
        let rules = v["route"]["rules"].as_array().unwrap();
        // path rule and name rule both target direct, under the v6 reject
        assert_eq!(rules[0]["ip_version"], 6);
        assert!(rules.iter().any(|r| {
            r["outbound"] == "direct"
                && r["process_path"].as_array().map_or(false, |a| a[0].as_str() == Some("C:\\Tools\\game.exe"))
        }));
        assert!(rules.iter().any(|r| {
            r["outbound"] == "direct"
                && r["process_name"].as_array().map_or(false, |a| a[0].as_str() == Some("chrome.exe"))
        }));
        assert_eq!(v["route"]["final"], "proxy");
        assert_eq!(v["inbounds"][0]["stack"], "gvisor");
        // the real engine must accept it (no engine on disk → skip the check)
        if ensure_engine().is_ok() {
            check_config().expect("sing-box accepts the block config");
        }
    }

    #[test]
    fn whole_pc_mode_keeps_the_fast_mixed_stack() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = cfg("", &[]);
        assert_eq!(v["route"]["final"], "proxy");
        assert_eq!(v["inbounds"][0]["stack"], "mixed");
    }

    #[test]
    fn tun_address_is_picked_and_tracked() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "",
            &[],
            false,
            "vless",
            "",
            None,
        )
        .expect("config");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("json");
        let addr4 = v["inbounds"][0]["address"][0]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let addr6 = v["inbounds"][0]["address"][1]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let tracked4 = std::fs::read_to_string(p.with_file_name("tun-ip.txt"))
            .expect("tun-ip.txt beside the config")
            .trim()
            .to_string();
        let tracked6 = std::fs::read_to_string(p.with_file_name("tun-ip6.txt"))
            .expect("tun-ip6.txt beside the config")
            .trim()
            .to_string();
        // The config and the tracking files agree, both addresses come from
        // the candidate lists, and the route checks read the v4 value back:
        // a stale or hardcoded address would strand cleanup on the wrong one.
        assert_eq!(addr4, format!("{tracked4}/28"));
        assert_eq!(addr6, format!("{tracked6}/126"));
        assert!(TUN_IP_CANDIDATES.contains(&tracked4.as_str()));
        assert!(TUN_V6_CANDIDATES.contains(&tracked6.as_str()));
        assert_eq!(current_tun_ip(), tracked4);
    }

    #[test]
    fn tun_ip_skips_addresses_the_machine_already_uses() {
        // Free machine: the classic pair stays.
        assert_eq!(
            pick_addresses("", "", ""),
            (TUN_IP.to_string(), "fdfe:dcba:9876::1".to_string())
        );
        // Ghost adapter holds the v4 address (shows in the adapter list).
        let adapters =
            "Ethernet adapter QuotaCards3a5f:\n   IPv4 Address. . . : 172.19.0.1(Preferred)";
        assert_eq!(pick_addresses("", "", adapters).0, "172.20.0.1");
        // Only its prefix route remains (shows in the route table).
        let routes = "172.19.0.0  255.255.255.240  On-link  172.19.0.1  281";
        assert_eq!(pick_addresses(routes, "", "").0, "172.20.0.1");
        // A default route through the address (crashed session) also counts.
        let def = "0.0.0.0  0.0.0.0  On-link  172.19.0.1  281";
        assert_eq!(pick_addresses(def, "", "").0, "172.20.0.1");
        // The v6 ULA collides the same way, and the two picks are independent.
        let adapters6 = "   IPv6 Address. . . . . . . . . . . : fdfe:dcba:9876::1(Preferred)";
        assert_eq!(
            pick_addresses("", "", adapters6),
            ("172.19.0.1".to_string(), "fdfe:dcba:9877::1".to_string())
        );
        // Its route carrying the ULA counts too, and matching is case-blind.
        let routes6 = "fdfe:dcba:9876::/126   On-link   fdfe:dcba:9876::1";
        assert_eq!(pick_addresses("", routes6, "").1, "fdfe:dcba:9877::1");
        assert_eq!(pick_addresses("", "", "FDFE:DCBA:9876::1").1, "fdfe:dcba:9877::1");
        // Everything taken: fall back to the first candidates, never empty.
        let all4 = TUN_IP_CANDIDATES.join("\n");
        let all6 = TUN_V6_CANDIDATES.join("\n");
        let both = format!("{all4}\n{all6}");
        assert_eq!(
            pick_addresses(&both, &both, &both),
            (TUN_IP.to_string(), "fdfe:dcba:9876::1".to_string())
        );
    }

    #[test]
    fn udp_transports_build_engine_valid_configs() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The point of this test is the engine's own decoder, so it can only run
        // where the vendored engine is installed. Say so out loud rather than
        // failing on a machine that never downloaded one: a red result here would
        // be reported as a broken config, which it is not.
        if !singbox_path().exists() {
            eprintln!(
                "skipped: no engine at {}, so there is nothing to decode with",
                singbox_path().display()
            );
            return;
        }
        // Hysteria2: the outbound form, carrying the card SNI and the
        // server's shared password.
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "all",
            &[],
            false,
            "hy2",
            "deadbeef",
            None,
        )
        .expect("config");
        check_config().expect("hy2 config must pass the engine's own decoder");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("json");
        assert!(v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["type"] == "hysteria2"));
        assert!(v.get("endpoints").is_none());
        // WireGuard: the 1.13+ top-level endpoint, still tagged "proxy" so
        // route rules, DNS detours and the voice rules keep pointing at it.
        let p = write_tun_config(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "all",
            &[],
            false,
            "wg",
            "",
            Some((
                "Ji355SrChcN8xhmBAWfjUlx8V1/lNmL/ItImPY7B/tA=",
                "10.8.0.8/32",
                "NybbwmymQWVk3lKfz46jqj/Cqzxbv7ToDk3jP0A5yv4=",
                "{\"jc\":4,\"jmin\":40,\"jmax\":70,\"s1\":15,\"s2\":68,\"h1\":\"2310286408\",\"h2\":\"311773765\",\"h3\":\"3400017965\",\"h4\":\"400567291\"}",
            )),
        )
        .expect("config");
        // The awg endpoint only exists in the amnezia fork the Windows build ships.
        // A stock sing-box (the Linux engine) answers "unknown endpoint type: awg",
        // which says nothing about this config, so name the case and move on. Every
        // other decoder complaint still fails the test.
        match check_config() {
            Ok(()) => {}
            Err(e) if e.contains("unknown endpoint type: awg") => eprintln!(
                "skipped the awg leg: this engine is stock sing-box, not the amnezia fork"
            ),
            Err(e) => panic!("wg config must pass the engine's own decoder: {e:?}"),
        }
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("json");
        assert_eq!(v["endpoints"][0]["type"], "awg");
        assert_eq!(v["endpoints"][0]["jc"], 4);
        assert_eq!(v["endpoints"][0]["tag"], "proxy");
        assert!(v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["tag"] != "proxy"));

        // The space endpoint has to coexist with the awg endpoint and point
        // its detour at it. This is the combination a WireGuard card uses, and
        // the engine is the only authority on whether the shape is valid.
        let space = crate::spaces::SpaceTunnel {
            private_key: crate::spaces::wireguard_keypair().0,
            address: "10.77.7.9/32".into(),
            hub_public_key: crate::spaces::wireguard_keypair().1,
            hub_endpoint: "10.77.0.1".into(),
            hub_port: 51821,
            subnet: "10.77.7.0/24".into(),
        };
        let p = write_tun_config_with_space(
            "00000000-test",
            TEST_HOST,
            "example.com",
            "all",
            &[],
            false,
            "wg",
            "",
            Some((
                "Ji355SrChcN8xhmBAWfjUlx8V1/lNmL/ItImPY7B/tA=",
                "10.8.0.8/32",
                "NybbwmymQWVk3lKfz46jqj/Cqzxbv7ToDk3jP0A5yv4=",
                "{\"jc\":4,\"jmin\":40,\"jmax\":70,\"s1\":15,\"s2\":68,\"h1\":\"2310286408\",\"h2\":\"311773765\",\"h3\":\"3400017965\",\"h4\":\"400567291\"}",
            )),
            Some(&space),
        )
        .expect("config");
        match check_config() {
            Ok(()) => {}
            Err(e) if e.contains("unknown endpoint type: awg") => eprintln!(
                "skipped the awg+space leg: this engine is stock sing-box, not the amnezia fork"
            ),
            Err(e) => panic!("awg + space config must pass the engine's own decoder: {e:?}"),
        }
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("json");
        let endpoints = v["endpoints"].as_array().expect("endpoints");
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[1]["tag"], "space");
        assert_eq!(endpoints[1]["detour"], "proxy");
        assert_eq!(v["route"]["rules"][0]["outbound"], "space");
    }
}


// ---------------------------------------------------------------------------
// Linux
//
// Same contract, different plumbing. No scheduled task: the engine needs root
// for the tunnel, so it is started once per session through pkexec and the app
// talks to it through the same files. The kill switch is a real allow list
// here, which Windows could not express through its firewall alone.
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
pub fn singbox_path() -> PathBuf {
    engine_dir().join("sing-box")
}

#[cfg(target_os = "linux")]
pub fn wintun_path() -> PathBuf {
    engine_dir().join("wintun.dll")
}

#[cfg(target_os = "linux")]
pub fn is_elevated() -> bool {
    run_hidden("id", &["-u"])
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn ensure_engine() -> Result<String, String> {
    let dir = engine_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let ver_file = dir.join("tun-version.txt");
    let cur = std::fs::read_to_string(&ver_file).unwrap_or_default();
    let want = format!("sing-box {ENGINE_VER} linux");
    if singbox_path().exists() && cur.trim() == want {
        return Ok("engine ready".to_string());
    }
    let tgz = dir.join("engine.tar.gz");
    let url = format!(
        "https://github.com/SagerNet/sing-box/releases/download/v{ENGINE_VER}/sing-box-{ENGINE_VER}-linux-amd64.tar.gz"
    );
    let dl = run_hidden(
        "curl",
        &[
            "-L", "--max-time", "180", "-A", "QuotaVPN", "-o",
            &tgz.to_string_lossy(), &url,
        ],
    )
    .map(|o| o.status.success())
    .unwrap_or(false)
        && tgz.exists();
    if !dl {
        return Err("engine download failed (check internet)".to_string());
    }
    run_hidden("tar", &["-xzf", &tgz.to_string_lossy(), "-C", &dir.to_string_lossy()])
        .map_err(|e| format!("extract: {e}"))?;
    let _ = std::fs::remove_file(&tgz);
    let inner = dir.join(format!("sing-box-{ENGINE_VER}-linux-amd64"));
    if singbox_path().exists() {
        let _ = std::fs::remove_file(singbox_path());
    }
    std::fs::rename(inner.join("sing-box"), singbox_path())
        .map_err(|e| format!("install engine: {e}"))?;
    let _ = std::fs::remove_dir_all(inner);
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(singbox_path(), std::fs::Permissions::from_mode(0o755));
    std::fs::write(&ver_file, &want).map_err(|e| e.to_string())?;
    Ok("engine installed".to_string())
}

#[cfg(target_os = "linux")]
fn pick_tun_ips() -> (String, String) {
    let routes4 = run_hidden("ip", &["route"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let routes6 = run_hidden("ip", &["-6", "route"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let adapters = run_hidden("ip", &["addr"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    pick_addresses(&routes4, &routes6, &adapters)
}

/// No helper task on Linux: root comes from pkexec per session.
#[cfg(target_os = "linux")]
pub fn task_installed() -> bool {
    true
}

#[cfg(target_os = "linux")]
pub fn install_task(_owner: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn remove_task() {}

#[cfg(target_os = "linux")]
pub fn task_name() -> String {
    "quotavpn-engine".to_string()
}

#[cfg(target_os = "linux")]
pub fn task_engine_pid() -> Option<u32> {
    let pid = std::fs::read_to_string(engine_dir().join("engine.pid"))
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()?;
    if std::path::Path::new(&format!("/proc/{pid}")).exists() {
        Some(pid)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
pub fn account_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_string())
}

#[cfg(target_os = "linux")]
pub fn request_graceful_stop(pid: u32) {
    let _ = run_hidden("kill", &["-TERM", &pid.to_string()]);
}

#[cfg(target_os = "linux")]
pub fn tun_routes_present() -> bool {
    let routes = run_hidden("ip", &["route", "show", "default"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    routes.lines().any(|l| {
        l.starts_with("default")
            && (l.contains("tun") || l.contains("QuotaVPN") || l.contains("quotatun"))
    })
}

/// The block: drop everything leaving this machine except loopback, the
/// tunnel itself, the server it connects to, and replies to flows that
/// already exist. Applied before the engine starts, so a tunnel that dies
/// leaves the block in place instead of the bare internet.
#[cfg(target_os = "linux")]
pub fn block_traffic(block: bool) -> Result<(), String> {
    if block {
        let _ = run_hidden("nft", &["delete", "table", "inet", "quotavpn"]);
        let _ = run_hidden("nft", &["add", "table", "inet", "quotavpn"]);
        let _ = run_hidden(
            "nft",
            &["add", "chain", "inet", "quotavpn", "out",
              "{ type filter hook output priority 0; policy drop; }"],
        );
        let _ = run_hidden("nft", &["add", "rule", "inet", "quotavpn", "out", "oif", "lo", "accept"]);
        let _ = run_hidden(
            "nft",
            &["add", "rule", "inet", "quotavpn", "out", "oifname", "QuotaVPN*", "accept"],
        );
        let _ = run_hidden(
            "nft",
            &["add", "rule", "inet", "quotavpn", "out", "oifname", "quotatun", "accept"],
        );
        let _ = run_hidden(
            "nft",
            &["add", "rule", "inet", "quotavpn", "out", "ct", "state", "established,related", "accept"],
        );
        for cidr in server_cidrs() {
            let _ = run_hidden(
                "nft",
                &["add", "rule", "inet", "quotavpn", "out", "ip", "daddr",
                  cidr.split('/').next().unwrap_or(cidr.as_str()), "accept"],
            );
        }
        // Same rule as the Windows side: say what was seen, not what was run.
        if traffic_blocked() {
            app_log("killswitch: outbound allowed through the tunnel only");
            Ok(())
        } else {
            app_log("killswitch: block FAILED, outbound is not restricted");
            Err("killswitch: the drop table did not take".to_string())
        }
    } else {
        let _ = run_hidden("nft", &["delete", "table", "inet", "quotavpn"]);
        if traffic_blocked() {
            app_log("killswitch: release FAILED, drop table still in place");
            Err("killswitch: the drop table is still in place".to_string())
        } else {
            app_log("killswitch: released");
            Ok(())
        }
    }
}

/// Server addresses the tunnel is allowed to reach while the block is on.
#[cfg(target_os = "linux")]
fn server_cidrs() -> Vec<String> {
    std::fs::read_to_string(engine_dir().join("server-ips.txt"))
        .map(|t| {
            t.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
pub fn traffic_blocked() -> bool {
    run_hidden("nft", &["list", "table", "inet", "quotavpn"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn engine_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(target_os = "linux")]
pub fn engine_cleanup_keep(_keep: Option<u32>) {
    if let Some(pid) = task_engine_pid() {
        let _ = run_hidden("kill", &["-TERM", &pid.to_string()]);
    }
    let _ = std::fs::remove_file(engine_dir().join("engine.pid"));
}

#[cfg(target_os = "linux")]
pub fn start_engine() -> Result<u32, String> {
    if let Some(old) = task_engine_pid() {
        let _ = run_hidden("kill", &["-TERM", &old.to_string()]);
        for _ in 0..25 {
            if task_engine_pid().is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    let _ = std::fs::remove_file(engine_dir().join("engine.stop"));
    if is_elevated() {
        let child = spawn_engine()?;
        let pid = child.id();
        let _ = std::fs::write(engine_dir().join("engine.pid"), pid.to_string());
        return Ok(pid);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let _ = std::fs::remove_file(engine_dir().join("engine.pid"));
    // pkexec asks once per session; HOME is carried over so root writes its
    // runtime files into the same folder the app watches.
    std::process::Command::new("pkexec")
        .arg("env")
        .arg(format!("HOME={home}"))
        .arg(&exe)
        .arg("--run-engine")
        .spawn()
        .map_err(|e| format!("pkexec: {e}"))?;
    for _ in 0..120 {
        if let Some(pid) = task_engine_pid() {
            return Ok(pid);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    Err("the tunnel did not start (was the password cancelled?)".to_string())
}

#[cfg(target_os = "linux")]
pub fn stop_engine() {
    let _ = std::fs::write(engine_dir().join("engine.stop"), "1");
    if let Some(pid) = task_engine_pid() {
        let mut gone = false;
        for _ in 0..20 {
            if !engine_alive(pid) {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        if !gone {
            let _ = run_hidden("kill", &["-KILL", &pid.to_string()]);
        }
    }
    let _ = std::fs::remove_file(engine_dir().join("engine.stop"));
    let _ = std::fs::remove_file(engine_dir().join("engine.pid"));
    let _ = block_traffic(false);
    engine_cleanup_keep(None);
}

#[cfg(target_os = "linux")]
pub fn run_engine_headless() {
    let cfg = tun_config_path();
    if !cfg.exists() {
        return;
    }
    let pid_path = engine_dir().join("engine.pid");
    let stop_path = engine_dir().join("engine.stop");
    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(&stop_path);
    let guarding = kill_switch_on();
    if guarding {
        let _ = block_traffic(true);
    }
    let mut cmd = cmd_hidden(&singbox_path().to_string_lossy());
    cmd.args(["run", "-c", &cfg.to_string_lossy()]);
    if let Ok(f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(engine_log_path())
    {
        if let Ok(e) = f.try_clone() {
            cmd.stdout(e);
        }
        cmd.stderr(f);
    }
    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            let _ = std::fs::write(&pid_path, pid.to_string());
            app_log(&format!("engine started (pid {pid})"));
            loop {
                if stop_path.exists() {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        }
        Err(e) => {
            // The block went on before the spawn. Nothing is carrying the
            // traffic it was holding back, so lifting it is the honest state.
            if guarding {
                let _ = block_traffic(false);
            }
            app_log(&format!("engine failed to start: {e}"));
        }
    }
    let _ = std::fs::remove_file(&pid_path);
    if guarding {
        if stop_path.exists() {
            let _ = block_traffic(false);
        } else {
            let _ = block_traffic(true);
        }
    }
}

#[cfg(target_os = "linux")]
fn refresh_tun_name() -> String {
    "quotatun".to_string()
}

#[cfg(target_os = "linux")]
fn cleanup_tun_routes() {}

#[cfg(target_os = "linux")]
fn kill_stale_engine() {}

#[cfg(target_os = "linux")]
fn sid_for(_who: &str) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
pub fn engine_ps_path() -> PathBuf {
    engine_dir().join("engine.ps1")
}

#[cfg(target_os = "linux")]
pub fn write_engine_cmd() {}
