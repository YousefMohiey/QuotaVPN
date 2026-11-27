//! Per-card management credentials, one DPAPI-sealed map.
//!
//! A card is identified by its VLESS UUID, but the management secret is a
//! separate server-issued value. The map holds one entry per card, so importing
//! a second card cannot overwrite the first. Nothing here writes a plaintext
//! secret: the whole map is sealed with `CryptProtectData` for the current
//! Windows user before it reaches disk.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use quotacards::{
    config::{AppConfig, Card, DEFAULT_HOST},
    qvreg,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

const STORE_FILE: &str = "card_credentials.bin";
const LEGACY_SECRET: &str = "device_hmac.bin";
const LEGACY_UUID: &str = "device_hmac.uuid";
const LEGACY_DEVICE_SECRET: &str = "device_secret";

type Map = BTreeMap<String, String>;

fn store_path() -> std::path::PathBuf {
    AppConfig::config_dir().join(STORE_FILE)
}

#[cfg(windows)]
mod dpapi {
    // CryptProtectData / CryptUnprotectData, current-user scope. The plaintext
    // exists only in this process's memory.
    use std::ffi::c_void;

    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const Blob,
            desc: *const u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const Blob,
            desc: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut Blob,
        ) -> i32;
        fn LocalFree(p: *mut c_void) -> *mut c_void;
    }

    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let mut out = Blob {
                len: 0,
                data: std::ptr::null_mut(),
            };
            let input = Blob {
                len: plain.len() as u32,
                data: plain.as_ptr() as *mut u8,
            };
            if CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut out,
            ) == 0
            {
                return None;
            }
            let bytes = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
            LocalFree(out.data as *mut c_void);
            Some(bytes)
        }
    }

    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let mut out = Blob {
                len: 0,
                data: std::ptr::null_mut(),
            };
            let input = Blob {
                len: sealed.len() as u32,
                data: sealed.as_ptr() as *mut u8,
            };
            if CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut out,
            ) == 0
            {
                return None;
            }
            let bytes = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
            LocalFree(out.data as *mut c_void);
            Some(bytes)
        }
    }
}

#[cfg(not(windows))]
mod dpapi {
    // The Windows app is the only caller. Failing closed is the right answer
    // on a target with no DPAPI: never write a plaintext secret.
    pub fn protect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
    pub fn unprotect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

fn validate(map: &Map) -> Result<(), String> {
    for (uuid, secret) in map {
        if qvreg::canonical_uuid(uuid).as_deref() != Some(uuid.as_str()) {
            return Err("the saved card credentials contain an invalid card id".into());
        }
        qvreg::secret_from_hex(secret)?;
    }
    Ok(())
}

fn save(map: &Map) -> Result<(), String> {
    let path = store_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create config directory: {e}"))?;
    }
    let text = serde_json::to_string(map).map_err(|e| format!("encode card credentials: {e}"))?;
    let sealed =
        dpapi::protect(text.as_bytes()).ok_or("the card credentials could not be sealed")?;
    // Same-directory temporary file, then a rename with one sealed backup.
    // Windows will not rename over an existing file, and a crash between
    // removing the old file and renaming the new one must not lose the store.
    let tmp = path.with_extension("bin.tmp");
    let backup = path.with_extension("bin.bak");
    std::fs::write(&tmp, &sealed).map_err(|e| format!("write card credentials: {e}"))?;
    if path.exists() {
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(&path, &backup).map_err(|e| format!("back up card credentials: {e}"))?;
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        // Put the old store back so the next launch still has every card.
        let _ = std::fs::rename(&backup, &path);
        return Err(format!("replace card credentials: {e}"));
    }
    let _ = std::fs::remove_file(&backup);
    Ok(())
}

