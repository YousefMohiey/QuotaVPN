//! Who is on this network.
//!
//! A home router hides its client list behind an admin login, so the honest way
//! to answer "who is on my wifi" from a normal PC is the one every network tool
//! uses: wake the subnet with a ping sweep, then read the neighbour table the
//! machine already keeps. No driver, no administrator, no packet capture: the
//! listing has to work before the app is allowed to ask for anything heavier.

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

/// One device seen on the local network.
#[cfg(not(target_os = "windows"))]
use crate::wincompat::CmdExt;
#[derive(Clone, serde::Serialize)]
pub struct Device {
    pub ip: String,
    pub mac: String,
    pub name: String,
    /// True for the machine the app is running on, so its row can be labelled
    /// instead of leaving the user to work it out.
    pub self_pc: bool,
    /// Where the name came from, ranked by [name_rank]. The strongest source
    /// wins whatever order the passes run in, so a later lookup can fill a gap
    /// but can never replace a name a better source already gave. Serialised
    /// as "source" so the shape the screen reads does not move.
    #[serde(rename = "source")]
    pub name_source: String,
    /// Protected devices are never capped or blocked: the router, this PC,
    /// and anything the user marked.
    #[serde(default)]
    pub protected: bool,
    /// True when protection is not the user's to remove: this PC and the
    /// router stay safe whatever the screen says.
    #[serde(default)]
    pub protected_always: bool,
    /// True when the last completed sweep proved this device is here: it
    /// answered, or the neighbour table resolved its hardware address during
    /// the burst. Presence is not a lease and it is not permanent: a row that
    /// is kept past one miss comes back with this false and stays on screen
    /// quiet, so a sleepy phone does not blink out and a device that really
    /// left is not kept forever. The screen decides how a quiet row looks; it
    /// is never deleted to say "gone".
    #[serde(default)]
    pub live: bool,
}

/// How much a name is worth, by where it came from. Higher wins, and an equal
/// rank never overwrites: a later pass can fill a gap but cannot undo an
/// equally good answer, so the order the passes happen to run in stops
/// deciding what the row is called.
///
/// admin (a name somebody set on purpose) beats the hostname a device puts in
/// its DHCP request, which beats what the device answers on the wire (mDNS,
/// then NetBIOS). The router's own lease table is an indirect copy of those
/// answers, so it ranks below them: it names the devices that answer nothing,
/// and never argues with one that named itself. A hardware fingerprint is the
/// last resort before an address with no name at all.
pub fn name_rank(source: &str) -> u8 {
    match source {
        "self" => 100,
        "admin" => 60,
        "dhcp" => 50,
        "mdns" => 40,
        "netbios" => 30,
        "router" => 25,
        "ping" => 20,
        // Written before the source was recorded: the old code stamped every
        // resolved name "device", so it is real but unattributable, and it
        // ranks below every source this build can actually name.
        "device" => 15,
        "vendor" => 10,
        _ => 0,
    }
}

/// Take a name only when its source outranks the one already on the row.
pub fn offer_name(d: &mut Device, name: &str, source: &str) {
    let name = name.trim();
    if name.is_empty() || name == "-" {
        return;
    }
    if name_rank(source) > name_rank(&d.name_source) {
        d.name = name.to_string();
        d.name_source = source.to_string();
    }
}

#[cfg(windows)]
fn hidden(prog: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    let mut c = Command::new(prog);
    c.creation_flags(NO_WINDOW);
    c
}

/// The address this machine uses to reach the internet, which is the one whose
/// subnet we care about.
fn local_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

/// Prefix length for the interface that owns `ip`, read from the system config.
/// Home networks are nearly always /24, so that is the fallback.
#[cfg(windows)]
fn prefix_for(ip: Ipv4Addr) -> u8 {
    let out = match hidden("ipconfig").output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return 24,
    };
    let target = ip.to_string();
    let mut current: Option<String> = None;
    for raw in out.lines() {
        let line = raw.trim();
        if line.to_lowercase().contains("ipv4") {
            if let Some(v) = line.split(':').nth(1) {
                current = Some(v.trim().to_string());
            }
        }
        if line.to_lowercase().contains("subnet mask") || line.contains("قناع") {
            if let (Some(cur), Some(mask)) = (current.as_ref(), line.split(':').nth(1)) {
                if *cur == target {
                    let m = mask.trim();
                    if let Ok(octets) = m
                        .split('.')
                        .map(|p| p.parse::<u8>())
                        .collect::<Result<Vec<_>, _>>()
                    {
                        if octets.len() == 4 {
                            let bits: u32 = octets
                                .iter()
                                .fold(0u32, |acc, o| (acc << 8) | (*o as u32));
                            return bits.leading_ones() as u8;
                        }
                    }
                }
            }
        }
    }
    24
}

