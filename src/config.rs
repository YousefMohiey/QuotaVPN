use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/embed.rs"));

/// Pre-pointed server (owner's) - used for zero-setup auto-connect when the
/// embedded device key was baked in at compile time.
pub const DEFAULT_HOST: &str = "quotavpn.duckdns.org";
pub const DEFAULT_USER: &str = "ubuntu";
pub const DEFAULT_PORT: u16 = 22;
/// The public OpenRad network that shipped before spaces became private. It is
/// only used to recognize and retire the old default; new defaults are private.
pub const LEGACY_NETWORK: &str = "QuotaVPN";

/// App config - lives in the OS config dir (works for every user, no fixed
/// paths hard-coded into the program).
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub server_ip: String,
    pub ssh_user: String,
    pub ssh_port: u16,
    /// OpenSSH private key (PEM) - auto-generated on first run by the app.
    pub private_key: String,
    /// Public key line (shown once so the user can paste it in Oracle console).
    pub public_key: String,
    pub cards: Vec<Card>,
    /// True once the server has Xray installed & synced (avoids re-installing).
    pub synced: bool,
    /// Shared Hysteria2 password fetched from the server (game mode needs it).
    #[serde(default)]
    pub hy2_password: String,
    /// WireGuard server public key, fetched once from the server.
    #[serde(default)]
    pub wg_server_pub: String,
    /// AmneziaWG obfuscation parameters (JSON), fetched once from the server.
    #[serde(default)]
    pub awg_params: String,
    /// Tunnel scope: "all" (whole PC) or "apps" (only tun_apps processes).
    #[serde(default)]
    pub tun_mode: String,
    /// Process names routed through the tunnel in "apps" mode.
    #[serde(default)]
    pub tun_apps: Vec<String>,
    /// The app build that last re-registered every card on the server. Lets
    /// the launch refresh run once per update instead of on every open.
    #[serde(default)]
    pub healed_version: String,
    /// Update channel the user picked. Empty or "normal" tracks official
    /// releases only; "experimental" also takes the newest test builds.
    #[serde(default)]
    pub update_channel: String,
    /// Hardware addresses the user chose to keep safe. A protected device can
    /// never be capped or blocked, whatever the rest of the screen says.
    #[serde(default)]
    pub lan_protected: Vec<String>,
    /// Spaces this install belongs to. The records carry the WireGuard private
    /// key and the coordinator token; the UI snapshot never reads them.
    #[serde(default)]
    pub spaces: Vec<crate::spaces::SpaceRecord>,
    /// QuotaSpace LAN networks. Each record is its own OpenRad network, so its
    /// peers and broadcast domain are isolated from every other record.
    #[serde(default)]
    pub lan_spaces: Vec<LanSpace>,
    /// Which space the engine should attach on the next connect. Empty means
    /// none. A space is attached at session start, not live, so changing this
    /// takes effect on the next tunnel.
    #[serde(default)]
    pub active_space: String,
    /// The space the running session actually carries. Written when a connect
    /// succeeds and cleared on stop, so the screen can tell "selected" from
    /// "applied" instead of claiming a live space the engine never loaded.
    #[serde(default)]
    pub tunnel_space: String,
    /// Coordinator address used for Spaces. The invite can carry it too, so a
    /// joining PC never needs an environment variable or a terminal.
    #[serde(default)]
    pub spaces_api: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Card {
    pub name: String,
    pub uuid: String,
    pub card_type: String, // "Gamerz" | "Streamerz"
    #[serde(default)]
    pub sni: String,
    /// WireGuard client key for this card (provisioned on first WG connect).
    #[serde(default)]
    pub wg_private: String,
    /// WireGuard address for this card, e.g. "10.8.0.7/32".
    #[serde(default)]
    pub wg_addr: String,
    /// True once the server confirmed this UUID is in Xray. Cached so a normal
    /// connect does not wait on the control plane; the failure path still
    /// repairs a card that disappeared.
    #[serde(default)]
    pub registered: bool,
}

impl Card {
    /// Picker label: name + domain ("Streamerz - youtube.com"), so cards
    /// are told apart by where they route, not just their kind.
    pub fn label(&self) -> String {
        if self.sni.is_empty() {
            format!("{} - {}", self.name, self.card_type)
        } else {
            format!("{} - {}", self.name, self.sni)
        }
    }
}

