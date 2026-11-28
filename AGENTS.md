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
- `tools/` - `make-icons.py` (all icon sets, desktop + android), `android-icons.py` (the Android
  launcher mipmaps, run by `build-apk.sh`), `build-apk.sh`, `sync-mobile-assets.sh`.
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
- Updater feed: write `latest.json` as UTF-8 without BOM. PowerShell's `Set-Content -Encoding utf8`
  adds a BOM on 5.1, `serde_json` then fails, and the experimental check reports `offered=none`
  even though the file looks correct. Use
  `[IO.File]::WriteAllText($path, $json, (New-Object Text.UTF8Encoding($false)))` and confirm the
  first bytes are `7B`, not `EF BB BF`.
- Phone: `bash tools/build-apk.sh` (needs `ANDROID_HOME`, and `cygpath` before any native SDK `.exe`).
- Icons: `python tools/make-icons.py`. It trims the master to its content box and verifies every frame;
  run it after touching `res/app-icon-src.png` only.

## Conventions that matter

- **React stack is mandatory for the desktop UI.** Before adding a component, check shadcn/ui;
  before hand-rolling an animation, check Motion. Do not add another UI library.
- **No dropdown menus.** Choices use the searchable list picker (`components/PickerDialog.tsx`)
  with a check on the current value.
- **Theme**: Quota uses the approved flat dark steps (`#000000`, `#181818`, `#1F1F1F`,
  `#272727`); Catppuccin, Tokyo Night, Nord, Dracula, Gruvbox, Rosé Pine and One Dark use their
  official dark palettes. No background gradients (`index.css` tokens, `.scene` grain only);
  `.glass*` is for dialogs and popovers, not every pane. Eight palettes live in
  `src/state/theme.tsx` and `index.css`; they change surfaces, text, lines and accent together,
  not just the accent. They are chosen in Settings > Appearance only, never in the rail. The
  picker is a row of palette orbs, not fake mini-windows. A custom primary colour can override
  the preset accent, and the real in-app icon follows it. The interface font is a user choice
  from Inter, Geist, Manrope, IBM Plex Sans, Archivo, JetBrains Mono and the installed Windows
  faces; Inter is the default, Geist Mono stays for addresses and latency. No italics. Phosphor
  is the only icon set, at the regular weight. Keep `backdrop-filter` free of the `-webkit-` twin:
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
  together. The desktop is 0.7.6-exp.13; the phone is 0.7.5. The phone still compiles the legacy SSH
  control plane (`legacy-ssh`) until its own credential migration lands; do not bump the phone
  to a version that has no tested APK behind it. Experimental desktop-only builds may move ahead
  of the phone; stable releases still ship both at the same version.
- **Releases**: bump, build the installer and the APK, then publish the GitHub release with the
  updater feed. The feed asset must be named exactly `latest.json`; the app reads
  `releases/latest/download/latest.json`. Ship both platforms at the same version. The feed also
  carries a `build` stamp (short git hash, written by the publish scripts) so a re-release under
  the same version number still reaches installed apps; commit the bump before creating the
  release (the tag follows the remote default branch). Set `QC_BUILD` to the same short hash for
  the Windows build; `build.rs` accepts it first and then searches PATH and Git for Windows for
  `git`. Confirm the stamp landed: the short hash must appear in the built `release/QuotaVPN.exe`
  (the setup exe is compressed), and a build that reports `dev` must not be published. A stale
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

One device at a time is carried by `src/lan_relay.rs`, inside the window process (the manifest
already requires an administrator token). It opens Npcap non-promiscuous on the wired adapter,
asks the segment who holds the device's address, then tells the device this PC is the gateway and
tells the router this PC is the device. Frames are re-addressed at layer 2 and sent back out, so
the PC's IP stack, routes, firewall and VPN never see them. There is no IP forwarding to enable
and no WinDivert pipeline. The capture opens before anything is claimed, a frame must actually be
sent before the session is returned, the device must answer its probe first, and every exit
including a panic sends the true mappings back. A killed process is restored from
`lan_journal.txt` at the next start.

Before the first claim, the PC's own neighbour entry for the router is made Permanent with the
router's real hardware address and recorded in `lan_pin.txt`. That entry is what keeps an ARP
reply from pointing the PC at itself and taking its own connection down; release removes it, a
watchdog re-pins it, and a session refuses to start if Windows will not pin it. `lan::tidy_roster`
runs before the page paints: it forces the router and this PC rows to their real addresses, drops
any other row wearing either address, and dedupes the rest. The page key is `ip|mac|self`, so two
rows can never collide into duplicate React children.

Npcap is the only packet driver the control path needs. The free installer has no silent mode, so
the app runs its wizard. The relay opens non-promiscuous on purpose: promiscuous mode is the
setting Npcap's own guide says to turn off when a wireless adapter misbehaves. Discovery and
naming never need the driver: the Network page reads `arp -a` and, when signed in, the router's
client table; the page's explicit Refresh starts the 254-address sweep in the background, and it
never runs automatically or blocks the list (`--lan-probe` is the headless version). The DHCP and
mDNS listeners still supply names.