/// Load the sealed map, migrating the first migration build's single-card
/// files if they are still present. A seal that cannot be opened is an error,
/// not an empty store: silently forgetting every credential is worse.
fn load() -> Result<Map, String> {
    let path = store_path();
    // A crash in the middle of a save leaves the previous sealed map in the
    // backup. Recover it before reporting an empty store.
    let backup = path.with_extension("bin.bak");
    if !path.exists() && backup.exists() {
        let _ = std::fs::rename(&backup, &path);
    }
    let mut map = match std::fs::read(&path) {
        Ok(sealed) => {
            let plain = dpapi::unprotect(&sealed)
                .ok_or("the saved card credentials could not be opened for this Windows user")?;
            let text =
                String::from_utf8(plain).map_err(|_| "the saved card credentials are not text")?;
            let parsed: Map = serde_json::from_str(&text)
                .map_err(|_| "the saved card credentials are damaged")?;
            parsed
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(format!("read card credentials: {e}")),
    };
    validate(&map)?;

    let dir = AppConfig::config_dir();
    let legacy_secret = dir.join(LEGACY_SECRET);
    let legacy_uuid = dir.join(LEGACY_UUID);
    let mut migrated = false;
    if let (Ok(sealed), Ok(uuid_text)) = (
        std::fs::read(&legacy_secret),
        std::fs::read_to_string(&legacy_uuid),
    ) {
        if let (Some(uuid), Some(secret)) =
            (qvreg::canonical_uuid(&uuid_text), dpapi::unprotect(&sealed))
        {
            if secret.len() == 32 {
                map.entry(uuid)
                    .or_insert_with(|| qvreg::secret_to_hex(&secret));
                migrated = true;
            }
        }
    }
    if migrated {
        save(&map)?;
        let _ = std::fs::remove_file(legacy_secret);
        let _ = std::fs::remove_file(legacy_uuid);
    }
    Ok(map)
}

/// Every card that has a management credential on this PC.
pub fn uuids() -> Vec<String> {
    load()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

pub fn has(uuid: &str) -> bool {
    let Some(uuid) = qvreg::canonical_uuid(uuid) else {
        return false;
    };
    load().map(|map| map.contains_key(&uuid)).unwrap_or(false)
}

/// Seal arbitrary text for this Windows user. The QuotaSpace network password
/// uses the same DPAPI protection as the card credentials, so a copied profile
/// cannot hand someone a working invite.
pub fn seal_text(text: &str) -> Result<String, String> {
    let sealed = dpapi::protect(text.as_bytes()).ok_or("the value could not be sealed")?;
    Ok(B64.encode(sealed))
}

pub fn open_text(text: &str) -> Result<String, String> {
    let sealed = B64
        .decode(text)
        .map_err(|_| "the sealed value is damaged".to_string())?;
    let plain = dpapi::unprotect(&sealed)
        .ok_or("the sealed value could not be opened for this Windows user")?;
    String::from_utf8(plain).map_err(|_| "the sealed value is not text".to_string())
}

/// Put a card back in the profile when its sealed credential survived but the
/// card list did not. This is the honest recovery for a config that was reset
/// or rewritten from a stale state: the credential is the part that cannot be
/// recreated without the provider, so the card it belongs to must not vanish
/// from the UI with it. The type and domain are the defaults import uses; the
/// preset switch rewrites the domain on the first press anyway.
fn merge_known_cards(cfg: &mut AppConfig, known: Vec<String>) -> usize {
    let mut added = 0;
    for uuid in known {
        if cfg.cards.iter().any(|card| card.uuid == uuid) {
            continue;
        }
        cfg.cards.push(Card {
            name: "Gamerz".into(),
            uuid,
            card_type: "Gamerz".into(),
            sni: "ea.com".into(),
            wg_private: String::new(),
            wg_addr: String::new(),
            registered: false,
        });
        added += 1;
    }
    added
}

/// Restore every card the sealed store still knows. Returns how many were
/// added so the caller can persist the healed profile once.
pub fn restore_missing_cards(cfg: &mut AppConfig) -> usize {
    merge_known_cards(cfg, uuids())
}

pub fn secret_for(uuid: &str) -> Result<Vec<u8>, String> {
    let uuid = qvreg::canonical_uuid(uuid).ok_or("that card id is malformed")?;
    let map = load()?;
    let secret = map.get(&uuid).ok_or(
        "no management credential is imported for this card; the card was not provisioned on this PC",
    )?;
    qvreg::secret_from_hex(secret)
}

pub fn store(uuid: &str, secret: &[u8]) -> Result<(), String> {
    if secret.len() != 32 {
        return Err("the card secret must be 32 bytes".into());
    }
    let uuid = qvreg::canonical_uuid(uuid).ok_or("that card id is malformed")?;
    let mut map = load()?;
    map.insert(uuid, qvreg::secret_to_hex(secret));
    save(&map)
}

pub fn forget(uuid: &str) -> Result<(), String> {
    let uuid = qvreg::canonical_uuid(uuid).ok_or("that card id is malformed")?;
    let mut map = load()?;
    map.remove(&uuid);
    save(&map)
}

#[derive(serde::Deserialize)]
struct ProfileCard {
    uuid: String,
    secret: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    sni: String,
    #[serde(default)]
    name: String,
}

#[derive(serde::Deserialize)]
struct Profile {
    #[serde(default)]
    hy2_password: String,
    cards: Vec<ProfileCard>,
}

fn profile_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("qvprofile.json"));
        }
    }
    paths.push(AppConfig::config_dir().join("qvprofile.json"));
    if let Some(parent) = AppConfig::config_dir().parent() {
        // A profile left in the old folder before the rename still imports.
        paths.push(parent.join("quotacards").join("qvprofile.json"));
    }
    paths
}

