//! Build glue: the embedded device key and the build id.
//!
//! It used to generate an import library for Npcap's `wpcap.dll` and link it, which made the
//! DLL a startup dependency: any PC without Npcap died with "wpcap.dll was not found" before
//! the window appeared. Npcap is loaded at runtime now, so the build needs no packet driver
//! at all and a missing one costs one feature.

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());

    // The device key, baked in when present. `src/config.rs` includes this file
    // unconditionally, so a build that does not write it fails with "could not find file
    // .../embed.rs" on a clean tree.
    //
    // It comes from the file, or from `QC_EMBED_PEM` when the file is not there. The file
    // is gitignored on purpose, so a CI checkout never has it: 0.6.9 shipped without the
    // embedded key, and an install with no key and no saved server has nowhere to point,
    // so every connect answered "Set up your server first" with no way to set one. The
    // variable is how a build machine supplies the key without the secret entering the
    // repo.
    println!("cargo:rerun-if-env-changed=QC_EMBED_PEM");
    let pem = manifest.join("embed").join("quotacards-embed.pem");
    let from_environment = std::env::var("QC_EMBED_PEM").ok().filter(|v| !v.trim().is_empty());
    let key = from_environment.or_else(|| {
        std::fs::read_to_string(&pem)
            .ok()
            .filter(|text| !text.trim().is_empty())
            .map(|text| text.trim().to_string())
    });
    let body = match key {
        Some(key) => {
            let esc = key.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
            format!("pub const EMBED_KEY: Option<&str> = Some(\"{esc}\");\n")
        }
        None => "pub const EMBED_KEY: Option<&str> = None;\n".to_string(),
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