#[cfg(not(windows))]
fn prefix_for(_ip: Ipv4Addr) -> u8 {
    24
}

/// The neighbour table: ip to mac, as the machine sees it right now.
#[cfg(windows)]
fn arp_table() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let out = match hidden("arp").arg("-a").output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return map,
    };
    for raw in out.lines() {
        let line = raw.trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let ip = parts[0];
        let mac = parts[1].replace('-', ":");
        let looks_ip = ip.split('.').count() == 4
            && ip.split('.').all(|p| p.parse::<u8>().is_ok());
        let looks_mac = mac.split(':').count() == 6;
        if looks_ip && looks_mac {
            map.insert(ip.to_string(), mac.to_lowercase());
        }
    }
    map
}

#[cfg(not(windows))]
fn arp_table() -> HashMap<String, String> {
    HashMap::new()
}

/// Ask one address to answer, and take its name from the reply when the system
/// resolves one. Windows prints "Pinging NAME [ip] with 32 bytes" when it knows
/// the name, and "Pinging ip with 32 bytes" when it does not.
#[cfg(windows)]
fn wake(ip: String) -> (String, bool, String) {
    let out = match hidden("ping").args(["-n", "1", "-w", "180", &ip]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return (ip, false, String::new()),
    };
    let alive = out.contains("TTL=") || out.contains("TTL =");
    // The reply's time-to-live is a free hint at the operating system: Windows
    // starts at 128, phones and Unix at 64.
    let ttl = out
        .split("TTL=")
        .nth(1)
        .or_else(|| out.split("TTL =").nth(1))
        .and_then(|rest| rest.trim().split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    (ip, alive, format!("{ttl}"))
}

#[cfg(not(windows))]
fn wake(ip: String) -> (String, bool, String) {
    (ip, false, String::new())
}

/// Devices the neighbour table already knows, with no sweep at all. Instant,
/// and enough to draw the list before the scan has finished.
/// The last scan, kept on disk so the page opens with the devices it already
/// knows instead of an empty table that fills in a few seconds later.
/// Names the router gave us, kept by hardware address so they survive a
/// restart. A router name belongs to the machine that owns that address, so
/// this is the one cache that cannot drift onto the wrong device.
fn router_names_path() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    base.join("quotavpn").join("lan_router_names.txt")
}

/// Every name the router has ever given, keyed by lower case address.
pub fn router_names() -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(router_names_path()) else {
        return out;
    };
    for line in text.lines() {
        if let Some((mac, name)) = line.split_once('|') {
            let mac = mac.trim().to_ascii_lowercase().replace('-', ":");
            let name = name.trim();
            if !mac.is_empty() && !name.is_empty() {
                out.insert(mac, name.to_string());
            }
        }
    }
    out
}

