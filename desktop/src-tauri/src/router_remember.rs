//! Remembering the router password the way Windows remembers passwords.
//!
//! The password is sealed with DPAPI under the current Windows account, so the
//! file is useless to anyone else and to any other machine. It is the same
//! protection the browser's saved passwords get. Signing in once is enough;
//! the app reconnects by itself from then on.

#[cfg(windows)]
use windows::Win32::Foundation::LocalFree;
#[cfg(windows)]
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

fn path() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::from("."));
    std::path::Path::new(&base)
        .join("quotavpn")
        .join("router_secret.bin")
}

#[cfg(windows)]
fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, None, None, None, None, 0, &mut out).ok()?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(out.pbData as *mut core::ffi::c_void);
        Some(bytes)
    }
}

#[cfg(windows)]
fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: sealed.len() as u32,
            pbData: sealed.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, 0, &mut out).ok()?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(out.pbData as *mut core::ffi::c_void);
        Some(bytes)
    }
}

#[cfg(not(windows))]
fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    Some(plain.to_vec())
}

#[cfg(not(windows))]
fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    Some(sealed.to_vec())
}

/// Keep the router login for next time.
pub fn save(user: &str, pass: &str) -> bool {
    let plain = format!("{user}\n{pass}");
    let Some(sealed) = protect(plain.as_bytes()) else {
        return false;
    };
    let target = path();
    if let Some(dir) = target.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(target, sealed).is_ok()
}

/// The saved login, if this account can still open it.
pub fn load() -> Option<(String, String)> {
    let sealed = std::fs::read(path()).ok()?;
    let plain = unprotect(&sealed)?;
    let text = String::from_utf8(plain).ok()?;
    let mut lines = text.splitn(2, '\n');
    let user = lines.next()?.to_string();
    let pass = lines.next()?.to_string();
    if user.is_empty() {
        return None;
    }
    Some((user, pass))
}

pub fn clear() -> bool {
    std::fs::remove_file(path()).is_ok()
}

pub fn saved() -> bool {
    path().exists()
}