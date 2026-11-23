//! Build glue: the build id, and the constant that says no key is baked in.
//!
//! It used to generate an import library for Npcap's `wpcap.dll` and link it, which made the
//! DLL a startup dependency: any PC without Npcap died with "wpcap.dll was not found" before
//! the window appeared. Npcap is loaded at runtime now (`lan_route::npcap()`), so the build
//! needs no packet driver at all and a missing one costs one feature.

fn main() {
    let manifest =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());

    // No credential is baked into a customer build, ever. This used to inject
    // embed/quotacards-embed.pem as EMBED_KEY, which meant every installer
    // carried a private key for the production server. Registration is now an
    // authenticated HTTPS call with a per-device secret, so the constant stays
    // present and empty: src/config.rs includes this file unconditionally.
    let body = "pub const EMBED_KEY: Option<&str> = None;\n".to_string();
    std::fs::write(out_dir.join("embed.rs"), body).expect("write embed.rs");

    // Which build this is, for bug reports and for the updater's own diagnostics.
    let build_id = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&manifest)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "dev".to_string());
    println!("cargo:rustc-env=QUOTACARDS_BUILD={build_id}");
    println!("cargo:rerun-if-changed=.git/HEAD");
}