/// Keep what the router just told us, and keep what it told us before.
pub fn save_router_names(pairs: Vec<(String, String)>) {
    let mut all = router_names();
    for (mac, name) in pairs {
        let mac = mac.trim().to_ascii_lowercase().replace('-', ":");
        let name = name.trim().to_string();
        if !mac.is_empty() && !name.is_empty() {
            all.insert(mac, name);
        }
    }
    let path = router_names_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut lines: Vec<String> = all.into_iter().map(|(m, n)| format!("{m}|{n}")).collect();
    lines.sort();
    let _ = std::fs::write(path, lines.join("
"));
}

/// The hardware address the neighbour table currently has for an address.
///
/// Used to hand a device back to its real router: a restore frame has to name the
/// router's MAC, and a guessed one would be worse than refusing.
pub fn mac_for(ip: &str) -> Option<String> {
    arp_table().get(ip).cloned().filter(|m| !m.is_empty())
}

pub fn known() -> Vec<Device> {
    // What the last scan saw, with anything the table reports right now folded
    // in. The page paints this at once and the live scan corrects it.
    // No disk cache: the page paints what the neighbour and router tables say
    // right now, which is both instant and current. Painting the last run's
    // list is what put last night's names against this morning's addresses.
    let mut out: Vec<Device> = Vec::new();
    for (ip, mac) in arp_table() {
        if !keep(&ip, &mac) {
            continue;
        }
        match out.iter_mut().find(|d| d.ip == ip) {
            Some(d) => {
                d.mac = mac;
                // The neighbour table produced its hardware address, which
                // means this machine resolved it without being asked to: the
                // same evidence a sweep counts as presence.
                d.live = true;
            }
            None => out.push(Device {
                ip,
                mac,
                name: String::from("-"),
                self_pc: false,
                name_source: String::new(),
                protected: false,
                protected_always: false,
                live: true,
            }),
        }
    }
    let named = router_names();
    for d in out.iter_mut() {
        if let Some(name) = named.get(&d.mac.to_ascii_lowercase()) {
            // The router's table is an indirect copy: it fills a row that has
            // no name, and never replaces one the device gave itself.
            offer_name(d, name, "router");
        }
    }
    sort_devices(&mut out);
    out
}

fn keep(ip: &str, mac: &str) -> bool {
    if mac.is_empty() || mac.starts_with("00:00:00") || mac.starts_with("ff:ff:ff") {
        return false;
    }
    let first: u32 = ip.split('.').next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let last: u32 = ip.split('.').last().and_then(|p| p.parse().ok()).unwrap_or(0);
    !(first == 0 || first >= 224 || ip == "255.255.255.255" || last == 255 || last == 0)
}

/// Names learned during the sweep in flight, and nothing more.
///
/// This was a file keyed by IP (`lan_probe_names.txt`) so the next scan would be
/// instant. The price was a name that outlived the device: an address handed to a
/// different phone kept its predecessor's name, and a restart resurrected the whole
/// set. A name is now kept only for the sweep that learned it, and cleared at the
/// start of the next one.
static SWEEP_NAMES: Mutex<Option<HashMap<String, (String, String)>>> = Mutex::new(None);

fn clear_sweep_names() {
    let mut guard = SWEEP_NAMES.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(HashMap::new());
}

fn sweep_names() -> HashMap<String, (String, String)> {
    let guard = SWEEP_NAMES.lock().unwrap_or_else(|e| e.into_inner());
    guard.clone().unwrap_or_default()
}

/// Keep a name for this sweep, with the source that produced it. A weaker source
/// never overwrites a stronger one, so a slow probe cannot undo a fast answer.
fn remember_probe(ip: &str, name: &str, source: &str) {
    if name.is_empty() || source.is_empty() {
        return;
    }
    let mut guard = SWEEP_NAMES.lock().unwrap_or_else(|e| e.into_inner());
    let all = guard.get_or_insert_with(HashMap::new);
    if let Some((_, old_source)) = all.get(ip) {
        if name_rank(old_source) > name_rank(source) {
            return;
        }
    }
    all.insert(ip.to_string(), (name.to_string(), source.to_string()));
}

/// A name has to look like a name. Reverse lookups return things such as
/// "0.0.0.0.0.0.0.0.8.E.F.ip6.arpa", which is an address, not a device.
fn usable_name(name: &str) -> bool {
    let n = name.trim();
    if n.is_empty() || n == "-" {
        return false;
    }
    let lower = n.to_lowercase();
    if lower.ends_with(".arpa") || lower.contains("in-addr") || lower.contains("ip6.arpa") {
        return false;
    }
    // A name made only of digits, dots and hex letters is an address.
    if n.split('.').all(|part| part.chars().all(|c| c.is_ascii_hexdigit())) && n.contains('.') {
        return false;
    }
    true
}

/// Take a batch of names learned passively and keep the ones that are new.
/// Its only caller is the mDNS half of the DHCP capture, so every name here
/// came from a device announcing itself.
pub fn remember_probed(found: &HashMap<String, String>) {
    // Names the DHCP capture overheard from a device announcing itself. Same
    // lifetime as everything else learned this sweep.
    for (ip, name) in found {
        if usable_name(name) {
            remember_probe(ip, name, "mdns");
        }
    }
}

fn sort_devices(list: &mut [Device]) {
    // This machine first, then by address: the row a person cares about most is
    // the one they are sitting at.
    list.sort_by_key(|d| (if d.self_pc { 0u8 } else { 1 }, addr_value(&d.ip)));
}

/// An address as a number, so rows can be ordered by it and two of them
/// compared without parsing twice. Anything that is not four octets sorts as
/// zero instead of panicking: an address is data, not a promise.
fn addr_value(ip: &str) -> u32 {
    ip.split('.')
        .filter_map(|p| p.parse::<u32>().ok())
        .fold(0u32, |acc, n| (acc << 8) | (n & 0xff))
}


/// Which of two rows for one device is the row to keep.
///
/// Identity first: a row that knows the hardware address describes the device
/// better than one that does not. Then presence, then the better name, then the
/// steadier address, so the outcome never depends on the order a sweep happened
/// to produce.
fn keep_first(a: &Device, b: &Device) -> bool {
    let a_mac = !a.mac.trim().is_empty();
    let b_mac = !b.mac.trim().is_empty();
    if a.self_pc != b.self_pc {
        return a.self_pc;
    }
    if a_mac != b_mac {
        return a_mac;
    }
    if a.live != b.live {
        return a.live;
    }
    let (ra, rb) = (name_rank(&a.name_source), name_rank(&b.name_source));
    if ra != rb {
        return ra > rb;
    }
    addr_value(&a.ip) <= addr_value(&b.ip)
}

/// Fold `extra` into `keep`: one device, one row, and nothing either row knew is
/// thrown away. The address and the hardware address stay the keeper's; the
/// flags are unioned because they describe the device, not the address it
/// happens to hold; and the name from the stronger source wins, which is the
/// same rule the passes already use when they offer a name.
fn absorb(keep: &mut Device, extra: &Device) {
    keep.self_pc |= extra.self_pc;
    keep.protected |= extra.protected;
    keep.protected_always |= extra.protected_always;
    keep.live |= extra.live;
    offer_name(keep, &extra.name, &extra.name_source);
}

/// One device is one row.
///
/// The hardware address is the identity: a phone that renews its lease is the
/// same device at a new address, so its two rows are one, and a stale row still
/// pointing at the old address cannot start a second history. An address joins
/// two rows only when a hardware address cannot tell them apart, because an
/// address is borrowed and can be handed to another machine; when both rows
/// carry different hardware addresses they are two devices and both stay.
///
/// A row that matches nothing is still a device that is here. It is kept: a miss
/// on the join is not evidence that anything left.
fn identity_pass(list: Vec<Device>) -> Vec<Device> {
    let mut out: Vec<Device> = Vec::with_capacity(list.len());
    let mut at_mac: HashMap<String, usize> = HashMap::new();
    let mut at_ip: HashMap<String, usize> = HashMap::new();
    for mut dev in list {
        // One form for the hardware address, because it is the key the screen
        // joins its counters on and a difference in case or separator would
        // miss the join.
        let mac = dev.mac.trim().to_ascii_lowercase().replace('-', ":");
        dev.mac = mac.clone();
        let same_mac = if mac.is_empty() { None } else { at_mac.get(&mac).copied() };
        let same_ip = at_ip.get(&dev.ip).copied().filter(|&at| {
            let other = out[at].mac.trim().to_ascii_lowercase().replace('-', ":");
            other.is_empty() || mac.is_empty() || other == mac
        });
        match same_mac.or(same_ip) {
            Some(at) => {
                if keep_first(&dev, &out[at]) {
                    let loser = std::mem::replace(&mut out[at], dev);
                    absorb(&mut out[at], &loser);
                } else {
                    absorb(&mut out[at], &dev);
                }
                // Both names now point at the row that survived, so the next
                // row sharing either one joins it instead of starting a third
                // row for the same device.
                let kept = out[at].mac.clone();
                if !kept.is_empty() {
                    at_mac.insert(kept, at);
                }
                at_ip.insert(out[at].ip.clone(), at);
            }
            None => {
                if !mac.is_empty() {
                    at_mac.insert(mac, out.len());
                }
                at_ip.insert(dev.ip.clone(), out.len());
                out.push(dev);
            }
        }
    }
    out
}

/// Every device on this subnet, sorted by address.
/// A device name is a string a stranger chose. Keep what a person can read and
/// drop the rest: a name carrying control characters or a broken encoding
/// renders as a row of empty boxes, which tells the user less than the maker
/// label would.
fn clean_name(raw: &str) -> String {
    let kept: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')'))
        .collect();
    // Runs of spaces come from names padded on the wire; one space reads right.
    let kept = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    let kept = kept.trim().trim_matches('.').to_string();
    if kept.chars().filter(|c| c.is_alphanumeric()).count() < 2 {
        String::new()
    } else {
        kept
    }
}