The router remains the optional second path, for names now and for upload control later.
`router_huawei.rs` logs in for the session only, never stores the password, and dumps the QoS and
MAC-filter pages. Names stay hidden until that login.

**The upload half works when the device accepts the claim.** The relay sends three forms of the
gateway claim to the device: a targeted reply, a gratuitous reply, and a gratuitous request. Some
stacks ignore the first and take one of the others; the 2514 does. A device that ignores all three
keeps its uploads on the direct path; `Network.tsx` detects `up_bytes == 0` and disables the
upload field with an explanation. Do not claim upload caps on such a device; the router's QoS is
the only enforcement point.

**No scheduled tasks, ever.** `Behavior:Win32/Persistence.A!ml` named
`C:\Windows\System32\Tasks\QuotaVPN Network ...` and quarantined the app twice, once from the
installer and once from the app. Registering a task whose action launches the executable is the
detected behaviour regardless of the token used. The engine is a detached child of the app, which
already holds an administrator token. `install_lan_task` and `install_helper_task` are cleanup
functions that delete tasks left by older versions, and `task_installed` / `lan_task_installed`
answer true because there is nothing to install. The release job fails if the binary contains
`<Task version=` or `schtasks /Create`.

## Current state

**Exp.7 desktop (27 November 2026).** Spaces starts the bundled QuotaSpace engine for the game LAN
(`desktop/src-tauri/src/openrad.rs`, `openrad/QuotaSpace.exe`, `modulus.bin`). One press creates the
`QuotaSpace` TAP adapter, clears a stale `26.x` address, configures the self-hosted server, starts the
service and joins the selected space's private network. The bundled engine was patched to fall back
to public STUN for its NAT mapping when the server has no UES hosts, so a direct UDP path is used
when both NATs allow it. A Windows PC and a VPS peer reached `DirectUdp` at about 45 ms; the card
shows only connected peers with `Direct` or `Relayed` and RTT. Spaces is a row of QuotaSpace space
chips, the selected space's card, its invite and Remove. There is no automatic baseline and
nothing connects on its own: the page starts empty, the user creates the first space, and Connect
starts the selected one. The old layer-3 spaces and the coordinator are gone. Each space is its own
OpenRad private network and the engine joins only the selected one, so peers and broadcasts never
mix between spaces. The old public `QuotaVPN` network has been deleted from the server and the unit
no longer passes `--public "QuotaVPN"`; existing peers move by pasting the new invite. The
self-hosted server is patched to keep a private network after its last member leaves and to promote
the oldest remaining member when the admin switches away; see `notes/OPENRAD-SERVER-PATCH.md`. The
in-house layer-2 switch below stays in the tree but is not the shipping LAN path. The window also
moved to Manrope with a user font picker, Geist Mono for data, flat surfaces, seven full palettes
(Serein, Eclipse, Slate, Nightfall, Ember, Verdant, Afterglow), Phosphor icons at one weight, and
the full approved dark surface set. The Windows app and the bundled
engine are signed with the local `CN=YousefMohiey` certificate, so UAC and the firewall prompt show
that publisher once the certificate is trusted on the PC.

**Spaces layer 2 (desktop, superseded).** A space can carry Ethernet instead of IP. The adapter is
TAP-Windows6 9.27.0, bundled in `desktop/src-tauri/tap` and installed by the NSIS hook; 9.27.0 is
the floor because 9.26 and earlier carry CVE-2024-1305. It is GPLv2, so the installer ships
`GPL-2.0.txt` and the driver source tarball. `desktop/src-tauri/src/tap.rs` opens the recorded
adapter and configures MAC, MTU, address and profile; `src/spaces_switch.rs` learns MACs, unicasts,
floods broadcast and multicast, drops spoofed, duplicated and link-local frames, and rate limits
each peer; `src/spaces_direct.rs` opens one Noise path per member and runs the switch. The space
record carries `mode` (`l3` or `l2`) and the coordinator stores it, returns each member's MAC and
accepts `POST /v1/spaces/{id}/mode`. Layer 3 is still the default, the phone is still 0.7.5, and
`notes/SPACES-L2.md` holds the audit, the plan and the proofs.

**The latest desktop release is v0.7.6-exp.13 (experimental, Windows); stable is v0.7.5, Windows
and Android.** This tree carries the 0.7.0
credential migration (`notes/HANDOFF-2026-11-22.md`): the desktop imports a server-issued `qv1:`
secret into a DPAPI-sealed per-card store (`qc1:` is still accepted), talks to the card API over
HTTPS, and no longer compiles the SSH control plane. The phone is 0.7.5 too; its
`server.rs`/`ssh.rs` path is still behind `legacy-ssh`. The exp.13 release carries the Windows setup
`QuotaVPN_0.7.6-exp.13_x64-setup.exe`; the stable 0.7.5 release carries the phone APK
`QuotaVPN-mobile-signed.apk` (the name the phone updater looks for). The standalone `QuotaVPN.exe`
is not uploaded as a release asset.

