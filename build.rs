//! Build glue: the embedded device key and the build id.
//!
//! It used to generate an import library for Npcap's `wpcap.dll` and link it, which made the
//! DLL a startup dependency: any PC without Npcap died with "wpcap.dll was not found" before
//! the window appeared. Npcap is loaded at runtime now (`lan_route::npcap()`), so the build
//! needs no packet driver at all and a missing one costs one feature.



fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());

    // The device key, baked in when present. `src/config.rs` includes this file
    // unconditionally, so a build that does not write it fails with "could not find file
    // .../embed.rs" on a clean tree. It was dropped when this script was rewritten for
    // Npcap, and a cached target directory hid the breakage until a tag-triggered CI run
    // on a clean checkout.
    let pem = manifest.join("embed").join("quotacards-embed.pem");
    let body = match std::fs::read_to_string(&pem) {
        Ok(pem) if !pem.trim().is_empty() => {
            let esc = pem.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
            format!("pub const EMBED_KEY: Option<&str> = Some(\"{esc}\");\n")
        }
        _ => "pub const EMBED_KEY: Option<&str> = None;\n".to_string(),
    };
    std::fs::write(out_dir.join("embed.rs"), body).expect("write embed.rs");
    println!("cargo:rerun-if-changed=embed/quotacards-embed.pem");

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
