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
    base.join("quotacards").join("engine")
}

pub fn singbox_path() -> PathBuf {
    engine_dir().join("sing-box.exe")
}

pub fn wintun_path() -> PathBuf {
    engine_dir().join("wintun.dll")
}

pub fn tun_config_path() -> PathBuf {
    engine_dir().join("singbox-tun.json")
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
pub fn is_elevated() -> bool {
    run_hidden("net", &["session"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Download + extract the sing-box engine (pinned) and wintun driver.
/// Version-stamped so a stale engine is replaced automatically.
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

/// Resolve the server domain NOW (system DNS intact) so its IPs can be
/// excluded from TUN routing - otherwise the tunnel eats its own path.
pub fn resolve_server_ips(host: &str) -> Result<Vec<String>, String> {
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
    let mut excludes = resolve_server_ips(host)?;
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
    let dns_final = if apps_only { "direct-dns" } else { "proxy-dns" };
    let route_final = if apps_only { "direct" } else { "proxy" };
    let mut route_rules = serde_json::json!([
        {"action": "sniff"},
        {"network": "udp", "port": [135, 137, 138, 139, 5353], "action": "reject"},
        {"ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject"},
        {"source_ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject"},
        {"protocol": "dns", "action": "hijack-dns"}
    ]);
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
    let dns_rules = if voice {
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
    let (outbounds, endpoints) = match transport {
        "hy2" => (
            serde_json::json!([
                {
                    "type": "hysteria2",
                    "tag": "proxy",
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
                },
                {"type": "direct", "tag": "direct"}
            ]),
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
                serde_json::json!([{"type": "direct", "tag": "direct"}]),
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
                {"type": "direct", "tag": "direct"}
            ]),
            serde_json::json!([]),
        ),
    };
    let mut cfg = serde_json::json!({
        "log": {"level": "warning"},
        "dns": {
            "servers": [
                {"type": "https", "tag": "proxy-dns", "server": PROXY_DOH_IP, "detour": "proxy"},
                {"type": "udp", "tag": "direct-dns", "server": DIRECT_DNS}
            ],
            "rules": dns_rules,
            "final": dns_final,
            "strategy": "ipv4_only"
        },
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "interface_name": if_name,
            "mtu": 9000,
            "address": [format!("{tun_ip}/28"), format!("{tun_v6}/126")],
            "auto_route": true,
            "strict_route": true,
            "stack": tun_stack,
            "route_exclude_address": excludes
        }],
        "outbounds": outbounds,
        "route": {
            "rules": route_rules,
            "final": route_final,
            "auto_detect_interface": true,
            "default_domain_resolver": "direct-dns"
        }
    });
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
        cfg["inbounds"][0]["route_address"] = serde_json::json!(VOICE_CIDRS.to_vec());
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
    Ok(p)
}

/// Validate the config before routing a single packet.
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
pub fn task_name() -> String {
    format!("QuotaVPN Tunnel {}", account_name())
}

/// This process's Windows account, asked from Windows rather than inherited
/// from the environment: a process started through runas or
/// CreateProcessWithLogonW carries the launcher's USERNAME, which made a
/// second user believe it was already set up.
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
pub fn task_installed() -> bool {
    run_hidden("schtasks", &["/Query", "/TN", &task_name()])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The script the helper task runs. It exists so the privileged side can clear
/// what a crashed run leaves behind *before* the engine starts: every dead
/// session leaves a phantom "sing-tun Tunnel" device, Windows keeps the adapter
/// name reserved, and the next start dies with
/// "create adapter: Cannot create a file when that file already exists".
/// Clearing phantoms needs administrator rights, which the task has.
pub fn engine_ps_path() -> PathBuf {
    engine_dir().join("run-engine.ps1")
}

/// The script the helper task runs. It exists so the privileged side can do
/// what the app may not: clear the phantom wintun devices a dead session
/// leaves behind (Windows keeps the adapter name reserved, and the next start
/// died with "create adapter: Cannot create a file when that file already
/// exists" until the machine was restarted), and report the engine pid, which
/// an ordinary user cannot read from a SYSTEM process.
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

/// Register the task. Runs in the elevated pass (`QuotaVPN.exe --setup-helper`),
/// so this is the only moment the product asks Windows for administrator.
pub fn install_task(owner: &str) -> Result<(), String> {
    write_engine_cmd();
    let tr = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"",
        engine_ps_path().display()
    );
    let out = run_hidden(
        "schtasks",
        &[
            "/Create", "/TN", &format!("QuotaVPN Tunnel {owner}"), "/TR", &tr, "/SC", "ONCE",
            "/ST", "00:00", "/RU", owner, "/RL", "HIGHEST", "/F",
        ],
    )
    .map_err(|e| format!("task: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn remove_task() {
    let deleted = run_hidden("schtasks", &["/Delete", "/TN", &task_name(), "/F"])
        .map(|o| o.status.success())
        .unwrap_or(false);
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
         taskkill /IM sing-box.exe /F | Out-Null
         Get-NetAdapter -IncludeHidden | Where-Object {{ $_.Name -like 'QuotaVPN*' }} | ForEach-Object {{
           Disable-NetAdapter -InputObject $_ -Confirm:$false
           Remove-NetAdapter -InputObject $_ -Confirm:$false
         }}
         Get-PnpDevice -Class Net | Where-Object {{ $_.FriendlyName -eq 'sing-tun Tunnel' }} | ForEach-Object {{
           pnputil /remove-device $_.InstanceId | Out-Null
         }}
",
        task_name()
    );
    if std::fs::write(&script, body).is_ok() {
        // Start-Process joins an argument array with spaces and quotes
        // nothing, so the whole switch list goes as one string.
        let ps = format!(
            "Start-Process -FilePath 'powershell.exe' -ArgumentList '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"' -Verb RunAs -Wait -WindowStyle Hidden",
            script.display()
        );
        let _ = run_hidden("powershell", &["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]);
        let _ = std::fs::remove_file(&script);
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
}

fn start_task() -> Result<(), String> {
    let out = run_hidden("schtasks", &["/Run", "/TN", &task_name()]).map_err(|e| format!("task: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn stop_task() {
    let _ = run_hidden("schtasks", &["/End", "/TN", &task_name()]);
}

/// The pid of the engine our task started: the sing-box whose command line
/// names our own config path. Anything else called sing-box is not ours.
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
pub fn engine_alive(pid: u32) -> bool {
    run_hidden("tasklist", &["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("sing-box.exe"))
        .unwrap_or(false)
}

/// Bring the engine up through the task and return its pid (0 while the task
/// is still starting; the caller waits on the routes anyway).
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
    write_engine_cmd();
    stop_task();
    start_task()?;
    for _ in 0..80 {
        if let Some(pid) = task_engine_pid() {
            let _ = std::fs::write(engine_dir().join("engine.pid"), pid.to_string());
            return Ok(pid);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    Ok(0)
}

/// Stop whatever the task is running, then clean routes and adapter.
pub fn stop_engine() {
    // An ordinary user may end the task but cannot kill the SYSTEM process it
    // started, so the script watches for this file and stops the engine
    // itself. Ending the task is the backstop.
    let stop = engine_dir().join("engine.stop");
    let _ = std::fs::write(&stop, "1");
    if let Some(pid) = task_engine_pid() {
        for _ in 0..30 {
            if !engine_alive(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
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
        let out = run_hidden(
            "powershell",
            &[
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "@(Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'QuotaVPN*' }).Count",
            ],
        );
        if let Ok(o) = out {
            let n = String::from_utf8_lossy(&o.stdout);
            return n.trim() != "0" && !n.trim().is_empty();
        }
        false
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
        (l.contains("quotacards") || l.contains("quotavpn"))
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

#[cfg(test)]
mod tests {
    use super::*;

    // The three tests rewrite one engine config file; serialize them.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn cfg(mode: &str, apps: &[&str]) -> serde_json::Value {
        let list: Vec<String> = apps.iter().map(|s| s.to_string()).collect();
        let p = write_tun_config(
            "00000000-test",
            crate::config::DEFAULT_HOST,
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

    #[test]
    fn voice_config_routes_only_voice() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The voice-only session: no app list, just the helper.
        let list: Vec<String> = vec![];
        let p = write_tun_config(
            "00000000-test",
            crate::config::DEFAULT_HOST,
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
        // every connection, so no route_address narrowing here
        assert!(v["inbounds"][0].get("route_address").is_none());
        // whole-device session plus the helper: everything rides the tunnel
        let v2 = cfg_voice("all", &[], true);
        assert_eq!(v2["route"]["final"], "proxy");
        assert!(v2["inbounds"][0].get("route_address").is_none());
        assert!(v2["route"]["rules"].as_array().unwrap().iter().any(|r| r["port_range"][0] == "3478:3480"));
        if engine_dir().join("sing-box.exe").exists() {
            check_config().expect("engine accepts the merged config");
        }
    }

    fn cfg_voice(mode: &str, apps: &[&str], voice: bool) -> serde_json::Value {
        let list: Vec<String> = apps.iter().map(|s| s.to_string()).collect();
        let p = write_tun_config(
            "00000000-test",
            crate::config::DEFAULT_HOST,
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
            crate::config::DEFAULT_HOST,
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
        // Hysteria2: the outbound form, carrying the card SNI and the
        // server's shared password.
        let p = write_tun_config(
            "00000000-test",
            crate::config::DEFAULT_HOST,
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
            crate::config::DEFAULT_HOST,
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
        check_config().expect("wg config must pass the engine's own decoder");
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
    }
}
