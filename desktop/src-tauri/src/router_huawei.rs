//! Names straight from the router.
//!
//! Every other source on a phone network is a dead end: the phone randomises
//! its hardware address, ignores NetBIOS, and its request for an address never
//! reaches a wired PC. The router knows all of it, because it handed out the
//! addresses. This speaks the web interface a Huawei ONT serves: a token, a
//! login, a session token, then the DHCP table as JavaScript.

use std::collections::HashMap;

#[derive(Clone, serde::Serialize)]
pub struct RouterClient {
    pub name: String,
    pub ip: String,
    pub mac: String,
}

pub struct Session {
    client: reqwest::Client,
    host: String,
    cookie: String,
    token: String,
}

fn b64(input: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::new();
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

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        // The management page is served with its own certificate, which no
        // authority signs. The connection never leaves the local network.
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())
}

fn cookie_from(resp: &reqwest::Response) -> String {
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| v.split(';').next())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Log in and keep the session, or say plainly that it did not work.
pub async fn login(host: &str, user: &str, pass: &str) -> Result<Session, String> {
    let client = client()?;
    let base = format!("https://{host}");

    let pre = client
        .get(format!("{base}/html/ssmp/common/getRandString.asp"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let pre = pre.trim().to_string();
    log(&format!("pre-token len={} html={}", pre.len(), pre.contains('<')));
    if pre.len() < 8 || pre.contains('<') {
        return Err("no token".into());
    }

    let form = [
        ("UserName", user.to_string()),
        ("PassWord", b64(pass)),
        ("Language", "english".to_string()),
        ("x.X_HW_Token", pre.clone()),
    ];
    let login = client
        .post(format!("{base}/login.cgi"))
        .header(reqwest::header::REFERER, format!("{base}/"))
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let login_status = login.status().as_u16();
    let cookie = cookie_from(&login);

    let token = client
        .get(format!("{base}/html/ssmp/common/GetRandToken.asp"))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let token = token.trim().trim_start_matches('\u{feff}').to_string();
    log(&format!(
        "login status={} cookie_len={} token_len={} html={}",
        login_status,
        cookie.len(),
        token.len(),
        token.contains('<')
    ));
    if token.is_empty() || token.contains('<') {
        return Err("login rejected".into());
    }

    Ok(Session {
        client,
        host: base,
        cookie,
        token,
    })
}

/// Pull one quoted argument list out of `new DHCPInfo(...)`.
fn fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut inside = false;
    let mut escaped = false;
    for ch in line.chars() {
        if !inside {
            if ch == '"' {
                inside = true;
                cur.clear();
            }
            continue;
        }
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => {
                out.push(cur.clone());
                inside = false;
            }
            _ => cur.push(ch),
        }
    }
    out
}

/// A small log of what the router said, so a failed sign in can be diagnosed
/// without asking anyone for their password. Never contains credentials.
fn log(line: &str) {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::from("."));
    let dir = std::path::Path::new(&base).join("quotavpn");
    let _ = std::fs::create_dir_all(&dir);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("router.log"))
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{stamp} {line}")
        });
}

impl Session {
    /// The router's own list: every device it handed an address to.
    pub async fn clients(&self) -> Result<Vec<RouterClient>, String> {
        // Routers in this family do not agree on the page name, so ask the
        // likely ones and keep the first that answers with a device list.
        const PAGES: [&str; 5] = [
            "/html/bbsp/common/GetLanUserDhcpInfo.asp",
            "/html/bbsp/lanhostinfo/lanhostinfo.asp",
            "/html/bbsp/common/lanuserinfo.asp",
            "/html/bbsp/common/GetHostInfo.asp",
            "/html/amp/ethinfo/ethinfo.asp",
        ];
        let mut body = String::new();
        for page in PAGES {
            match self
                .client
                .post(format!("{}{page}", self.host))
                .header(reqwest::header::COOKIE, self.cookie.clone())
                .form(&[("x.X_HW_Token", self.token.clone())])
                .send()
                .await
            {
                Ok(r) => {
                    let status = r.status().as_u16();
                    let text = r.text().await.unwrap_or_default();
                    log(&format!("page {page} status={status} len={}", text.len()));
                    if text.contains("DHCPInfo") || text.contains("HostInfo") {
                        body = text;
                        break;
                    }
                }
                Err(e) => log(&format!("page {page} error={e}")),
            }
        }

        let mut out = Vec::new();
        for piece in body.split("new DHCPInfo(").skip(1) {
            let line: String = piece.chars().take_while(|c| *c != ')').collect();
            let a = fields(&line);
            if a.len() < 4 {
                continue;
            }
            let name = a[1].trim().to_string();
            let ip = a[2].trim().to_string();
            let mac = a[3].trim().replace('-', ":").to_lowercase();
            if ip.is_empty() {
                continue;
            }
            out.push(RouterClient { name, ip, mac });
        }
        log(&format!("clients parsed={} body_len={}", out.len(), body.len()));
        // The raw list, so a field that moves between firmware versions can be
        // seen instead of guessed. It holds names, addresses and nothing else.
        let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::from("."));
        let _ = std::fs::write(
            std::path::Path::new(&base).join("quotavpn").join("router_dhcp.txt"),
            &body,
        );
        for c in &out {
            log(&format!("client name={:?} ip={} mac={}", c.name, c.ip, c.mac));
        }
        Ok(out)
    }
}

/// Ask a router for its list, and keep it keyed by address for the scan.
pub async fn names_by_ip(session: &Session) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(list) = session.clients().await {
        for c in list {
            if !c.name.is_empty() {
                map.insert(c.ip, c.name);
            }
        }
    }
    map
}
