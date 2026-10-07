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

# ================= lib.rs ====================================================
P = ROOT + r"\desktop\src-tauri\src\lib.rs"
t = load(P)

start = t.index("#[tauri::command]\nasync fn net_info() -> Option<NetInfo> {")
end = t.index("    None\n}\n", start) + len("    None\n}\n")
new_block = '''/// One provider's answer. The two JSON services carry the provider and the
/// place; Cloudflare's trace carries the bare address.
async fn net_from_ipwho(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client.get("https://ipwho.is/").send().await.ok()?;
    let v = r.json::<serde_json::Value>().await.ok()?;
    if v.get("success").and_then(|x| x.as_bool()) == Some(false) {
        return None;
    }
    let ip = v.get("ip").and_then(|x| x.as_str())?.to_string();
    if ip.is_empty() {
        return None;
    }
    let isp = v
        .pointer("/connection/isp")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/connection/org").and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string();
    let place = join_place(
        v.get("city").and_then(|x| x.as_str()),
        v.get("country").and_then(|x| x.as_str()),
    );
    Some(NetInfo { ip, isp, place })
}

async fn net_from_ipapi(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client.get("https://ipapi.co/json/").send().await.ok()?;
    let v = r.json::<serde_json::Value>().await.ok()?;
    let ip = v.get("ip").and_then(|x| x.as_str())?.to_string();
    if ip.is_empty() {
        return None;
    }
    let isp = v.get("org").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let place = join_place(
        v.get("city").and_then(|x| x.as_str()),
        v.get("country_name").and_then(|x| x.as_str()),
    );
    Some(NetInfo { ip, isp, place })
}

async fn net_from_cf(client: &reqwest::Client) -> Option<NetInfo> {
    let r = client.get("https://cloudflare.com/cdn-cgi/trace").send().await.ok()?;
    let body = r.text().await.ok()?;
    let ip = body.lines().find_map(|l| l.strip_prefix("ip="))?.trim().to_string();
    if ip.is_empty() {
        return None;
    }
    Some(NetInfo { ip, isp: String::new(), place: String::new() })
}

#[tauri::command]
async fn net_info() -> Option<NetInfo> {
    // The two JSON providers are raced against each other: whichever answers
    // first wins and a blocked or slow one simply hands the job over. Waiting
    // on them one after another is what used to leave this line lagging
    // seconds behind the rest of the window. Cloudflare backs both up with
    // the bare address.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("QuotaVPN")
        .build()
        .ok()?;
    tokio::select! {
        r = net_from_ipwho(&client) => r.or(net_from_ipapi(&client).await).or(net_from_cf(&client).await),
        r = net_from_ipapi(&client) => r.or(net_from_ipwho(&client).await).or(net_from_cf(&client).await),
    }
}

/// One app's icon as a base64 PNG, pulled straight from the executable's own
/// resources. An empty string means the shell had nothing readable for it and
/// the row falls back to a letter tile.
#[derive(serde::Serialize)]
struct AppIconOut {
    pkg: String,
    png: String,
}

#[tauri::command]
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
}

/// A small base64 so icon bytes can ride inside JSON without a new crate.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Read a GDI bitmap into raw bytes at the requested depth.
#[cfg(windows)]
unsafe fn read_dib(hbm: windows::Win32::Graphics::Gdi::HBITMAP, w: i32, h: i32, bpp: u16) -> Option<Vec<u8>> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    let mut bi = BITMAPINFO::default();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = w;
    bi.bmiHeader.biHeight = -h; // top-down rows
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = bpp;
    bi.bmiHeader.biCompression = BI_RGB;
    let row = ((w as usize * bpp as usize + 31) / 32) * 4;
    let mut buf = vec![0u8; row * h as usize];
    let dc = CreateCompatibleDC(None);
    if dc.0.is_null() {
        return None;
    }
    let lines = GetDIBits(dc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
    let _ = DeleteDC(dc);
    if lines == 0 {
        return None;
    }
    Some(buf)
}

/// The executable's own icon (the shell's large size, 32px) as PNG bytes.
#[cfg(windows)]
fn icon_png(path: &str) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HGDIOBJ};
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    unsafe {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut shfi = SHFILEINFOW::default();
        let got = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if got == 0 || shfi.hIcon.is_invalid() {
            return None;
        }
        let hicon = shfi.hIcon;
        let out = (|| {
            let mut ii = ICONINFO::default();
            GetIconInfo(hicon, &mut ii).ok()?;
            let res = (|| {
                let color = if !ii.hbmColor.0.is_null() { ii.hbmColor } else { ii.hbmMask };
                let mut bm = BITMAP::default();
                if GetObjectW(
                    HGDIOBJ(color.0),
                    std::mem::size_of::<BITMAP>() as i32,
                    Some(&mut bm as *mut _ as *mut _),
                ) == 0
                {
                    return None;
                }
                let (w, h) = (bm.bmWidth, bm.bmHeight);
                if w <= 0 || h <= 0 || w > 1024 || h > 1024 {
                    return None;
                }
                let mut px = read_dib(color, w, h, 32)?;
                if !px.chunks(4).any(|p| p[3] != 0) {
                    // Legacy icons keep transparency in the AND mask.
                    let mask = read_dib(ii.hbmMask, w, h, 1)?;
                    let row = ((w as usize + 31) / 32) * 4;
                    for y in 0..h as usize {
                        for x in 0..w as usize {
                            let bit = (mask[y * row + x / 8] >> (7 - (x % 8))) & 1;
                            let p = &mut px[(y * w as usize + x) * 4..][..4];
                            if bit == 1 {
                                p[0] = 0;
                                p[1] = 0;
                                p[2] = 0;
                                p[3] = 0;
                            } else {
                                p[3] = 255;
                            }
                        }
                    }
                }
                for p in px.chunks_mut(4) {
                    p.swap(0, 2); // BGRA -> RGBA
                }
                let img = image::RgbaImage::from_raw(w as u32, h as u32, px)?;
                let mut out = std::io::Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(img)
                    .write_to(&mut out, image::ImageFormat::Png)
                    .ok()?;
                Some(out.into_inner())
            })();
            if !ii.hbmColor.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
            }
            if !ii.hbmMask.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
            }
            res
        })();
        let _ = DestroyIcon(hicon);
        out
    }
}

#[cfg(not(windows))]
fn icon_png(_path: &str) -> Option<Vec<u8>> {
    None
}
'''
t = t[:start] + new_block + t[end:]

