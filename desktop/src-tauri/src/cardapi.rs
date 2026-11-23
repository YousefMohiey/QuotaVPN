//! HTTPS client for the card management API.
//!
//! The secret stays in this process and is never part of a command line or a
//! log. The body carries the UUID, timestamp, nonce and signature, all of which
//! the server re-derives before it changes anything.

use quotacards::qvreg;
use std::time::Duration;

pub async fn call(op: &str, uuid: &str) -> Result<String, String> {
    call_with_timeout(op, uuid, 20).await
}

/// A status check is a UI action and must not hold the row for twenty seconds
/// on a network that drops the port. Registration and revocation keep the
/// longer budget because they change the server.
pub async fn call_with_timeout(op: &str, uuid: &str, seconds: u64) -> Result<String, String> {
    let secret = crate::credentials::secret_for(uuid)?;
    let ts = qvreg::now_ts();
    let nonce = qvreg::nonce();
    let sig = qvreg::sign(&secret, op, uuid, ts, &nonce);
    let url = qvreg::url_for(qvreg::DEFAULT_API_BASE, op)?;
    let body = qvreg::request_body(op, uuid, ts, &nonce, &sig);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(seconds))
        .build()
        .map_err(|e| format!("card client: {e}"))?;
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("could not reach the card server: {e}"))?;
    let code = response.status().as_u16();
    let payload = response.text().await.unwrap_or_default();
    qvreg::map_response(code, &payload)
}

/// Blocking wrapper for the headless flags, which run before Tauri starts.
pub fn call_blocking(op: &str, uuid: &str) -> Result<String, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("card runtime: {e}"))?;
    runtime.block_on(call(op, uuid))
}
