//! Fetch probe: the same reqwest the app uses, outside the app, printing the
//! FULL error chain. Run from desktop/src-tauri:
//!   cargo run --example fetchprobe -- <url> [...]
fn main() {
    let urls: Vec<String> = std::env::args().skip(1).collect();
    if urls.is_empty() {
        eprintln!("usage: fetchprobe <url> [...]");
        std::process::exit(2);
    }
    // Why a fetch can differ between launch contexts: dump the full
    // environment to a file when asked, then print anything proxy or
    // certificate related that this process actually sees.
    if std::env::var("FP_DUMP").is_ok() {
        let mut all: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
        all.sort();
        let _ = std::fs::write("C:/Tools/qc-ship/fp-env-dump.txt", all.join("\n"));
        println!("env dump: {} vars", all.len());
    }
    for (k, v) in std::env::vars() {
        let lk = k.to_lowercase();
        if lk.contains("ssl") || lk.contains("proxy") || lk.contains("cert") {
            println!("env  {k}={v}");
        }
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .user_agent("QuotaVPN")
            .build()
            .expect("client");
        for url in &urls {
            match client.get(url).send().await {
                Ok(r) => {
                    let status = r.status();
                    let len = r.text().await.map(|t| t.len()).unwrap_or(0);
                    println!("OK   {url}\n     status={status} bytes={len}");
                }
                Err(e) => {
                    println!("FAIL {url}");
                    let mut src: Option<&(dyn std::error::Error + 'static)> = Some(&e);
                    while let Some(s) = src {
                        println!("     -> {s}");
                        src = s.source();
                    }
                }
            }
        }
    });
}