t = sub(t, "            net_info,\n            speed_servers,", "            net_info,\n            app_icons,\n            speed_servers,")
save(P, t)
print("lib.rs ok")

# ================= Cargo.toml ================================================
P = ROOT + r"\desktop\src-tauri\Cargo.toml"
t = load(P)
t = sub(t,
        'windows = { version = "0.62", features = ["Win32_NetworkManagement_IpHelper", "Win32_NetworkManagement_Ndis", "Win32_Foundation", "Win32_System_Diagnostics_ToolHelp", "Win32_System_Registry"] }',
        'windows = { version = "0.62", features = ["Win32_NetworkManagement_IpHelper", "Win32_NetworkManagement_Ndis", "Win32_Foundation", "Win32_System_Diagnostics_ToolHelp", "Win32_System_Registry", "Win32_UI_Shell", "Win32_UI_WindowsAndMessaging", "Win32_Graphics_Gdi", "Win32_Storage_FileSystem"] }')
save(P, t)
print("cargo ok")

# ================= index.css =================================================
P = ROOT + r"\desktop\ui-next\src\index.css"
t = load(P)
t = sub(t, '''  body {
    @apply text-foreground antialiased;
    background: var(--bg);
    margin: 0;
    font-size: 13.5px;
    line-height: 1.45;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
  }''',
        '''  body {
    @apply text-foreground antialiased;
    background: var(--bg);
    margin: 0;
    font-size: 13.5px;
    line-height: 1.45;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    /* The app is a product surface, not a document: text does not select by
       accident. Fields and the few values people copy stay selectable. */
    -webkit-user-select: none;
    user-select: none;
  }
  input,
  textarea,
  [contenteditable="true"],
  .select-text {
    -webkit-user-select: text;
    user-select: text;
  }''')
save(P, t)
print("css ok")
