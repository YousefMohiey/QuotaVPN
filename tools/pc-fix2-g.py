import io

ROOT = r"C:\Tools\QuotaCards"

def load(p):
    return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:4000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))

def sub(t, old, new, n=1):
    assert t.count(old) == n, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, n)

# ---- Cargo: the threading API for resolving process paths ----
P = ROOT + r"\desktop\src-tauri\Cargo.toml"
t = load(P)
t = sub(t, '"Win32_System_Registry", "Win32_UI_Shell"',
        '"Win32_System_Registry", "Win32_System_Threading", "Win32_System_Com", "Win32_UI_Shell"')
save(P, t)
print("cargo threading ok")

# ---- lib.rs: app_icons resolves names to paths first ----
P = ROOT + r"\desktop\src-tauri\src\lib.rs"
t = load(P)

t = sub(t, '''#[tauri::command]
async fn app_icons(paths: Vec<String>) -> Vec<AppIconOut> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .into_iter()
            .map(|pkg| {
                let png = icon_png(&pkg).map(|bytes| b64(&bytes)).unwrap_or_default();
                AppIconOut { pkg, png }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}''',
        '''#[tauri::command]
async fn app_icons(pkgs: Vec<String>) -> Vec<AppIconOut> {
    tauri::async_runtime::spawn_blocking(move || {
        // The routing list carries process names; the icons live on disk, so
        // resolve every name to its executable's full path first. A name we
        // cannot reach (elevated or protected) simply yields no icon.
        let paths = process_paths();
        pkgs.into_iter()
            .map(|pkg| {
                let path = if pkg.contains('\\\\') || pkg.contains('/') {
                    pkg.clone()
                } else {
                    paths.get(&pkg.to_ascii_lowercase()).cloned().unwrap_or_default()
                };
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
                    .trim_end_matches('\\0')
                    .to_ascii_lowercase();
                if !name.is_empty() && !map.contains_key(&name) {
                    if let Ok(h) =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, entry.th32ProcessID)
                    {
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
}''')

t = sub(t, '''        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut shfi = SHFILEINFOW::default();''',
        '''        // The shell helpers want COM up on the thread that calls them.
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        );
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut shfi = SHFILEINFOW::default();''')
save(P, t)
print("lib.rs icons ok")

# ---- ipc.ts: the arg key follows the Rust param name ----
P = ROOT + r"\desktop\ui-next\src\lib\ipc.ts"
t = load(P)
t = sub(t, 'export const appIcons = (paths: string[]): Promise<AppIcon[]> => call<AppIcon[]>("app_icons", { paths })',
        'export const appIcons = (pkgs: string[]): Promise<AppIcon[]> => call<AppIcon[]>("app_icons", { pkgs })')
save(P, t)
print("ipc key ok")
