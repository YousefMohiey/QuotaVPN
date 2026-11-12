//! Build glue for the Windows-only clientless routing.
//!
//! Sending a forged ARP frame needs Npcap's `wpcap.dll`. The build host has the Npcap
//! runtime but not its SDK, and it builds with the GNU toolchain, which cannot consume
//! an MSVC `.lib`. So the import library is generated here from a committed `.def` with
//! MinGW's `dlltool`, which is already on PATH. No SDK download, no binary committed.
//!
//! The earlier version ran `gendef` at build time. That was not deterministic: when it
//! produced nothing the build silently linked against whatever happened to be lying
//! around, and one call site failed to resolve while the rest appeared fine. A committed
//! `.def` is reviewable and identical on every machine.
//!
//! If `dlltool` or the `.def` is missing the build still succeeds, and the module
//! reports that device routing is unavailable instead of failing to compile the app.

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

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let def = manifest.join("native").join("wpcap.def");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let lib = out.join("libwpcap.a");

    println!("cargo:rerun-if-changed={}", def.display());

    if !def.exists() {
        println!("cargo:warning=native/wpcap.def is missing; device routing will report itself unavailable");
        return;
    }

    let built = std::process::Command::new("dlltool")
        .arg("-d")
        .arg(&def)
        .arg("-l")
        .arg(&lib)
        .status();

    if !matches!(built, Ok(s) if s.success()) || !lib.exists() {
        // Fail rather than warn. A silent "device routing is unavailable" is the same trap as
        // the strip step: the build looks green and the feature is simply missing at runtime.
        // The commonest cause is not Npcap at all: without a mingw64 bin on PATH, dlltool
        // cannot find `as.exe` and dies with "CreateProcess", and the rustup self-contained
        // folder ships gcc and ld but no assembler.
        panic!(
            "dlltool could not build libwpcap.a from native/wpcap.def.\
             \n  Commonest cause: no mingw64 bin on PATH, so dlltool cannot find as.exe.\
             \n  On the build host: prepend /c/Tools/mingw_extract/mingw64/bin (or the rustup-launcher \
             mingw64\\bin) before running cargo."
        );
    }

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=wpcap");
}
