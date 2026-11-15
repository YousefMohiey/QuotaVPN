# QuotaVPN - notes for coding agents

The long-form handoff (system map, flows, code map, build and release, traps) is in
`notes/HANDOFF.md`. Read it before changing anything structural.

A VPN client for Windows and Android that routes per-app and per-quota traffic
through a personal Xray/VLESS server. Both apps carry all three transports:
Standard (VLESS), WireGuard (UDP/53) and Hysteria2 (UDP/443). WE's DPI drops plain
WireGuard handshakes from Egypt, so the desktop's WireGuard is AmneziaWG (obfuscated
WireGuard on a forked engine) and connects from there; Hysteria2 is the other
Egypt-proof UDP transport. The phone still runs stock libbox and cannot speak AWG.

## Layout

- `src/` - core Rust: `vpn.rs` (routing rules, per-app policy, WireGuard/Hysteria helpers),
  `server.rs` (server-side provisioning helpers), protocol code shared by both apps.
- `desktop/src-tauri/` - the Windows app: Tauri 2 backend, commands in `src/lib.rs`
  (tray, single instance, signed updater, `tunnel_*`, `generate_card`, `import_card`, `revoke_card`).
- `desktop/ui-next/` - the Windows UI. React 19 + TypeScript + Vite + Tailwind v4 +
  shadcn/ui + Motion + Lucide. This is the shipping UI.
- `desktop/ui/` - the old vanilla UI, kept only as a reference for behaviours the user liked.
- `android/tauri-app/` - Tauri 2 Android app, Kotlin `VpnService`, phone UI in `ui/`
  (vanilla HTML/CSS/JS, not React).
- `android/tauri-plugin-qctunnel/` - the native tunnel plugin, ships `libs/libbox.aar` (27 MB, prebuilt, do not delete).
- `tools/` - `make-icons.py` (all icon sets, desktop + android), `build-apk.sh`, `sync-mobile-assets.sh`.
- `docs/` - `STATUS.md` tracks the current state; read it first.

## Build and verify

Windows, run from git-bash. The toolchain this project was developed against:
`C:/Tools/mingw_extract/mingw64/bin` (MinGW), Android SDK + NDK 28.2.13676358,
`JAVA_HOME` at a JDK 23, Node 22, `tauri-cli` 2.11.4.

- UI: `cd desktop/ui-next && npm install && npm run build`. Run this before any
  installer build: `beforeBuildCommand` is empty, so `tauri build` embeds whatever
  `ui-next/dist` currently holds.
- Desktop app: `cd desktop/src-tauri && cargo build --release --offline --target x86_64-pc-windows-gnu`
  The dev profile does not link under MinGW (`export ordinal too large`), so always
  verify through a release build. Copy `WebView2Loader.dll` next to the exe to run it.
- Installer: from `desktop/`, `node ../android/tauri-cli-npm/node_modules/@tauri-apps/cli/tauri.js build --bundles nsis --target x86_64-pc-windows-gnu`
  with `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` set and NSIS on PATH.
  Produces `bundle/nsis/QuotaVPN_<version>_x64-setup.exe` plus its `.sig`.
- Phone: `bash tools/build-apk.sh` (needs `ANDROID_HOME`, and `cygpath` before any native SDK `.exe`).
- Icons: `python tools/make-icons.py`. It trims the master to its content box and verifies every frame;
  run it after touching `res/app-icon-src.png` only.

## Conventions that matter

- **React stack is mandatory for the desktop UI.** Before adding a component, check shadcn/ui;
  before hand-rolling an animation, check Motion. Do not add another UI library.
- **No dropdown menus.** Choices use the searchable list picker (`components/PickerDialog.tsx`)
  with a check on the current value.
- **Theme**: luminous night scene behind everything, every surface a frosted glass pane
  (`index.css` tokens, `.scene`, `.glass*`). Keep `backdrop-filter` free of the `-webkit-` twin:
  Vite's minifier keeps only that one and Chromium then ignores it, killing the blur.
- **All user-visible strings go through `src/lib/i18n/{en,ar}.ts`** (formal Modern Standard Arabic,
  RTL). Never mirror the layout in Arabic, only the text direction.
- **Never emit em-dashes** in code, UI strings, or docs. Plain hyphens only. The user treats
  anything else as an obvious tell.
