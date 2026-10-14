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

fn client() -> Result<reqwest::Client, String> {
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

const WEB_HEADERS: [(&str, &str); 9] = [
    ("channelid", "702"),
    ("csrftoken", ""),
    ("languagecode", "en-US"),
    ("isselfcare", "true"),
    ("delegatorsubsid", ""),
    ("iscoporate", "false"),
    ("ismobile", "false"),
    ("Origin", "https://my.te.eg"),
    ("Referer", "https://my.te.eg/echannel/"),
];

#[derive(serde::Serialize)]
pub struct ChallengeOut {
    interaction: bool,
    image: String,
}

#[tauri::command]
pub async fn we_quota_challenge(
    state: tauri::State<'_, WeQuota>,
    identifier: String,
) -> Result<ChallengeOut, String> {
    let client = client()?;
    let mut jar = HashMap::new();
    let cap = post(
        &client,
        CAPTCHA,
        serde_json::json!({"merchantName": "E-Care", "serviceName": "Login", "identifier": identifier}),
        &mut jar,
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
    *state.0.lock().map_err(|e| e.to_string())? = Some(WeSession {
        cookies: jar,
        csrf: String::new(),
        sub_id: String::new(),
        customer: String::new(),
        token,
    });
    Ok(ChallengeOut { interaction, image })
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
    let acct = format!("FBB{}", landline.trim_start_matches('0'));
    let (mut jar, token) = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(s) => (s.cookies.clone(), s.token.clone()),
            None => return Err("no challenge: ask for a code first".to_string()),
        }
    };
    let auth = post(
        &client,
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
        &mut jar,
        &WEB_HEADERS,
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
pub struct QuotaUnit {
    offer: String,
    total: f64,
    used: f64,
    remain: f64,
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
    let mut headers = WEB_HEADERS.to_vec();
    let csrf_owned = csrf.clone();
    headers.push(("csrftoken", csrf_owned.as_str()));
    let offers = post(
        &client,
        &format!("{ECHANNEL}/cz/v1/auth/getSubscribedOfferings"),
        serde_json::json!({"msisdn": acct, "numberServiceType": "FBB", "groupId": ""}),
        &mut jar,
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
    for offer in list {
        let Some(offer_id) = offer.get("mainOfferingId").cloned() else {
            continue;
        };
        let quota = post(
            &client,
            &format!("{ECHANNEL}/cz/cbs/bb/queryFreeUnit"),
            serde_json::json!({"subscriberId": sub_id, "mainOfferId": offer_id}),
            &mut jar,
            &headers,
        )
        .await?;
        if ret_code(&quota) != "0" {
            continue;
        }
        let units = quota["body"].as_array().cloned().unwrap_or_default();
        for u in units {
            let num = |k: &str| u.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
            out.push(QuotaUnit {
                offer: u.get("offerName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                total: num("total"),
                used: num("used"),
                remain: num("remain"),
            });
        }
    }
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