/// Adapters Windows invents are not devices on the network: a VirtualBox
/// host-only card, a TAP adapter or a Wi-Fi Direct ghost each carry a hardware
/// address but no machine stands behind it.
fn virtual_mac(mac: &str) -> bool {
    let m = mac.to_ascii_lowercase().replace('-', ":");
    [
        "0a:00:27", "00:ff", "00:50:56", "08:00:27", "52:54:00", "00:15:5d",
    ]
    .iter()
    .any(|p| m.starts_with(p))
}

/// Every hardware address that belongs to this machine. Windows invents extra
/// adapters (Wi-Fi Direct ghosts, host-only cards, TAP), and each of them can
/// answer as if it were a separate device on the network.
fn local_macs() -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let Ok(o) = hidden("getmac").args(["/fo", "csv", "/nh"]).output() else {
        return out;
    };
    let text = String::from_utf8_lossy(&o.stdout);
    for line in text.lines() {
        let Some(first) = line.trim().trim_start_matches('"').split('"').next() else {
            continue;
        };
        let m = first.to_ascii_lowercase().replace('-', ":");
        if m.len() == 17 && m.matches(':').count() == 5 {
            out.insert(m);
        }
    }
    out
}

/// The hardware address of the adapter that owns `ip`, which is what this
/// machine is called on the wire. ARP never lists your own address, so it
/// cannot come from the table.
fn own_mac(ip: &str) -> String {
    let Ok(o) = hidden("ipconfig").arg("/all").output() else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&o.stdout);
    let mut last = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.contains("Physical Address") {
            if let Some((_, v)) = l.split_once(':') {
                last = v.trim().to_ascii_lowercase().replace('-', ":");
            }
        }
        if l.contains(ip) && last.len() == 17 {
            return last;
        }
    }
    String::new()
}