**Exp.12 appearance (28 November 2026).** The palette set is now Quota plus the most-used editor
themes: Catppuccin, Tokyo Night, Nord, Dracula, Gruvbox, Rosé Pine and One Dark. Quota is the
default and keeps the approved flat dark steps; each editor palette uses its official colours
mapped onto the same surface steps. Appearance is Settings only: a row of palette orbs, a primary
colour picker and an interface font picker. The rail swatches are gone, and the old preview
cards are replaced by the orbs. The interface font defaults to Inter; Geist, Manrope, IBM Plex
Sans, Archivo and JetBrains Mono are bundled alongside it, and the installed Windows faces remain.
The in-app icon is the shipping PNG, unchanged: a canvas pass recolours only the royal-blue tail
to the active accent. The Windows icon and tray asset stay static. Button labels follow the
accent, so pale presets use black text and the deep Quota blue keeps white. Theme values migrate
(`serein`/`obsidian` to Quota, `eclipse` to Tokyo Night, `slate` to Nord, `nightfall`/`violet` to
Dracula, `ember` to Gruvbox, `verdant`/`mint` to Catppuccin, `afterglow` to Rosé Pine).

**Exp.13 spaces and scale (28 November 2026).** Appearance now has an Interface scale row
(90/100/110/125/150). It applies through the WebView zoom, so fixed sizes scale too, and it is saved
under `qc-scale`. Spaces leave gracefully on Disconnect and before a switch, so the patched server
promotes a remaining member and keeps the network. A switch uses `openrad_switch`; create and join
leave the current space first. Creating or joining while not connected no longer connects on its
own; the new space waits for Connect. If the selected space disappears, the page stays unselected
instead of jumping to another one. The owner Remove dialog offers Leave space as the primary action
and Delete space as the explicit destructive one. A Disconnect now shows `left` in the server log
and the network survives with zero members.

0.7.4 restored the phone UI that shipped before the rejected 0.7.3 rebuild. 0.7.5 is the motion
and interaction pass on that same layout: a sliding active pill in the bottom bar, a short page
crossfade, the desktop's `#13161a` disc with a single waiting arc while connecting, brand blue for
the connecting state, the desktop primary foreground on the Start test button, press feedback
across lists and pickers, a selected-check pop, and a real measuring state on Speed (tinted button,
moving line, Stop). The automatic server probe on launch is silent; a connection the user starts
says Connecting instead of Working. 0.7.4 remains published; 0.7.3 was removed. 0.7.5 carries the
white-window fix from 0.7.3 and the Npcap fix from 0.7.2; 0.7.1 and 0.7.0 remain published. The
Network page ships in the Windows build, carried by the layer-2 relay.

`notes/HANDOFF-2026-11-16.md` is the change log from 0.6.5 to 0.6.17. The Windows installer is
built locally on this PC and signed with the updater key at `C:/Tools/qc-updater.key`;
`latest.json` is composed by hand from the installer, `.sig` and the locally built
`QuotaVPN.exe` (the exe is used for the build stamp, not shipped as a release asset). The
`signed-release` workflow is a manual fallback only; tags do not start it. Its 0.7.1 attempt failed
on a transient NSIS download. The feed is served from
`raw.githubusercontent.com/.../master/latest.json`, committed to the branch on every release,
because that URL answers 200 with no redirect; the release asset is the second endpoint. There is
no code signing and there is no signing provider wired up: every installer is unsigned, which is
why SmartScreen warns and heuristic engines sometimes quarantine it. The broken 0.6.6 to 0.6.8
releases were removed from the Releases page on 24 November 2026, leaving 0.7.0 and later; their
tags remain, so do not rebuild from them. `notes/defender.md` has the whole sequence and the
false-positive report.

What the shipping builds are: Windows is the React app in `desktop/ui-next`; the phone still runs
the vanilla UI in `android/tauri-app/ui`. The desktop keeps all three transports when a card has
cached Hysteria2/WireGuard values; a newly imported card is VLESS only until the API can provision
the other two. `notes/STATUS.md` has the Sept 2026 detail and `notes/linux.md` covers the Linux port.

`cargo test` is green (121 passed) on this tree. The Windows app builds for the MSVC host target;
the GNU target currently fails in `embed-resource`, not in the Rust code. The card API hostname is
`quotavpn.duckdns.org` and resolves to `80.225.89.162`. TCP 443 and TCP 8444 both answer, the
`qvreg` API certificate for the new name is deployed, and the real card reports active.

## Agent skills and MCPs

Load `quota-vpn`, `rust-windows-app`, `tauri-mcp`, `craft-ui-style`,
`reference-ui-replication` and `delivery-verification` before UI or build work.
The project `opencode.json` registers the `tauri` MCP for driving the built app;
use it for screenshots, DOM snapshots and console checks.
