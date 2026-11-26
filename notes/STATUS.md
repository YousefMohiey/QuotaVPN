# Status (Sept 2026)

> **0.7.0 note:** the Windows app no longer generates, imports or revokes cards over SSH. It
> imports a server-issued `qc1:` credential into a DPAPI-sealed store and calls the card API over
> HTTPS. See `notes/HANDOFF-2026-11-22.md`. The phone still uses the legacy SSH path.
>
> **0.7.1 note:** the Network page is carried by a layer-2 relay on Npcap, not the retired
> WinDivert forward pipeline. The PC's gateway entry is pinned for the session, so a session
> cannot take the PC's own connection down. The relay sends three ARP forms to the device; a
> device that accepts one carries both directions, so download and upload caps and Block work
> without a router login. A device that ignores all three bypasses uploads and the page says so.
> The page is one roster and one detail rail; it reads the neighbour and router tables at once, so
> devices appear immediately. An explicit Refresh scans for sleeping devices in the background.
> The caps are two labelled boxes with an apply action, the header is one row, and the rest of the
> app now carries the same state motion. The action is Monitor, and idle devices are hidden while
> router names are on, with a Show idle link. Dialogs rise and close on a backdrop press, the Home
> cells and Speed band carry larger moving chevrons and icons, the Speed host and IP copy, and pages
> slide in the direction of the rail. Network now sits before Speed. Published 24 November 2026.
>
> **0.7.2 note:** the driver check and the capture loader now load `wpcap.dll` from
> `C:\Windows\System32\Npcap` with that folder on the dependency search path. Npcap installed
> without WinPcap-compatible mode keeps `Packet.dll` beside it, so the old load made a working
> driver look missing.
>
> **0.7.3 note:** the delayed WebView2 suspend now rechecks the current memory state after its
> grace period. A rapid minimize and restore can no longer be suspended after the resume, which
> is what left a white window. 0.7.3 was replaced by 0.7.4; its release and tag were removed.
>
> **0.7.4 note:** 0.7.4 replaces the withdrawn 0.7.3. It restores the phone UI that shipped
> before the 0.7.3 rebuild, with the fixes that were actually needed: the settings and speed
> columns scroll instead of shrinking (that flex shrink was the overlap and clipped text on short
> screens), the session strip wraps, the selection caret is gone, the four-cell speed grid becomes
> 2x2 on very narrow phones, the display face is Archivo like the desktop, and the launcher icon
> is the QuotaVPN mark again. `tools/android-icons.py` now writes the mipmaps on every build,
> because `tauri android init` had put the Tauri template icon there. The phone updater still
> scans for the newest release that carries the APK. The Windows build carries the white-window
> fix from 0.7.3 and the Npcap fix from 0.7.2.
>
> **0.7.5 note:** a motion and interaction pass on the restored phone UI. The bottom bar now has
> one active pill that slides between tabs, pages crossfade with a 4px lift, the connect disc uses
> the desktop's `#13161a` face with a single waiting arc while connecting and the desktop green
> while live, the Start test button uses the desktop primary foreground, the speed control has a
> real measuring state, and list/picker rows have press feedback and a selected-check pop. The
> automatic server probe on launch is silent; a connection the user starts says Connecting instead
> of Working. 0.7.4 remains published; 0.7.3 remains removed.
>
> **Spaces (unreleased, 25 November 2026):** the first slice of a private virtual LAN. The desktop
> has a Spaces screen, the core has the invite/key/endpoint model, and `embed/qc-spaces.py` is the
> coordinator. The data plane is one WireGuard endpoint inside the existing sing-box session, with
> `detour: proxy`, so the handshake rides the main tunnel and needs no new public UDP port or
> AmneziaWG layer. The hub is `embed/qc-spaces-setup.sh` plus a kernel `qcspaces` interface on the
> VPS. The server has not been deployed yet and the phone is untouched at 0.7.5. The full design,
> including why layer 2 and direct peer-to-peer are later phases, is in `notes/FEATURE-SPACES.md`.
>
> **Spaces research update:** Radmin is direct-first, not relay-first. Direct peers use UDP
> hole punching; the TCP relay is only the fallback. The current Spaces slice always relays
> through the VPS, so it cannot match a 9 ms direct link, and it is layer 3, so Minecraft LAN
> discovery does not work. The corrected direction, the public reverse-engineering evidence and
> the GitHub implementations are in section 12 of `notes/FEATURE-SPACES.md`. OpenRad 2.4.5 was
> inspected at source and release level; the verdict is in `notes/OPENRAD-INTEGRATION.md`. The
> decision is to build inside QuotaVPN, not to ship OpenRad as a sidecar or a second app.
>
> **Direct-path probe, 26 November 2026:** the first in-house step is implemented. The coordinator
> records each member's public UDP endpoint on port 8446, the desktop has a `space_probe` command,
> and the Spaces screen has a Direct path test showing Direct or Relay with an RTT. The probe now
> establishes a real Noise IK session (`Noise_IK_25519_ChaChaPoly_BLAKE2s`) over the same socket,
> sends one encrypted echo, and measures the RTT. The coordinator selftest covers the rendezvous;
> the core has encrypted-channel and tamper tests; the desktop has a loopback secure-probe test.
> The direct LAN engine is now in the tree: `spaces_direct` creates a second Wintun adapter at
> the member's `10.77.<n>.x/24`, `spaces_forward` carries IPv4 datagrams over the Noise session,
> and the Spaces screen has Start/Stop controls. The forwarding loop has a real bidirectional
> loopback test over the encrypted path; the Wintun adapter has not been created on this machine
> yet because a live sing-box tunnel is running and a second adapter route should not be added
> under it without an explicit test. The next test is two PCs with the tunnel off.
>
> The official Radmin VPN 2.1.4952.2 installer was downloaded and its Authenticode signature
> verified (Famatech Corp., DigiCert, valid). Craft Anything 0.6.1 was downloaded, hash verified,
> and its triage identified the installer as a packed Inno Setup 6.5.2 x86 build. It was not
> installed because the live tunnel would conflict with Radmin's 26.0.0.0/8 adapter and broadcast
> routes. Details are in `notes/RADMIN-INSPECTION.md`. No VPS or Oracle change was made.
>
> **Experimental 0.7.6 pre-release, 26 November 2026:** the desktop Spaces build is published as
> a GitHub pre-release with its own signed `latest.json` asset. The stable feed at the repository
> root and `releases/latest` still point at 0.7.5, and the phone stays on 0.7.5. The build uses a
> plain `0.7.6` version because the app's version parser turns `0.7.6-exp.1` into `0.7.0.1` and
> would never offer it. Release notes and test steps are in `notes/RELEASE-0.7.6.md`.
>
> **Experimental 0.7.7, 26 November 2026:** fixes the Create dialog. The device field now defaults
> to the Windows computer name instead of an empty box with a placeholder, validation names the
> missing field and clears on typing, and an unreachable coordinator is reported with its URL.
> The pre-release carries the signed installer and its own feed; stable stays at 0.7.5. Notes are
> in `notes/RELEASE-0.7.7.md`.
>
> **Experimental 0.7.8, 26 November 2026:** the space no longer rides the main VPN. Activating a
> space does not turn the VPN on, and the card offers **Connect directly** to an online member
> instead of **Connect the VPN**. The direct path carries the space over its own Wintun adapter.
> The relay remains available only when the user connects the main VPN manually. Notes are in
> `notes/RELEASE-0.7.8.md`.
>
> **Experimental 0.7.9, 26 November 2026:** the direct engine now belongs to the app, not the
> Spaces screen. A supervisor starts it for the active space, keeps it alive when the window is
> hidden or the screen is left, and reconnects when another member is online. A manual Stop pauses
> it. The app adds its own inbound UDP/TCP firewall rules for all profiles. The coordinator is
> still the signaling path. Notes are in `notes/RELEASE-0.7.9.md`.
>
> **Experimental 0.7.10, 26 November 2026:** zero-setup invites. The invite carries the coordinator
> address (`qvs2:` wrapper), so the joining PC pastes one code and never runs setx. The Spaces
> header has a coordinator field. The coordinator is deployed publicly on the VPS at
> `https://quotavpn.duckdns.org/spaces`, behind the existing Xray TLS fallback and nginx path, with
> no Oracle change, no new open port and no VPN service restart. Radmin's live data path was
> inspected at the same time and was direct TCP, not relay. Notes are in `notes/RELEASE-0.7.10.md`.
>
> **Experimental 0.7.11, 26 November 2026:** the direct carrier is TCP. The UDP handshake timed out
> between the two test PCs while Radmin's live session was direct TCP (`Connected ... via
> TCP/incoming`). Each member now opens a stable TCP listener, keeps a helper connection to the
> coordinator from the same local port so the NAT mapping stays open, and reports `ip:port` to the
> coordinator. The owner listens, the other member connects, and the Noise session runs over
> framed TCP. The coordinator stores and returns the TCP endpoint. Notes are in
> `notes/RELEASE-0.7.11.md`.
>
> **Experimental 0.7.12, 26 November 2026:** the port test from the VPS showed `51393` and `51394`
> both closed, so Radmin's TCP was hole-punched, not a forwarded listener. The helper is now a
> TCP STUN probe (`stun.nextcloud.com:3478`) that discovers the real mapped endpoint from the
> listener port; the owner punches from that port and accepts, and the joining member connects
> from its own listener port with retries. This is symmetric TCP hole punching, the same shape as
> Radmin. Notes are in `notes/RELEASE-0.7.12.md`.
>
> **Experimental 0.7.13, 26 November 2026:** 0.7.12 bound the STUN helper after the listener, and
> Windows gave the port to the helper (`Bound` and `Established`, no `Listen`). The order is now
> STUN first, then the listener, then the punch; the helper holds the STUN stream without
> rebinding. Notes are in `notes/RELEASE-0.7.13.md`.
>
> **Experimental 0.7.14, 26 November 2026:** 0.7.13 then hit WSAEADDRINUSE (10048) on the third
> socket bound to `51394` (STUN, listener, punch). The STUN socket is now closed before the punch,
> and the joining side drops its listener before connecting, leaving the owner with listener plus
> punch and the joiner with one session socket. Notes are in `notes/RELEASE-0.7.14.md`.

Maintained state of record: `notes/HANDOFF.md` (v0.3.2, Sept 21 2026). This page is a
historical snapshot; where it disagrees with HANDOFF.md, HANDOFF.md wins.

## Works

- Standard mode connects; traffic counts from gamerz/streamerz quota
  (randomized handshake + SNI stamp, verified by packet capture: SNI on
  the wire, WE app bucket moves).
- Game (Hysteria2) connects with working traffic; the PC's WireGuard is
  AmneziaWG (obfuscated) since WE's DPI drops plain handshakes - it connects
  and carries traffic from Egypt. Both count from general quota (ISP reads
  SNI off TCP only). The phone's WireGuard cannot speak AWG yet (stock
  libbox).
- Cards generate / copy / revoke; same `vless://` links import into
  NekoBox/v2rayNG and count correctly there.