/// How many sweeps in a row an address may miss before its row is dropped.
///
/// One miss is a sample, not a verdict: a phone asleep in a pocket answers no
/// ping, Windows ages a neighbour entry out, and a burst can simply be unlucky.
/// Two in a row is a device that is not there.
const MISS_LIMIT: u32 = 2;

/// What a sweep that did not see an address says about it. A row this sweep
/// did see never asks: it is live and its streak is back to zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Missing once: keep the row, quietly, with one more chance.
    Quiet,
    /// Missing [MISS_LIMIT] times in a row: it left.
    Gone,
}

/// The rule on its own, so it can be tested without a network. `misses` is the
/// streak stored before this sweep, not counting it.
fn fate(misses: u32) -> Fate {
    if misses + 1 >= MISS_LIMIT {
        Fate::Gone
    } else {
        Fate::Quiet
    }
}

/// One remembered row, and how many sweeps in a row have missed it.
struct Remembered {
    dev: Device,
    misses: u32,
}

/// The last row for every address, behind a mutex because a Refresh can land
/// while the previous scan is still running. This is the memory that turns a
/// single sample into presence.
static PRESENCE: Mutex<Option<HashMap<String, Remembered>>> = Mutex::new(None);

/// Fold this sweep into the remembered map, and return the rows to show.
///
/// The map is seeded from the list the last run saved, so an empty first sample
/// after a restart does not wipe the page either. An empty sweep needs no case
/// of its own: every remembered address misses once, and the second empty sweep
/// in a row drops the lot, which is the reference rule written as two cases
/// instead of one.
fn merge_presence(fresh: Vec<Device>) -> Vec<Device> {
    let seen: HashMap<String, Device> = fresh.into_iter().map(|d| (d.ip.clone(), d)).collect();
    let mut guard = PRESENCE.lock().unwrap_or_else(|e| e.into_inner());
    // Seeded empty on purpose. Presence is a memory for the session in front of
    // you: a row from a previous run is not evidence about a device now.
    let remembered = guard.get_or_insert_with(HashMap::new);
    let mut out: Vec<Device> = Vec::with_capacity(seen.len() + remembered.len());
    let mut seen_ips: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(seen.len());
    for (ip, mut dev) in seen {
        dev.live = true;
        seen_ips.insert(ip.clone());
        remembered.insert(ip, Remembered { dev: dev.clone(), misses: 0 });
        out.push(dev);
    }
    // Everything else earns a miss. One is still worth showing, quietly.
    remembered.retain(|ip, r| {
        if seen_ips.contains(ip) || r.dev.self_pc {
            return true;
        }
        match fate(r.misses) {
            Fate::Quiet => {
                r.misses += 1;
                r.dev.live = false;
                out.push(r.dev.clone());
                true
            }
            Fate::Gone => false,
        }
    });
    out
}

/// Older builds kept two files: a name per address and the whole last device
/// list. Neither is read any more. Removing them is not required for
/// correctness, but leaving them behind is how someone later believes the app
/// still remembers names.
fn forget_legacy_caches() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let base = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        for name in ["lan_probe_names.txt", "lan_devices.txt"] {
            let _ = std::fs::remove_file(base.join("quotavpn").join(name));
        }
    });
}