- **The Windows manifest is not optional.** It carries the requested execution level and the
  Common-Controls 6 dependency. Without it the exe binds comctl32 v5, `TaskDialogIndirect` does not
  exist there, and the app dies with "Entry Point Not Found" before it draws a window: that is what
  shipped as 0.6.6. `desktop/src-tauri/build.rs` hands `res/quotacards.manifest` to
  `tauri_build::WindowsAttributes::app_manifest`, which compiles it with whatever the toolchain has
  (rc.exe on MSVC, windres on GNU). Do not compile it by hand again, and the release job greps the
  built exe for the dependency before it publishes.
- **Version lives in four files**: `desktop/src-tauri/{tauri.conf.json,Cargo.toml}` and
  `android/tauri-app/src-tauri/{tauri.conf.json,Cargo.toml}`. Bump all four together.
- **Releases**: bump, build the installer and the APK, then publish the GitHub release with the
  updater feed. The feed asset must be named exactly `latest.json`; the app reads
  `releases/latest/download/latest.json`. Ship both platforms at the same version. The feed also
  carries a `build` stamp (short git hash, written by the publish scripts) so a re-release under
  the same version number still reaches installed apps; commit the bump before creating the
  release (the tag follows the remote default branch). Confirm the stamp landed: the short hash
  must appear in the built `release/QuotaVPN.exe` (the setup exe is compressed). A stale
  stamp makes every install re-offer the same release.
- **Secrets** (keystore, updater key and passwords, the server private key in
  `%APPDATA%/quotacards/config.json`) never enter the repo and never get printed.
- **Commit author for this repository is `YousefMohiey`.** Do not add agent or assistant
  attribution to commits, code comments, or docs.

## Verifying UI work

Run the built UI with `npm run preview` (port 4173) and drive it over CDP. Two traps:
the harness throttles timers in pages it considers background, so enable
`Emulation.setFocusEmulationEnabled` before timing anything, and the animation clock can be
frozen, so measure geometry numerically rather than trusting a screenshot.

## The LAN stack

Everything a device sees of this machine goes through one elevated pass, never the window
process. `src/windivert.rs` loads the WinDivert DLL that ships next to the executable and wraps
the `WINDIVERT_LAYER_NETWORK_FORWARD` layer; `src/lan_forward.rs` is the loop that counts, caps
and blocks on it, reading `lan/policy.json` from the engine directory and writing
`lan/state.json`. The window process writes policy, starts the pass through a scheduled task
(`QuotaVPN Network <user>`, argument `--lan-pipeline`), and mirrors the page's flags, which is why
none of it raises a UAC prompt after setup. `Session` and `MonitorSession` keep their old shapes,
so the screen reads what it always read.

The WinDivert pair is fetched, not shipped: setup downloads the official release, checks the
`.sys` Authenticode signature and keeps it in `engine_dir/windivert`, because an unsigned
installer carrying a kernel driver is what Defender quarantines. The app looks there first.

Npcap is not required for watching, capping, blocking, naming or routing. It is only the backend
for forging the 42-byte ARP claim frame on a router that does not already point its devices at
this PC, chosen in one place through `wpcap::claim_link`; with no driver that answers gateway
mode, which is correct exactly when the router's DHCP already sends devices here. Discovery and
naming use `arp -a`, a ping sweep, a UDP socket on 67 when elevated, and unicast mDNS queries.
`pipeline_ready` answers whether the screen can work; `lan_claim_ready` answers whether a claim
frame can be forged, and only claiming should ever be gated on it.

## Current state

**The latest published release is v0.6.13.** The four version files are aligned at 0.6.13 in this
tree. The Windows build is done by the `signed-release` workflow on a GitHub runner from a `v*`
tag and `latest.json` is composed by hand from the run's `.exe` and `.sig` artifacts. There is no
code signing: every installer is unsigned, which is why SmartScreen warns and heuristic engines
sometimes quarantine it. Three releases are published and broken on purpose
of record: v0.6.6 had no manifest and could not start, Defender quarantined v0.6.7 for carrying a
kernel driver, and v0.6.8 for installing persistence from the installer. Do not rebuild from those
tags. `notes/defender.md` has the whole sequence and the false-positive report.

What the shipping builds are: Windows is the React app in `desktop/ui-next`; the phone still runs
the vanilla UI in `android/tauri-app/ui`. All three transports ship on both platforms, the
desktop's WireGuard being AmneziaWG. `notes/STATUS.md` has the Sept 2026 detail and
`notes/linux.md` covers the Linux port.

`cargo test` is green (53 passed) on Linux. The Windows-only modules and `src/vpn.rs` itself
type-check for `x86_64-pc-windows-gnu` through a stub crate, because the real Windows build stops
in the `ring` build script without a mingw C compiler; the Tauri crate still needs a Windows
toolchain, so its release build is the first real check of it.
