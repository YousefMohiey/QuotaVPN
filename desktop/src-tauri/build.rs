//! Desktop build: Tauri codegen plus the Windows UAC manifest (TUN mode
//! needs admin), embedded with windres. The exe icon is NOT embedded here:
//! Tauri sets it from icons/icon.ico at bundle time, and a second embedded
//! copy creates a second icon group whose entries no longer match the icon
//! data, which is exactly how Explorer ends up showing a pixelated icon.
//! Degrades to a warning, never a build failure, if no resource compiler
//! is found.
use std::{env, fs, path::Path, path::PathBuf};

/// The short commit, found through PATH first and the usual Git for Windows
/// location second. The Tauri build can strip PATH down far enough that plain
/// `git` is missing; falling back to "dev" there gives the updater no identity
/// and makes it re-offer the same release forever.
fn git_stamp(root: &Path) -> Option<String> {
    let mut candidates = vec!["git".to_string()];
    if cfg!(windows) {
        candidates.push(r"C:\Program Files\Git\cmd\git.exe".to_string());
    }
    for git in candidates {
        let output = std::process::Command::new(git)
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(root)
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let stamp = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !stamp.is_empty() {
                    return Some(stamp);
                }
            }
        }
    }
    None
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // repo root two levels up: desktop/src-tauri -> desktop -> root
    let root = manifest_dir.join("..").join("..");

    // Build identity for the updater: a hotfix re-released under the same
    // version number differs only by build, so the app compares this stamp
    // against the feed's "build" field. Falls back to "dev" (git missing,
    // not a checkout), and a dev build is never offered a same-version
    // update because it has no identity to compare.
    let stamp = env::var("QC_BUILD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit()))
        .or_else(|| git_stamp(&root))
        .unwrap_or_else(|| "dev".to_string());
    println!("cargo:rustc-env=QC_BUILD={stamp}");
    // Re-read the stamp whenever either side of the ref moves: the HEAD file
    // changes on checkouts, while a commit only advances the branch ref under
    // refs/heads (cargo watches a directory recursively). Watching HEAD alone
    // kept the PREVIOUS commit's stamp in a rebuild after a commit, and a
    // stale stamp makes the updater re-offer the same release forever.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");

    // The application manifest goes through tauri-build, which compiles the
    // resources with whatever this toolchain actually has: rc.exe on MSVC,
    // windres on GNU. Doing it by hand was the bug in 0.6.6: this script shelled
    // out to windres itself and, on a runner that had none, warned and shipped an
    // exe with no manifest at all. That exe bound comctl32 v5, which has no
    // TaskDialogIndirect, so it died with "Entry Point Not Found" before it could
    // draw a window. `res/quotacards.manifest` carries both halves now: the
    // requested execution level and the Common-Controls 6 dependency.
    // tauri-build 2.6 reads <target-dir>/build on the GNU branch. On a clean
    // cross-build that directory does not exist yet and the whole build dies
    // with os error 3 before the manifest step. Create it first; harmless when
    // it already exists.
    if let Ok(out) = env::var("OUT_DIR") {
        let mut target_dir = PathBuf::from(out);
        for _ in 0..3 {
            target_dir.pop();
        }
        let _ = fs::create_dir_all(target_dir.join("build"));
    }

    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("../../res/quotacards.manifest"));
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("tauri-build");

    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    // Test binaries are not covered by the manifest above, and on the GNU
    // toolchain a test binary without the Common-Controls dependency dies before
    // main (0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND). They get an asInvoker copy
    // through windres, because the app manifest demands admin and a test runner
    // must not. A missing windres costs the tests, never the app, so this is a
    // note and not a failure.
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let windres = ["windres.exe", "x86_64-w64-mingw32-windres.exe"]
        .into_iter()
        .find(|w| std::process::Command::new(w).arg("--version").output().is_ok())
        .or_else(|| {
            let known = PathBuf::from(r"C:\Tools\mingw_extract\mingw64\bin\windres.exe");
            known.exists().then_some("C:\\Tools\\mingw_extract\\mingw64\\bin\\windres.exe")
        });
    let Some(w) = windres else {
        println!("cargo:warning=windres not found; test binaries will have no manifest");
        return;
    };
    let test_manifest = out.join("test.manifest");
    let _ = fs::write(
        &test_manifest,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<assembly xmlns=\"urn:schemas-microsoft-com:asm.v1\" manifestVersion=\"1.0\">\n",
            "  <dependency><dependentAssembly><assemblyIdentity type=\"win32\" ",
            "name=\"Microsoft.Windows.Common-Controls\" version=\"6.0.0.0\" ",
            "processorArchitecture=\"*\" publicKeyToken=\"6595b64144ccf1df\" ",
            "language=\"*\"/></dependentAssembly></dependency>\n",
            "</assembly>\n"
        ),
    );
    let test_rc = out.join("test.rc");
    let test_res = out.join("test.res");
    let tm = test_manifest.to_string_lossy().replace('\\', "/");
    if fs::write(&test_rc, format!("1 RT_MANIFEST \"{tm}\"\n")).is_ok()
        && std::process::Command::new(w)
            .args([
                test_rc.to_string_lossy().as_ref(),
                "-O",
                "coff",
                "-o",
                test_res.to_string_lossy().as_ref(),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        && test_res.exists()
    {
        println!("cargo:rustc-link-arg-tests={}", test_res.to_string_lossy());
    } else {
        println!("cargo:warning=windres failed; test binaries will have no manifest");
    }
    println!("cargo:rerun-if-changed=../../res/quotacards.manifest");
}
