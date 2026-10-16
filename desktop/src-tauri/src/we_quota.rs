//! WE (Telecom Egypt) quota readout for the Home quota card.
//!
//! The same calls the MyWE site makes: captcha token, password login,
//! subscribed offers, free units. Cookies live in a Rust-side session;
//! the password only passes through per call and is never stored.

use std::collections::HashMap;
use std::sync::Mutex;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";
const ECHANNEL: &str = "https://my.te.eg/echannel/service/besapp/base/rest/busiservice";
const CAPTCHA: &str = "https://captcha.te.eg/api/Captcha/GenerateCaptcha";

struct WeSession {
    cookies: HashMap<String, String>,
    csrf: String,
    sub_id: String,
    customer: String,
    token: String,
}

pub struct WeQuota(Mutex<Option<WeSession>>);

impl WeQuota {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(UA)
        .build()
        .map_err(|e| e.to_string())
}

fn cookie_header(jar: &HashMap<String, String>) -> String {
    jar.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn harvest(jar: &mut HashMap<String, String>, headers: &reqwest::header::HeaderMap) {
    for raw in headers.get_all(reqwest::header::SET_COOKIE) {
        if let Ok(s) = raw.to_str() {
            let pair = s.split(';').next().unwrap_or("").trim();
            if let Some((k, v)) = pair.split_once('=') {
                jar.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
}

async fn post(
    client: &reqwest::Client,
    url: &str,
    payload: serde_json::Value,
    jar: &mut HashMap<String, String>,
    extra: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    // No Origin/Referer here: captcha.te.eg answers InvalidMerchantRequest
    // when they are present, while the echannel calls below need them.
    let mut req = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    if !jar.is_empty() {
        req = req.header(reqwest::header::COOKIE, cookie_header(jar));
    }
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let resp = req
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("NET:{e}"))?;
    harvest(jar, resp.headers());
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("NET:{e}"))
}

fn ret_code(v: &serde_json::Value) -> String {
    v.get("header")
        .and_then(|h| h.get("retCode"))
        .and_then(|c| c.as_str())
        .unwrap_or("?")
        .to_string()
}

const WEB_BASE: [(&str, &str); 8] = [
    ("channelid", "702"),
    ("languagecode", "en-US"),
    ("isselfcare", "true"),
    ("delegatorsubsid", ""),
    ("iscoporate", "false"),
    ("ismobile", "false"),
    ("Origin", "https://my.te.eg"),
    ("Referer", "https://my.te.eg/echannel/"),
];

/// One csrftoken header, never two: the server reads the first and an
/// empty duplicate used to shadow the real login token on usage calls.
fn web_headers(csrf: &str) -> Vec<(&str, &str)> {
    let mut h = WEB_BASE.to_vec();
    h.push(("csrftoken", csrf));
    h
}

#[derive(serde::Serialize)]
pub struct ChallengeOut {
    pub interaction: bool,
    pub image: String,
}

fn fnum(v: &serde_json::Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_u64().map(|n| n as f64))
        .or_else(|| v.as_i64().map(|n| n as f64))
        .unwrap_or(0.0)
}

pub async fn challenge_inner(
    client: &reqwest::Client,
    jar: &mut HashMap<String, String>,
    identifier: &str,
) -> Result<(ChallengeOut, String), String> {
    let cap = post(
        client,
        CAPTCHA,
        serde_json::json!({"merchantName": "E-Care", "serviceName": "Login", "identifier": identifier}),
        jar,
        &[],
    )
    .await?;
    if cap.get("status").and_then(|s| s.as_str()) != Some("Success") {
        let status = cap
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("no-answer");
        return Err(format!("check refused {status}"));
    }
    let interaction = cap.get("requireInteraction").and_then(|b| b.as_bool()).unwrap_or(false);
    let image = if interaction {
        cap.get("captcha").and_then(|c| c.as_str()).unwrap_or("").to_string()
    } else {
        String::new()
    };
    let token = cap.get("token").and_then(|t| t.as_str()).unwrap_or("").to_string();
    Ok((ChallengeOut { interaction, image }, token))
}

pub async fn login_inner(
    client: &reqwest::Client,
    jar: &mut HashMap<String, String>,
    token: &str,
    landline: &str,
    password: &str,
    code: &str,
) -> Result<(String, String, String), String> {
    let acct = format!("FBB{}", landline.trim_start_matches('0'));
    let auth = post(
        client,
        &format!("{ECHANNEL}/v1/auth/userAuthenticate"),
        serde_json::json!({
            "acctId": acct,
            "password": password,
            "imgCacheKey": token,
            "appLocale": "en-US",
            "isSelfcare": "Y",
            "isMobile": "N",
            "imgCode": code,
            "isConvergent": "0",
        }),
        jar,
        &web_headers(""),
    )
    .await?;
    if ret_code(&auth) != "0" {
        return Err("login refused".to_string());
    }
    let body = &auth["body"];
    let csrf = body.get("token").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let sub_id = body
        .get("subscriber")
        .and_then(|s| s.get("subscriberId"))
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let customer = body
        .get("customer")
        .and_then(|c| c.get("custName"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    if csrf.is_empty() || sub_id.is_empty() {
        return Err("login refused".to_string());
    }
    Ok((customer, csrf, sub_id))
}

pub async fn usage_inner(
    client: &reqwest::Client,
    jar: &mut HashMap<String, String>,
    csrf: &str,
    sub_id: &str,
    acct: &str,
) -> Result<Vec<QuotaUnit>, String> {
    let headers = web_headers(csrf);
    let offers = post(
        client,
        &format!("{ECHANNEL}/cz/v1/auth/getSubscribedOfferings"),
        serde_json::json!({"msisdn": acct, "numberServiceType": "FBB", "groupId": ""}),
        jar,
        &headers,
    )
    .await?;
    if ret_code(&offers) != "0" {
        return Err("offers refused".to_string());
    }
    let list = offers["body"]["offeringList"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut seen_offers: Vec<String> = Vec::new();
    for offer in list {
        let Some(offer_id) = offer.get("mainOfferingId").cloned() else {
            continue;
        };
        // Several rows share one main offering id: read it once.
        let id_key = offer_id.as_str().unwrap_or_default().to_string();
        if id_key.is_empty() || seen_offers.iter().any(|s| s == &id_key) {
            continue;
        }
        seen_offers.push(id_key);
        let quota = post(
            client,
            &format!("{ECHANNEL}/cz/cbs/bb/queryFreeUnit"),
            serde_json::json!({"subscriberId": sub_id, "mainOfferId": offer_id}),
            jar,
            &headers,
        )
        .await?;
        if ret_code(&quota) != "0" {
            continue;
        }
        let units = quota["body"].as_array().cloned().unwrap_or_default();
        for u in units {
            let mut buckets = u
                .get("freeUnitBeanDetailList")
                .and_then(|b| b.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|b| QuotaBucket {
                    label: b
                        .get("originType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    item: b
                        .get("itemCode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    total: fnum(&b["initialAmount"]),
                    remain: fnum(&b["currentAmount"]),
                })
                .filter(|b| b.total > 0.0)
                .collect::<Vec<_>>();
            // Read main first, then gaming, then streaming, then the rest.
            buckets.sort_by_key(|b| {
                let s = b.item.to_lowercase();
                if s.contains("gaming") {
                    1
                } else if s.contains("racing") || s.contains("stream") {
                    2
                } else if s.contains("_data") {
                    0
                } else {
                    3
                }
            });
            let unit = QuotaUnit {
                offer: u.get("offerName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                total: fnum(&u["total"]),
                used: fnum(&u["used"]),
                remain: fnum(&u["remain"]),
                buckets,
            };
            // One line sells several offers sharing the same bucket: show it once.
            if !out.iter().any(|o: &QuotaUnit| {
                o.offer == unit.offer && o.total == unit.total && o.remain == unit.remain
            }) {
                out.push(unit);
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn we_quota_challenge(
    state: tauri::State<'_, WeQuota>,
    identifier: String,
) -> Result<ChallengeOut, String> {
    let client = client()?;
    let mut jar = HashMap::new();
    let (out, token) = challenge_inner(&client, &mut jar, &identifier).await?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(WeSession {
        cookies: jar,
        csrf: String::new(),
        sub_id: String::new(),
        customer: String::new(),
        token,
    });
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct LoginOut {
    customer: String,
}

#[tauri::command]
pub async fn we_quota_login(
    state: tauri::State<'_, WeQuota>,
    landline: String,
    password: String,
    code: String,
) -> Result<LoginOut, String> {
    let client = client()?;
    let (mut jar, token) = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(s) => (s.cookies.clone(), s.token.clone()),
            None => return Err("no challenge: ask for a code first".to_string()),
        }
    };
    let (customer, csrf, sub_id) =
        login_inner(&client, &mut jar, &token, &landline, &password, &code).await?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(WeSession {
        cookies: jar,
        csrf,
        sub_id,
        customer: customer.clone(),
        token: String::new(),
    });
    Ok(LoginOut { customer })
}

#[derive(serde::Serialize)]
pub struct QuotaBucket {
    pub label: String,
    pub item: String,
    pub total: f64,
    pub remain: f64,
}

#[derive(serde::Serialize)]
pub struct QuotaUnit {
    pub offer: String,
    pub total: f64,
    pub used: f64,
    pub remain: f64,
    /// The package split: main, gaming, streaming, gift. The top-level
    /// numbers above are their sum; the detail list is where the split lives.
    pub buckets: Vec<QuotaBucket>,
}

#[tauri::command]
pub async fn we_quota_usage(state: tauri::State<'_, WeQuota>, acct: String) -> Result<Vec<QuotaUnit>, String> {
    let client = client()?;
    let (mut jar, csrf, sub_id) = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(s) if !s.csrf.is_empty() => (s.cookies.clone(), s.csrf.clone(), s.sub_id.clone()),
            _ => return Err("not logged in".to_string()),
        }
    };
    let out = usage_inner(&client, &mut jar, &csrf, &sub_id, &acct).await?;
    {
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(s) = guard.as_mut() {
            s.cookies = jar;
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn we_quota_logout(state: tauri::State<'_, WeQuota>) {
    *state.0.lock().expect("quota session poisoned") = None;
}

/// The `--we-probe` entry point: same inner flow the UI drives, headless.
pub fn probe_main() {
    let number = std::env::var("QC_WE_NUMBER").unwrap_or_default();
    let password = std::env::var("QC_WE_PASS").unwrap_or_default();
    if number.trim().is_empty() || password.is_empty() {
        eprintln!("probe needs QC_WE_NUMBER and QC_WE_PASS in the environment");
        std::process::exit(2);
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("probe runtime");
    // Optional code on the command line for non-interactive runs.
    let arg_code = std::env::args()
        .skip_while(|a| a != "--we-probe")
        .nth(1)
        .unwrap_or_default();
    rt.block_on(async {
        let client = client().expect("client builds");
        let mut jar = HashMap::new();
        let (ch, token) = challenge_inner(&client, &mut jar, number.trim())
            .await
            .expect("challenge answers Success");
        let code;
        if ch.interaction {
            use std::io::BufRead;
            let path = std::env::var("TEMP")
                .unwrap_or_else(|_| "C:/Windows/Temp".to_string())
                + "/we-probe-captcha.jpg";
            let raw = ch.image.split_once(',').map(|(_, b)| b).unwrap_or("");
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw)
                .expect("captcha decodes");
            std::fs::write(&path, bytes).expect("captcha saves");
            eprintln!("PROBE_NEEDS_CODE {path}");
            code = if !arg_code.is_empty() {
                arg_code.clone()
            } else {
                eprintln!("type the letters from the image and press Enter:");
                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line).expect("stdin");
                line.trim().to_string()
            };
        } else {
            println!("challenge: Success, no code read");
            code = String::new();
        }
        let (customer, csrf, sub_id) = login_inner(
            &client,
            &mut jar,
            &token,
            number.trim(),
            &password,
            &code,
        )
        .await
        .expect("login accepts the number and password");
        assert!(!customer.is_empty(), "login returns a customer name");
        println!("login: ok");
        let acct = format!("FBB{}", number.trim().trim_start_matches('0'));
        let units = usage_inner(&client, &mut jar, &csrf, &sub_id, &acct)
            .await
            .expect("offers and free units read clean");
        assert!(!units.is_empty(), "at least one quota unit comes back");
        for u in &units {
            println!("UNIT {} remain {} / total {}", u.offer, u.remain, u.total);
            for b in &u.buckets {
                println!(
                    "  BUCKET {} [{}] remain {} / total {}",
                    b.label, b.item, b.remain, b.total
                );
            }
        }
        println!("PROBE_GREEN");
    });
}

#[cfg(test)]
mod tests {
    // NOTE: `cargo test` harness binary does not start on this build box
    // (STATUS_ENTRYPOINT_NOT_FOUND), so the shippable gate is the example
    // binary instead: cargo run --example we_probe with QC_WE_NUMBER and
    // QC_WE_PASS in the environment.
    use super::*;

    /// Creds come from the environment, never the repo: without them the
    /// live test skips instead of failing.
    fn creds() -> Option<(String, String)> {
        match (std::env::var("QC_WE_NUMBER"), std::env::var("QC_WE_PASS")) {
            (Ok(n), Ok(p)) if !n.trim().is_empty() && !p.is_empty() => Some((n, p)),
            _ => None,
        }
    }

    /// The exact flow the app runs, against the real WE servers. This is
    /// the gate: it must be green before any build ships to the owner.
    #[tokio::test]
    async fn live_quota_flow() {
        let Some((number, password)) = creds() else {
            eprintln!("SKIP live_quota_flow: set QC_WE_NUMBER and QC_WE_PASS");
            return;
        };
        let client = client().expect("client builds");
        let mut jar = HashMap::new();
        let (ch, token) = challenge_inner(&client, &mut jar, number.trim())
            .await
            .expect("challenge answers Success");
        if ch.interaction {
            eprintln!("SKIP live_quota_flow: server wants a code read right now");
            return;
        }
        let (customer, csrf, sub_id) =
            login_inner(&client, &mut jar, &token, number.trim(), &password, "")
                .await
                .expect("login accepts the number and password");
        assert!(!customer.is_empty(), "login returns a customer name");
        let acct = format!("FBB{}", number.trim().trim_start_matches('0'));
        let units = usage_inner(&client, &mut jar, &csrf, &sub_id, &acct)
            .await
            .expect("offers and free units read clean");
        assert!(!units.is_empty(), "at least one quota unit comes back");
        assert!(
            units.iter().any(|u| !u.buckets.is_empty()),
            "the package split (main / gaming / streaming) comes back"
        );
        for u in &units {
            assert!(u.remain <= u.total, "remain never exceeds total");
            eprintln!("UNIT {} remain {} / total {}", u.offer, u.remain, u.total);
            for b in &u.buckets {
                eprintln!(
                    "  BUCKET {} [{}] remain {} / total {}",
                    b.label, b.item, b.remain, b.total
                );
            }
        }
    }
}