pub fn scan() -> Vec<Device> {
    forget_legacy_caches();
    clear_sweep_names();

    let Some(local) = local_ipv4() else {
        return Vec::new();
    };
    let prefix = prefix_for(local);
    // Only a home-sized subnet is swept: a /16 would take minutes and is not
    // what this screen is for.
    let hosts: u32 = if prefix >= 24 { 254 } else { 254 };
    let base = u32::from(local) & !((1u32 << (32 - prefix.min(30))) - 1);
    let me = local.to_string();

    let found: Arc<Mutex<Vec<(String, bool, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    // 32 workers: a /24 sweeps in about five seconds, which is the most a
    // person will wait for a device list before assuming it is broken.
    let chunk = 8usize;
    let mut batch = Vec::new();
    for i in 1..=hosts {
        let ip = Ipv4Addr::from(base + i).to_string();
        if ip == me {
            continue;
        }
        batch.push(ip);
        if batch.len() == chunk {
            let queue = std::mem::take(&mut batch);
            let sink = Arc::clone(&found);
            handles.push(thread::spawn(move || {
                for ip in queue {
                    let r = wake(ip);
                    if let Ok(mut g) = sink.lock() {
                        g.push(r);
                    }
                }
            }));
        }
    }
    if !batch.is_empty() {
        let sink = Arc::clone(&found);
        handles.push(thread::spawn(move || {
            for ip in batch {
                let r = wake(ip);
                if let Ok(mut g) = sink.lock() {
                    g.push(r);
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    // The list has to appear at once, so names come from what is already known:
    // what a device announced to the router, and what asking it directly found
    // last time. Probing every device again is what made Refresh feel broken.
    let table = arp_table();
    let announced_by_mac = crate::dhcp_names::cached();
    let probed = sweep_names();
    let mut out: Vec<Device> = Vec::new();
    let results = found.lock().map(|g| g.clone()).unwrap_or_default();
    for (ip, alive, ttl) in results {
        let mac = table.get(&ip).cloned().unwrap_or_default();
        if !keep(&ip, &mac) {
            continue;
        }
        if !alive && mac.is_empty() {
            continue;
        }
        // Where the name came from travels with the name: a probe answer knows
        // which protocol answered it, and the router's DHCP cache is a source
        // of its own. Dropping that here is what made every row claim "device".
        let resolved: Option<(String, &str)> = probed
            .get(&ip)
            .filter(|pair| !pair.0.is_empty())
            .map(|pair| (pair.0.clone(), pair.1.as_str()))
            .or_else(|| {
                announced_by_mac
                    .get(&mac)
                    .filter(|name| !name.is_empty())
                    .map(|name| (name.to_string(), "dhcp" as &str))
            });
        // A name if one exists, otherwise the most useful true thing: who made
        // it, and (from the background pass) what kind of device it is.
        let ttl: u32 = ttl.parse().unwrap_or(0);
        let (label, source) = match resolved {
            Some((name, src)) if usable_name(&name) => (name, src.to_string()),
            _ => (crate::fingerprint::describe_ttl(&mac, ttl), String::from("vendor")),
        };
        out.push(Device {
            ip,
            mac,
            name: label,
            self_pc: false,
            name_source: source,
            protected: false,
            protected_always: false,
            // Answered the sweep, or its hardware address resolved: either is
            // evidence that it is here now.
            live: true,
        });
    }

    // One sweep is a sample, not a verdict. Folding it into the memory is what
    // stops a Refresh during a quiet moment from returning an empty table, and
    // what drops a device that really left on its second miss in a row.
    let mut out = merge_presence(out);

    // This machine is always in the list and always labelled.
    let me_mac = table
        .get(&me)
        .cloned()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| own_mac(&me));
    let host = std::env::var("COMPUTERNAME")
        .unwrap_or_else(|_| String::from("this pc"))
        .to_lowercase();
    out.push(Device {
        ip: me.clone(),
        mac: me_mac,
        name: host,
        self_pc: true,
        name_source: String::from("self"),
        protected: false,
        protected_always: false,
        // The machine drawing the list is, by definition, present.
        live: true,
    });

    // Names come from strangers: keep the readable part, and when nothing
    // readable is left let the fingerprint module say the most useful true
    // thing (who made it, then what it is). The word "private" used to be
    // stamped on every unnamed row, which was both a guess and, for a real
    // card in locally-administered space, simply wrong: 148 register blocks
    // live there, so a genuine 3COM read as "Private address".
    for d in out.iter_mut() {
        if d.self_pc {
            continue;
        }
        let cleaned = clean_name(&d.name);
        if !cleaned.is_empty() {
            d.name = cleaned;
            continue;
        }
        if d.name == "-" {
            continue;
        }
        let label = crate::fingerprint::describe(&d.mac, &d.ip, "", None);
        d.name = label.clone();
        if label != "-" {
            d.name_source = String::from("vendor");
        }
    }

    // One device is one row, decided by hardware address, before anything is
    // written down or shown: the roster and the counters then agree on what a
    // device is.
    let mut out = identity_pass(out);
    sort_devices(&mut out);

    // A hardware address belongs to exactly one machine. A stale row can point
    // several addresses at the same hardware, and this machine's own virtual
    // adapters appear as if they were devices. Both make the list lie, so the
    // invented ones go and only the first address keeps a shared address.
    let named = router_names();
    for d in out.iter_mut() {
        if let Some(name) = named.get(&d.mac.to_ascii_lowercase()) {
            // The router's table is an indirect copy: it names the devices
            // that answer nothing, and never replaces a name a device gave
            // itself.
            offer_name(d, name, "router");
        }
    }

    let mine = local_macs();
    out.retain(|d| d.self_pc || !virtual_mac(&d.mac));
    out.retain(|d| {
        d.self_pc || !mine.contains(&d.mac.to_ascii_lowercase().replace('-', ":"))
    });
    sort_devices(&mut out);
    out
}


/// Is the packet driver present? The control layer needs it; the listing does
/// not, so the app can show who is on the line before asking for anything.
pub fn npcap_installed() -> bool {
    let sys = std::env::var("SystemRoot").unwrap_or_else(|_| String::from("C:/Windows"));
    for rel in ["System32/Npcap/wpcap.dll", "System32/wpcap.dll", "SysWOW64/Npcap/wpcap.dll"] {
        if std::path::Path::new(&format!("{sys}/{rel}")).is_file() {
            return true;
        }
    }
    false
}

/// Ask the devices that still have no name, in the background, and remember
/// what they answer. The list is already on screen by the time this runs.
pub fn enrich_names(list: Vec<Device>) {
    let mut handles = Vec::new();
    for d in list {
        if d.self_pc || (d.name != "-" && !d.name.is_empty()) {
            continue;
        }
        let ip = d.ip.clone();
        let mac = d.mac.clone();
        handles.push(thread::spawn(move || {
            // The strongest answer wins, not the last one. Every probe records
            // which protocol produced its name, so a late reverse-name lookup
            // can no longer overwrite what the device said about itself.
            let mut found = String::new();
            let mut found_src: &str = "";
            // Ask on the wire first: a socket has to get past the firewall,
            // a frame addressed to the device does not.
            if !mac.is_empty() {
                if let Some(n) = crate::lan_ask::mdns_ask(&ip, &mac, std::time::Duration::from_millis(900)) {
                    if name_rank("mdns") > name_rank(found_src) {
                        found = n;
                        found_src = "mdns";
                    }
                }
            }
            if let Ok(o) = hidden("ping").args(["-a", "-n", "1", "-w", "600", &ip]).output() {
                let text = String::from_utf8_lossy(&o.stdout).to_string();
                for raw in text.lines() {
                    let line = raw.trim();
                    if let Some(rest) = line.strip_prefix("Pinging ") {
                        let head = rest.split(" with ").next().unwrap_or("");
                        let head = head.split(" [").next().unwrap_or("").trim();
                        if !head.is_empty() && head != ip && head.split('.').count() != 4 {
                            if name_rank("ping") > name_rank(found_src) {
                                found = head.to_string();
                                found_src = "ping";
                            }
                        }
                        break;
                    }
                }
            }
            if found.is_empty() {
                if let Ok(v4) = ip.parse::<std::net::Ipv4Addr>() {
                    let wait = std::time::Duration::from_millis(700);
                    if let Some(n) = crate::lan_names::netbios(v4, wait) {
                        found = n;
                        found_src = "netbios";
                    } else if let Some(n) = crate::lan_names::mdns(v4, wait) {
                        found = n;
                        found_src = "mdns";
                    }
                }
            }
            if usable_name(&found) {
                remember_probe(&ip, &found, found_src);
            } else {
                // No name to be had, so say what the device is instead.
                let kind = crate::fingerprint::kind(&ip);
                let label = crate::fingerprint::describe(&mac, &ip, "", kind);
                if label != "-" {
                    remember_probe(&ip, &label, "vendor");
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Presence is a policy, and the policy is pure: the numbers below are
    // sweeps, not seconds, and no network is involved. The guard exists for the
    // sleepy phone, so what it must never do is drop a row on the first miss.
    // A name that survives the sweep which learned it is a name that can be
    // wrong: the address gets handed to another device and the old label stays.
    // These two pin the new contract, including that nothing reaches the disk.
    // These three act on one global map, so they take a lock of their own: the
    // suite runs in parallel and each of them starts by clearing the map.
    static NAME_TESTS: Mutex<()> = Mutex::new(());

    fn name_test_guard() -> std::sync::MutexGuard<'static, ()> {
        NAME_TESTS.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn a_name_lives_for_one_sweep_only() {
        let _serial = name_test_guard();
        clear_sweep_names();
        remember_probe("192.168.1.50", "yousef-phone", "mdns");
        assert_eq!(
            sweep_names().get("192.168.1.50").map(|p| p.0.as_str()),
            Some("yousef-phone")
        );
        clear_sweep_names();
        assert!(sweep_names().is_empty(), "a new sweep must start unknown");
    }

    #[test]
    fn a_weaker_source_cannot_overwrite_a_stronger_one() {
        let _serial = name_test_guard();
        clear_sweep_names();
        remember_probe("192.168.1.51", "admin-name", "admin");
        remember_probe("192.168.1.51", "vendor-ish", "vendor");
        assert_eq!(
            sweep_names().get("192.168.1.51").map(|p| p.1.as_str()),
            Some("admin"),
            "the weaker answer won"
        );
        clear_sweep_names();
    }

    #[test]
    fn a_name_never_reaches_the_disk() {
        let _serial = name_test_guard();
        // A machine that ran an older build still has these on disk, which is the
        // whole reason the one-time cleanup exists. Clear them first so the test
        // measures this code rather than the history of the machine.
        forget_legacy_caches();
        let base = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        for name in ["lan_probe_names.txt", "lan_devices.txt"] {
            let _ = std::fs::remove_file(base.join("quotavpn").join(name));
        }

        clear_sweep_names();
        remember_probe("192.168.1.52", "some-device", "netbios");

        for name in ["lan_probe_names.txt", "lan_devices.txt"] {
            assert!(
                !base.join("quotavpn").join(name).exists(),
                "a name cache file came back: {name}"
            );
        }
        clear_sweep_names();
    }

    #[test]
    fn one_miss_is_not_a_verdict() {
        assert_eq!(fate(0), Fate::Quiet);
        assert_ne!(fate(0), Fate::Gone);
    }

    #[test]
    fn the_second_miss_in_a_row_drops_the_row() {
        assert_eq!(fate(1), Fate::Gone);
        assert_eq!(fate(MISS_LIMIT - 1), Fate::Gone);
    }

    #[test]
    fn a_device_that_really_left_does_not_stay_forever() {
        // Every streak past the limit is still a verdict: the map cannot keep a
        // row alive by holding a large counter.
        assert_eq!(fate(MISS_LIMIT), Fate::Gone);
        assert_eq!(fate(99), Fate::Gone);
    }

    fn row(ip: &str, mac: &str, live: bool) -> Device {
        Device {
            ip: ip.to_string(),
            mac: mac.to_string(),
            name: String::from("-"),
            self_pc: false,
            name_source: String::new(),
            protected: false,
            protected_always: false,
            live,
        }
    }

    /// One device is one row: a phone that renews its lease comes back at a new
    /// address with the same hardware address, and the row it leaves behind is
    /// the same row at the live address.
    #[test]
    fn a_renewed_lease_is_one_row() {
        let out = identity_pass(vec![
            row("192.168.1.5", "aa:bb:cc:dd:ee:01", false),
            row("192.168.1.9", "AA-BB-CC-DD-EE-01", true),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].ip, "192.168.1.9", "the live address wins the row");
        assert_eq!(out[0].mac, "aa:bb:cc:dd:ee:01", "one form for the key");
        assert!(out[0].live);
    }

    /// Nothing is merged away on a guess: a row with no hardware address cannot
    /// be proven equal to anything, so it keeps its own row.
    #[test]
    fn a_row_without_a_hardware_address_is_kept() {
        let out = identity_pass(vec![
            row("192.168.1.5", "aa:bb:cc:dd:ee:01", true),
            row("192.168.1.7", "", true),
        ]);
        assert_eq!(out.len(), 2);
    }

    /// The same address held by two machines is two devices, not one: an address
    /// is borrowed and can be handed on, and the hardware address is what tells
    /// them apart.
    #[test]
    fn one_address_two_hardware_addresses_is_two_rows() {
        let out = identity_pass(vec![
            row("192.168.1.5", "aa:bb:cc:dd:ee:01", true),
            row("192.168.1.5", "aa:bb:cc:dd:ee:02", true),
        ]);
        assert_eq!(out.len(), 2);
    }

    /// A row that arrived without its hardware address still joins the device
    /// that has one, and presence is unioned rather than lost.
    #[test]
    fn a_row_without_a_hardware_address_joins_on_the_address() {
        let out = identity_pass(vec![
            row("192.168.1.5", "aa:bb:cc:dd:ee:01", false),
            row("192.168.1.5", "", true),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].mac, "aa:bb:cc:dd:ee:01");
        assert!(out[0].live, "presence is unioned, not lost");
    }

    /// A merge keeps the name from the stronger source and the flags of both,
    /// whichever row came first.
    #[test]
    fn a_merge_keeps_the_better_name_and_unions_the_flags() {
        let mut a = row("192.168.1.5", "aa:bb:cc:dd:ee:01", true);
        a.name = String::from("acme");
        a.name_source = String::from("vendor");
        let mut b = row("192.168.1.9", "aa:bb:cc:dd:ee:01", false);
        b.name = String::from("yousef-phone");
        b.name_source = String::from("mdns");
        b.protected = true;
        for out in [identity_pass(vec![a.clone(), b.clone()]), identity_pass(vec![b, a])] {
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].name, "yousef-phone");
            assert_eq!(out[0].name_source, "mdns");
            assert!(out[0].protected, "protection belongs to the device");
        }
    }
}
