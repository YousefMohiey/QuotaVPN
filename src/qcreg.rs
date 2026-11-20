//! Registration over HTTPS with a per-device secret, no SSH anywhere.
//!
//! The device secret never leaves the machine in the clear and no shared
//! credential is compiled in: each installation generates its own 32-byte
//! secret, protects it with DPAPI, and signs `op\uuid\tts\nonce` with HMAC-SHA256.
//! The server stores only the secret's owner mapping, so a client can only ever
//! act on its own card.
//!
//! Transport is `curl.exe` (present on every Windows 10/11), which keeps this
//! module free of an HTTP stack while still speaking real TLS to the endpoint.

use std::path::PathBuf;

const ENDPOINT: &str = "https://quotacards.duckdns.org:8444/v1/register";

pub struct Device {
    pub uuid: String,
    secret: Vec<u8>,
}

fn secret_path() -> PathBuf {
    crate::config::AppConfig::config_dir().join("device_hmac.bin")
}

#[cfg(windows)]
mod dpapi {
    // CryptProtectData / CryptUnprotectData, current user scope.
    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const Blob,
            desc: *const u16,
            entropy: *const Blob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            data_out: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const Blob,
            desc: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            data_out: *mut Blob,
        ) -> i32;
        fn LocalFree(p: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    #[repr(C)]
    pub struct Blob {
        pub len: u32,
        pub data: *mut u8,
    }
    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let mut out = Blob { len: 0, data: std::ptr::null_mut() };
            let inp = Blob { len: plain.len() as u32, data: plain.as_ptr() as *mut u8 };
            if CryptProtectData(&inp, std::ptr::null(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut out) == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
            LocalFree(out.data as *mut std::ffi::c_void);
            Some(v)
        }
    }
    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let mut out = Blob { len: 0, data: std::ptr::null_mut() };
            let inp = Blob { len: sealed.len() as u32, data: sealed.as_ptr() as *mut u8 };
            if CryptUnprotectData(&inp, std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut out) == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
            LocalFree(out.data as *mut std::ffi::c_void);
            Some(v)
        }
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect(p: &[u8]) -> Option<Vec<u8>> { Some(p.to_vec()) }
    pub fn unprotect(p: &[u8]) -> Option<Vec<u8>> { Some(p.to_vec()) }
}

fn random_secret() -> Vec<u8> {
    let mut buf = [0u8; 32];
    // rand_core is already a dependency of this crate.
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut buf);
    buf.to_vec()
}

/// Load this installation's secret, creating and sealing one on first use.
pub fn load_or_make_secret() -> Result<Vec<u8>, String> {
    let path = secret_path();
    if let Ok(sealed) = std::fs::read(&path) {
        return dpapi::unprotect(&sealed).ok_or_else(|| "device secret could not be opened".to_string());
    }
    let secret = random_secret();
    let sealed = dpapi::protect(&secret).ok_or_else(|| "device secret could not be sealed".to_string())?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, &sealed).map_err(|e| format!("write device secret: {e}"))?;
    Ok(secret)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sign(secret: &[u8], op: &str, uuid: &str, ts: i64, nonce: &str) -> String {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
    let msg = format!("{op}\n{uuid}\n{ts}\n{nonce}");
    hex(ring::hmac::sign(&key, msg.as_bytes()).as_ref())
}

fn nonce() -> String {
    let mut buf = [0u8; 12];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut buf);
    hex(&buf)
}

fn post(body: &str) -> Result<(u16, String), String> {
    let out = crate::lan::hidden("curl")
        .args([
            "-sS", "-k", "-X", "POST", ENDPOINT,
            "-H", "content-type: application/json",
            "--max-time", "20",
            "-d", body,
            "-w", "\n%{http_code}",
        ])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut lines = text.trim_end().rsplitn(2, '\n');
    let code = lines.next().unwrap_or("0").trim().parse::<u16>().unwrap_or(0);
    let payload = lines.next().unwrap_or("").to_string();
    Ok((code, payload))
}

/// Register or revoke one card, or ask for its state. Returns the server's
/// sentence on failure so the window can show a real reason.
pub fn call(op: &str, uuid: &str) -> Result<String, String> {
    let secret = load_or_make_secret()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let n = nonce();
    let sig = sign(&secret, op, uuid, ts, &n);
    let body = format!(
        r#"{{"op":"{op}","uuid":"{uuid}","ts":{ts},"nonce":"{n}","sig":"{sig}"}}"#
    );
    let (code, payload) = post(&body)?;
    match code {
        200 => Ok(payload),
        401 => Err("the server rejected the request time; check the clock".into()),
        403 => Err("this device is not authorised to manage that card".into()),
        404 => Err("that card is not registered yet".into()),
        409 => Err("duplicate request refused; try again".into()),
        429 => Err("too many attempts; wait a minute".into()),
        502 => Err("the VPN server did not apply the change".into()),
        other => Err(format!("registration server answered {other}: {payload}")),
    }
}
