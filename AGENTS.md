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
  `qcreg.rs` (pure `qc1:` credential parsing, HMAC signing and request shape), protocol code shared
  by both apps. `server.rs` and `ssh.rs` sit behind the `legacy-ssh` feature: the phone still uses
  them, the Windows app turns the feature off so `russh` never enters its binary.
- `desktop/src-tauri/` - the Windows app: Tauri 2 backend, commands in `src/lib.rs`
  (tray, single instance, signed updater, `tunnel_*`, `import_credential`, `card_status`,
  `forget_card`, `copy_card`, `set_card_sni`). Card credentials live in
  `src/credentials.rs` (DPAPI-sealed map) and the HTTPS client is `src/cardapi.rs`.
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

- UI: `cd desktop/ui-next && npm install && npm run build`. `beforeBuildCommand` also runs this,
  but a clean UI build before packaging is still the habit that catches a stale `dist`.
- Desktop app: `cd desktop/src-tauri && cargo build --release` (the MSVC host target, which is
  what the release workflow uses). The release workflow on `windows-latest` is the canonical
  build. The GNU target still compiles but currently fails at link: on a Windows MSVC host
  `embed-resource` uses `rc.exe` and writes a `.res` file named `resource.lib`, which MinGW's
  `ld` refuses. Do not treat that as a code failure. Copy `WebView2Loader.dll` next to the exe
  to run a bare build, or use the installer.
- Installer: from `desktop/`,
  `node ../android/tauri-cli-npm/node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`
  with `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` set and NSIS on PATH.
  For a private build without the updater key, add
  `--config <file>` where the file contains `{"bundle":{"createUpdaterArtifacts":false}}`.
  Produces `bundle/nsis/QuotaVPN_<version>_x64-setup.exe` plus its `.sig` when signing is on.
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
  `android/tauri-app/src-tauri/{tauri.conf.json,Cargo.toml}`. A normal release bumps all four
  together. The 0.7.0 migration is deliberately split: the desktop is 0.7.0 and the phone stays
  at 0.6.18 until its own credential migration lands. Do not bump the phone to a version that
  has no tested APK behind it.
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

The desktop app itself can be driven through the `tauri` MCP server (skill `tauri-mcp`).
`app_launch` cannot open a WebView2 port on this app: it runs elevated, and WebView2 150+
ignores the env-var arguments that `tauri-driver` uses. Build the MCP binary with
`powershell -ExecutionPolicy Bypass -File tools/build-mcp.ps1`; it writes
`desktop/src-tauri/target/release/QuotaVPN-mcp.exe` with `--remote-debugging-port=9222`
and leaves the normal binary untouched. Launch that copy, then `tauri_app_attach` with
`debugPort: 9222`. Never ship the MCP copy.

## The LAN stack

Everything a device sees of this machine goes through one elevated pass, never the window
process. `src/windivert.rs` loads the WinDivert DLL that ships next to the executable and wraps
the `WINDIVERT_LAYER_NETWORK_FORWARD` layer; `src/lan_forward.rs` is the loop that counts, caps
and blocks on it, reading `lan/policy.json` from the engine directory and writing
`lan/state.json`. The window process writes policy and starts the pass as a detached child of
itself (argument `--lan-pipeline`); there is no scheduled task any more, see below.

The WinDivert pair is neither shipped nor fetched. It was fetched for a day and that, plus the
task registration, is what kept getting the app quarantined. The loader still verifies the DLL
against a SHA-256 pinned in `src/vpn.rs` before `LoadLibrary`, and only looks in
`engine_dir/windivert` and beside the executable (the checkout paths exist in debug builds only).
Without the driver, watching, capping and blocking cannot work: they filter packets this PC
forwards, and nothing else on Windows can drop a forwarded packet.

Npcap is the backend for forging the 42-byte ARP claim frame, chosen in one place through
`wpcap::claim_link`. Discovery and naming never need it: they use `arp -a`, a ping sweep, a UDP
socket on 67 when elevated, and unicast mDNS queries. `lan_claim_ready` gates only claiming.
GatewayMode used to answer "sent" without sending anything, so a route could be reported as
working when it was not; it answers false now and `claim` refuses with a reason.

**No scheduled tasks, ever.** `Behavior:Win32/Persistence.A!ml` named
`C:\Windows\System32\Tasks\QuotaVPN Network ...` and quarantined the app twice, once from the
installer and once from the app. Registering a task whose action launches the executable is the
detected behaviour regardless of the token used. The engine and the pipeline are detached
children of the app, which already holds an administrator token. `install_lan_task` and
`install_helper_task` are cleanup functions that delete tasks left by older versions, and
`task_installed` / `lan_task_installed` answer true because there is nothing to install. The
release job fails if the binary contains `<Task version=` or `schtasks /Create`.

## Current state

**The latest published release is v0.6.18.** This tree carries the Windows 0.7.0 credential
migration (`notes/HANDOFF-2026-11-22.md`): the desktop imports a server-issued `qc1:` secret into a
DPAPI-sealed per-card store, talks to the card API over HTTPS, and no longer compiles the SSH
control plane. The phone stays at 0.6.18 for now; its `server.rs`/`ssh.rs` path is behind
`legacy-ssh`. The private Windows 0.7.0 installer was built at
`desktop/src-tauri/target/release/bundle/nsis/QuotaVPN_0.7.0_x64-setup.exe` with updater artifacts
off. It is not published and must not be distributed until the owner installs and approves it.

`notes/HANDOFF-2026-11-16.md` is the change log from 0.6.5 to 0.6.17. The Windows build is done by
the `signed-release` workflow on a GitHub runner from a `v*` tag and `latest.json` is composed by
hand from the run's `.exe`, `.sig` and `QuotaVPN.exe` artifacts. The feed is served from
`raw.githubusercontent.com/.../master/latest.json`, committed to the branch on every release,
because that URL answers 200 with no redirect; the release asset is the second endpoint. There is
no code signing and there is no signing provider wired up: every installer is unsigned, which is
why SmartScreen warns and heuristic engines sometimes quarantine it. Three releases are published
and broken on purpose of record: v0.6.6 had no manifest and could not start, Defender quarantined
v0.6.7 for carrying a kernel driver, and v0.6.8 for installing persistence from the installer. Do
not rebuild from those tags. `notes/defender.md` has the whole sequence and the false-positive
report.

What the shipping builds are: Windows is the React app in `desktop/ui-next`; the phone still runs
the vanilla UI in `android/tauri-app/ui`. The desktop keeps all three transports when a card has
cached Hysteria2/WireGuard values; a newly imported card is VLESS only until the API can provision
the other two. `notes/STATUS.md` has the Sept 2026 detail and `notes/linux.md` covers the Linux port.

`cargo test` is green (102 passed) on this tree. The Windows app builds for the MSVC host target;
the GNU target currently fails in `embed-resource`, not in the Rust code. The card API hostname is
`quotavpn.duckdns.org` and resolves to `80.225.89.162`. TCP 443 answers; TCP 8444 still times out,
so the OCI ingress rule and an API certificate for the new name are the remaining network
blockers.

## Agent skills and MCPs

Load `quota-vpn`, `rust-windows-app`, `tauri-mcp`, `craft-ui-style`,
`reference-ui-replication` and `delivery-verification` before UI or build work.
The project `opencode.json` registers the `tauri` MCP for driving the built app;
use it for screenshots, DOM snapshots and console checks.