/// One isolated QuotaSpace LAN. `network` is the OpenRad network name on the
/// coordination server; `password` is the network password for a private one.
/// The id is the network name, which is unique, so two records can never point
/// at the same network. `primary` marks the one baseline space the app always
/// keeps; `provisioned` is false until the first connect has created the
/// network on the server.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LanSpace {
    pub id: String,
    pub name: String,
    pub network: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub password_sealed: String,
    #[serde(default)]
    pub owner: bool,
    #[serde(default, rename = "default")]
    pub primary: bool,
    #[serde(default)]
    pub provisioned: bool,
}

fn encode_name(text: &str) -> String {
    URL_SAFE_NO_PAD.encode(text.as_bytes())
}

fn decode_name(text: &str) -> Option<String> {
    URL_SAFE_NO_PAD
        .decode(text)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn valid_network(network: &str) -> bool {
    !network.is_empty()
        && network.len() <= 48
        && network
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_password(password: &str) -> bool {
    password.len() <= 64 && password.bytes().all(|b| b.is_ascii_alphanumeric())
}

impl LanSpace {
    pub fn is_default(&self) -> bool {
        self.primary
    }

    /// The line another PC pastes to join this exact network. The name is
    /// base64 encoded so spaces in it, Arabic, or punctuation cannot break the
    /// field separators. A legacy public network has an empty password and the
    /// code still ends with a colon; new default and private spaces always
    /// carry one.
    pub fn invite(&self) -> String {
        format!(
            "qspace1:{}:{}:{}",
            encode_name(&self.name),
            self.network,
            self.password
        )
    }

    /// A new private network for a space the user created. The network name is
    /// generated, so two spaces never collide even when their display names
    /// do. The creator owns the network and can delete it for everyone.
    pub fn new(name: &str) -> Self {
        let network = format!(
            "qs-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        let password = uuid::Uuid::new_v4().simple().to_string()[..24].to_string();
        Self {
            id: network.clone(),
            name: name.trim().to_string(),
            network,
            password,
            password_sealed: String::new(),
            owner: true,
            primary: false,
            provisioned: false,
        }
    }

    pub fn from_invite(text: &str) -> Result<Self, String> {
        // The longest valid code is well under 200 characters. Cap before any
        // base64 decode so a huge paste cannot allocate the app to a halt.
        if text.len() > 512 {
            return Err("that invite code is too long".into());
        }
        let mut parts = text.trim().splitn(4, ':');
        if parts.next() != Some("qspace1") {
            return Err("that invite code is not a QuotaSpace code".into());
        }
        let name = parts
            .next()
            .and_then(decode_name)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty() && name.chars().count() <= 40)
            .ok_or("that invite code has no valid space name")?;
        let network = parts.next().unwrap_or_default().to_string();
        if !valid_network(&network) {
            return Err("that invite code has no valid network".into());
        }
        let password = parts.next().unwrap_or_default().to_string();
        if !valid_password(&password) {
            return Err("that invite code has no valid password".into());
        }
        Ok(Self {
            id: network.clone(),
            name,
            network,
            password,
            password_sealed: String::new(),
            owner: false,
            primary: false,
            provisioned: false,
        })
    }

    /// The baseline space, private from the first run. The network is created
    /// on the first connect, because the app has no server session at startup.
    pub fn default_space() -> Self {
        let mut space = Self::new("QuotaSpace");
        space.primary = true;
        space.provisioned = false;
        space
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("quotavpn")
    }

    /// Move the old QuotaCards folders to QuotaVPN on first launch. The
    /// roaming folder carries the config, cards and sealed credentials; the
    /// local folder carries the engine and driver cache. If a folder is
    /// locked, the small roaming files are copied and the engine is rebuilt.
    pub fn migrate_dirs() {
        let roaming = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let old = roaming.join("quotacards");
        let new = roaming.join("quotavpn");
        if !new.exists() && old.exists() && std::fs::rename(&old, &new).is_err() {
            let _ = std::fs::create_dir_all(&new);
            for name in [
                "config.json",
                "card_credentials.bin",
                "device_secret",
                "device_hmac.bin",
                "device_hmac.uuid",
            ] {
                let _ = std::fs::copy(old.join(name), new.join(name));
            }
        }
        let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let old = local.join("quotacards");
        let new = local.join("quotavpn");
        if !new.exists() && old.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    /// Per-device secret, made once per install and kept beside the config.
    /// The server agent ties every call that changes anything to this value,
    /// so the device key that ships inside the app can only ever speak for a
    /// device that does not exist yet.
    pub fn device_secret() -> String {
        let p = Self::config_dir().join("device_secret");
        if let Ok(existing) = std::fs::read_to_string(&p) {
            let s = existing.trim().to_string();
            if s.len() >= 32 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                return s;
            }
        }
        let s = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        if let Some(dir) = Self::config_dir().to_str() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&p, &s);
        s
    }

    /// Parse a config, tolerating the UTF-8 BOM that PowerShell and some
    /// editors add. A BOM previously made serde fail, which reset the whole
    /// config and lost the cards and spaces.
    fn parse(text: &str) -> Option<Self> {
        serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
    }

    pub fn load() -> Self {
        let p = Self::config_path();
        if let Ok(txt) = std::fs::read_to_string(&p) {
            if let Some(c) = Self::parse(&txt) {
                return c;
            }
        }
        // A force-kill during the old direct write could truncate the file.
        // The backup is the last config that parsed before that write.
        let bak = p.with_extension("json.bak");
        if let Ok(txt) = std::fs::read_to_string(&bak) {
            if let Some(c) = Self::parse(&txt) {
                return c;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(dir) = Self::config_dir().to_str() {
            let _ = std::fs::create_dir_all(dir);
        }
        let p = Self::config_path();
        let Ok(txt) = serde_json::to_string_pretty(self) else {
            return;
        };
        // Keep the last good file, but never copy a corrupt one over a good
        // backup. Then replace the main file through a temporary file so a
        // kill mid-write cannot leave a half-written config.
        let bak = p.with_extension("json.bak");
        if let Ok(old) = std::fs::read_to_string(&p) {
            if Self::parse(&old).is_some() {
                let _ = std::fs::copy(&p, &bak);
            }
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, &txt).is_ok() {
            if std::fs::rename(&tmp, &p).is_err() {
                let _ = std::fs::write(&p, &txt);
                let _ = std::fs::remove_file(&tmp);
            }
        } else {
            let _ = std::fs::write(&p, &txt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, LanSpace};

    #[test]
    fn a_bom_does_not_reset_the_config() {
        let text = serde_json::to_string(&AppConfig::default()).expect("config json");
        let with_bom = format!("\u{feff}{text}");
        assert!(AppConfig::parse(&with_bom).is_some());
    }

    #[test]
    fn an_invite_round_trips_a_name_with_spaces_and_arabic() {
        let space = LanSpace {
            id: "qs-abc123".into(),
            name: "ليلة واركرافت 3".into(),
            network: "qs-abc123".into(),
            password: "a1b2c3d4e5f60718".into(),
            password_sealed: String::new(),
            owner: true,
            primary: false,
            provisioned: true,
        };
        let parsed = LanSpace::from_invite(&space.invite()).expect("invite parses");
        assert_eq!(parsed.name, space.name);
        assert_eq!(parsed.network, space.network);
        assert_eq!(parsed.password, space.password);
        assert_eq!(parsed.id, space.network);
        assert!(!parsed.owner);
        assert!(!parsed.primary);
    }

    #[test]
    fn the_default_space_is_private_and_waits_for_its_first_connect() {
        let space = LanSpace::default_space();
        assert!(space.is_default());
        assert!(!space.provisioned);
        assert!(!space.password.is_empty());
        assert!(space.network.starts_with("qs-"));
        let parsed = LanSpace::from_invite(&space.invite()).expect("default invite parses");
        assert_eq!(parsed.network, space.network);
        assert_eq!(parsed.name, "QuotaSpace");
        assert_eq!(parsed.password, space.password);
        assert!(!parsed.primary);
    }

    #[test]
    fn a_bad_invite_is_refused() {
        assert!(LanSpace::from_invite("qvs1:whatever").is_err());
        assert!(LanSpace::from_invite("qspace1:zz:QuotaVPN:").is_err());
        assert!(LanSpace::from_invite("qspace1:51736b:bad network:pass").is_err());
        assert!(LanSpace::from_invite("qspace1:51736b:QuotaVPN:bad pass").is_err());
        assert!(LanSpace::from_invite(&"qspace1:".repeat(100)).is_err());
    }
}