/// Import a provider-generated `qvprofile.json` without any customer paste.
/// The file holds one or two cards; each secret is sealed and the card is
/// placed in config.json. The profile is renamed after a successful read so a
/// later launch does not repeat the import.
pub fn import_profile(cfg: &mut AppConfig) -> Result<usize, String> {
    let Some(path) = profile_paths().into_iter().find(|path| path.exists()) else {
        return Ok(0);
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read profile: {e}"))?;
    let profile: Profile =
        serde_json::from_str(&text).map_err(|e| format!("parse profile: {e}"))?;
    let profile_hy2 = profile.hy2_password.clone();
    let mut imported = 0;
    for entry in profile.cards {
        let uuid = qvreg::canonical_uuid(&entry.uuid).ok_or("profile has a bad card id")?;
        let secret = qvreg::secret_from_hex(&entry.secret)?;
        store(&uuid, &secret)?;
        let kind = if entry.kind == "Streamerz" {
            "Streamerz"
        } else {
            "Gamerz"
        };
        let sni = if entry.sni.trim().is_empty() {
            if kind == "Streamerz" {
                "youtube.com".to_string()
            } else {
                "ea.com".to_string()
            }
        } else {
            entry.sni.trim().to_string()
        };
        let name = if entry.name.trim().is_empty() {
            kind.to_string()
        } else {
            entry.name.trim().to_string()
        };
        if let Some(card) = cfg.cards.iter_mut().find(|card| card.uuid == uuid) {
            card.name = name;
            card.card_type = kind.to_string();
            card.sni = sni;
        } else {
            cfg.cards.push(Card {
                name,
                uuid,
                card_type: kind.to_string(),
                sni,
                wg_private: String::new(),
                wg_addr: String::new(),
                registered: false,
            });
        }
        imported += 1;
    }
    if cfg.server_ip.is_empty() {
        cfg.server_ip = DEFAULT_HOST.to_string();
    }
    // Hysteria2 is the transport the voice helper adds for the voice networks.
    // It is one shared server password, so it travels with the profile rather
    // than inside a card. Only a plausible secret is accepted; a malformed
    // value must not overwrite a working one.
    let hy2 = profile_hy2.trim();
    if hy2.len() >= 16 && hy2.bytes().all(|b| b.is_ascii_alphanumeric()) {
        cfg.hy2_password = hy2.to_string();
    }
    let _ = std::fs::rename(&path, path.with_extension("json.imported"));
    Ok(imported)
}

/// Parse a pasted `qv1:` line plus, optionally, the matching `vless://` link.
/// The secret goes into the sealed store; the card goes into config.json.
/// `kind` comes from the UI, which owns the SNI catalogue.
pub fn import_into_config(text: &str, kind: Option<&str>) -> Result<String, String> {
    let parsed = qvreg::parse_paste(text)?;
    let uuid = parsed.credential.uuid.clone();
    store(&uuid, &parsed.credential.secret)?;

    let mut cfg = AppConfig::load();
    if cfg.server_ip.is_empty() {
        cfg.server_ip = DEFAULT_HOST.to_string();
    }
    let kind = kind.unwrap_or("Gamerz").to_string();
    if let Some(card) = cfg.cards.iter_mut().find(|card| card.uuid == uuid) {
        if let Some(name) = parsed.name {
            card.name = name;
        }
        if let Some(sni) = parsed.sni {
            card.sni = sni;
        }
        if card.sni.is_empty() {
            card.sni = "ea.com".into();
        }
    } else {
        cfg.cards.push(Card {
            name: parsed.name.unwrap_or_else(|| "Card".into()),
            uuid: uuid.clone(),
            card_type: kind,
            sni: parsed.sni.unwrap_or_else(|| "ea.com".into()),
            wg_private: String::new(),
            wg_addr: String::new(),
            registered: false,
        });
    }
    cfg.save();
    Ok(uuid)
}

/// Remove the plaintext per-device secret the SSH agent used to append. It is
/// dead on Windows now and should not sit in the profile.
pub fn remove_legacy_device_secret() {
    let _ = std::fs::remove_file(AppConfig::config_dir().join(LEGACY_DEVICE_SECRET));
}

/// Headless DPAPI round trip, for a build check without touching the server.
pub fn selftest() -> Result<(), String> {
    let plain = b"quotacards-dpapi-selftest-0123456789";
    let sealed = dpapi::protect(plain).ok_or("DPAPI refused to seal the test value")?;
    let opened = dpapi::unprotect(&sealed).ok_or("DPAPI refused to open the test value")?;
    if opened != plain {
        return Err("DPAPI returned a different value".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "04886278-28b2-42a1-ac66-186aa09e43e5";

    #[test]
    fn a_sealed_card_survives_an_empty_profile() {
        let mut cfg = AppConfig::default();
        assert_eq!(merge_known_cards(&mut cfg, vec![UUID.to_string()]), 1);
        assert_eq!(cfg.cards.len(), 1);
        assert_eq!(cfg.cards[0].uuid, UUID);
        assert_eq!(cfg.cards[0].card_type, "Gamerz");
        assert_eq!(cfg.cards[0].sni, "ea.com");
        assert!(!cfg.cards[0].registered);
    }

    #[test]
    fn restoring_twice_keeps_one_row_per_card() {
        let mut cfg = AppConfig::default();
        merge_known_cards(&mut cfg, vec![UUID.to_string()]);
        assert_eq!(merge_known_cards(&mut cfg, vec![UUID.to_string()]), 0);
        assert_eq!(cfg.cards.len(), 1);
    }

    #[test]
    fn a_live_profile_keeps_its_own_card_fields() {
        let mut cfg = AppConfig::default();
        cfg.cards.push(Card {
            name: "Streamerz".into(),
            uuid: UUID.to_string(),
            card_type: "Streamerz".into(),
            sni: "youtube.com".into(),
            wg_private: String::new(),
            wg_addr: String::new(),
            registered: true,
        });
        assert_eq!(merge_known_cards(&mut cfg, vec![UUID.to_string()]), 0);
        assert_eq!(cfg.cards[0].name, "Streamerz");
        assert_eq!(cfg.cards[0].sni, "youtube.com");
    }

    #[test]
    fn a_quota_space_password_round_trips_through_dpapi() {
        let plain = "0e597d25a1274188a3508d21";
        let sealed = seal_text(plain).expect("seal the space password");
        assert_ne!(sealed, plain);
        assert_eq!(open_text(&sealed).expect("open the space password"), plain);
    }
}