- Cards self-heal: connect re-registers any local card missing on the
  server (`qc-list` + idempotent add).
- Per-app VPN (all / only-these / all-but-these), quick-settings tile,
  kill-switch path via system Always-on VPN (+ Block connections).
- Speed test runs the official Ookla CLI on full runs (live graph, results
  as they are measured), with the built-in measurer for single tiles and
  fallback.
- Windows ships as an NSIS installer; closing the window hides to the
  tray (Show / Check for updates / Quit), and updates install from
  GitHub by themselves (signed feed, auto-restart).
- PC app redesigned as a native Windows app (Fluent spacing, Segoe UI
  Variable, connection card with live traffic, Settings-style rows),
  same theme as the phone.
- Per-app routing on PC: all / only-these / all-except, matched by
  process in the engine; config shapes locked by cargo tests.
- Motion: the connection dial sits centred and slides left the moment
  Connect is pressed, with the status/traffic panel revealing alongside
  it. Motion survives the OS "animation effects off" setting (that
  setting only stands down the endless decorative pulses).
- Phone app has its own update check (Settings, Updates): checks
  GitHub, downloads the APK, opens the system installer for one
  confirm tap. Same version number as the PC build.
- Look and feel keyed to the app icon on both platforms: near-black
  navy surfaces, royal blue accent, off-white text, muted semantics
  (no neon anywhere). Desktop hero choreography: the dial starts
  centred and slides left on connect while the status, profile, live
  traffic and Session/IP/Server tiles reveal in sequence. Cards are
  uniform, one colour, kind shown as a badge. Audited against the Web
  Interface Guidelines (labels, aria-live, skip link, focus rings,
  contrast, overscroll, list performance).
- Server page in 4 sections (Status/Connection/General/Protection),
  centered pickers, no content trapped behind the tab bar, EN + AR RTL.
- Honest labels: Game/WireGuard warn they count general.
- Server: Xray + Hy2 + WG all live; DuckDNS self-updates every 5 min;
  firewall rule order fixed; setup scripts persist it.

## Known issues

- `embed/qc-agent.sh` in repo is older than the live box (live also
  serves `qc-list`, `qc-hy2-pass`, `qc-wg-pub/add/del`). Reconcile
  before a fresh-server rebuild from this repo alone.
- "Connected" = TUN up, not handshake-verified. A dead UDP path still
  shows Connected with no traffic.
- Upload hosts rot: litterbox currently WAF-blocks APK uploads, uguu
  rejects `.apk`, bashupload/gofile unreachable from here; tmpfiles.org
  works but expires fast. Revisit per release.
- `libbox.aar` is a pinned binary: sing-box schema moves under it
  (1.13 killed the wireguard outbound). Any engine bump = re-validate
  all three configs with `sing-box check` first.
- WE filters UDP/51820 on some lines (hence the UDP/53 door). New
  networks may need the same treatment.

## Roadmap (proposed)

- [ ] Handshake-aware status (confirm WG/Hy2 session before "Connected").
- [ ] Reconcile `qc-agent.sh` v2 into repo; single-script fresh rebuild.
- [ ] DNS-split experiment: direct DNS for the SNI domain only, test if
  quota classification changes.
- [ ] TCP-based low-latency mode (if Game must ever count quota).
- [ ] Stable release hosting for APKs (own server URL or GitHub Releases).
- [ ] Version display in-app (tell builds apart on-device).
- [ ] iOS / desktop-mobile parity: out of scope until Android is bored.
