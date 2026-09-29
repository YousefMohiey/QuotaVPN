![QuotaVPN](assets/banner.svg?v=6)

<h1 align="center">QuotaVPN</h1>

<p align="center">Windows and Android. Point a connection at the data package you paid for, and watch it bill there.</p>

<p align="center">
  <a href="https://github.com/YousefMohiey/QuotaVPN/releases/latest"><b>Download</b></a>
  &nbsp;&nbsp;&nbsp;
  <a href="https://quotavpn.app/">Website</a>
  &nbsp;&nbsp;&nbsp;
  <a href="https://quotavpn.app/app/">Windows demo</a>
  &nbsp;&nbsp;&nbsp;
  <a href="https://quotavpn.app/phone/">Android demo</a>
</p>

<p align="center">
  <a href="https://github.com/YousefMohiey/QuotaVPN/releases/latest"><img src="https://img.shields.io/github/v/release/YousefMohiey/QuotaVPN?color=5cb28e&labelColor=0b0f18" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011-0b0f18?logo=windows&logoColor=white" alt="Windows 10 and 11">
  <img src="https://img.shields.io/badge/Android-0b0f18?logo=android&logoColor=3ddc84" alt="Android">
  <img src="https://img.shields.io/badge/UI-English%20%2B%20Arabic-0b0f18" alt="English and Arabic">
  <img src="https://img.shields.io/badge/license-MIT-0b0f18" alt="MIT license">
</p>

## Why it exists

Your ISP decides which quota a connection bills to by reading the server name in the TLS handshake, at the very start of the connection. QuotaVPN puts the server name of the package you chose into that handshake, so the session bills where you want it: your gaming package, your streaming package, or the general pool. That is the whole idea.

## Install

| | File | Notes |
|---|---|---|
| **Windows 10 / 11** | `QuotaVPN_<version>_x64-setup.exe` | Installs for the current user, no admin rights. Updates itself from then on. |
| **Android** | `QuotaVPN-mobile-signed.apk` | Sideload once. Updates itself from the same releases page. |

Both come from the [latest release](https://github.com/YousefMohiey/QuotaVPN/releases/latest). If you would rather look before installing, the website runs both interfaces live, built from the same source: [the Windows build](https://quotavpn.app/app/) and [the Android build](https://quotavpn.app/phone/). Nothing is routed and nothing is installed.

The installers are not code-signed yet, so Windows SmartScreen will warn about an unknown publisher the first time. Pick **More info**, then **Run anyway**. Everything that warning is about is auditable, because this repository is the source the builds came from.

## The app

- **Two packages, Gamerz and Streamerz.** Each carries its own server list (EA, Riot, Steam, YouTube, Meta, Prime Video and more) plus any custom name you add.
- **Three transports, labelled honestly.** Standard keeps the package quota; WireGuard and Hysteria2 spend general quota and are the raw-speed options.
- **Per-app routing** on both platforms: the whole device, only these apps, or everything except them.
- **A speed screen** with ping, download and upload against the server, and history.
- **English and Arabic** in both apps. The layout keeps its shape; nothing mirrors.
- **Signed self-updates.** The app fetches the signed installer and applies it for you.

## Screens

| Connected | Valorant voice chat | Server list | Arabic |
|---|---|---|---|
| ![The home screen, connected](assets/app-home.png) | ![The Valorant voice chat helper](assets/app-valorant-voice.png) | ![The server list](assets/app-servers.png) | ![The Arabic interface](assets/app-arabic.png) |

## Details

<details>
<summary><b>How each transport counts</b></summary>

| Mode | Transport | Counts from |
|------|-----------|-------------|
| Standard | VLESS on TCP 443 with the package server name | Your package (Gamerz / Streamerz) |
| WireGuard | Raw UDP, no TLS handshake | General quota |
| Hysteria2 | UDP 443 with the same server name | General quota (ISPs read the name off TCP only) |

The engine is sing-box: on Windows it runs with wintun inside the app, on Android through `VpnService` with libbox. Traffic goes through the tunnel or nowhere.

</details>

<details>
<summary><b>Run your own server</b></summary>

You bring Ubuntu 22.04 or newer (an Oracle Always Free instance is enough). Two scripts in `embed/`:

1. `qc-fresh.sh` installs Xray (VLESS on TCP/443) plus the restricted `qc-agent` key.
2. `qc-net.sh` adds Hysteria2 (UDP/443), WireGuard (UDP/51820, with a UDP/53 fallback for ISPs that filter the default port) and the helper scripts.

Open these on the security list of the instance's own subnet:

| Direction | Protocol | Port | Used by |
|-----------|----------|------|---------|
| In | TCP | 22 | setup SSH |
| In | TCP | 443 | Standard |
| In | UDP | 443 | Hysteria2 |
| In | UDP | 51820 | WireGuard |
| In | UDP | 53 | WireGuard fallback |

Point a DuckDNS (or any) name at the box and refresh it from a cron; the app resolves it on every connect. SSH stays key-only, and the key the apps carry runs through a forced command that can only register a device, revoke a device, manage its WireGuard peers, and hand it its transport credentials. It cannot open a shell.

</details>

<details>
<summary><b>Repository map</b></summary>

- `desktop/ui-next/` the Windows UI (React 19 + Tailwind v4, English and Arabic)
- `desktop/src-tauri/` the Windows backend: whole-device sing-box with wintun, process list, tray, updater
- `android/tauri-app/` the Android app: the shared Rust core with the phone UI
- `android/tauri-plugin-qctunnel/` the Kotlin `VpnService`, libbox engine, Quick Settings tile, per-app picker
- `src/` the Rust core both apps share
- `embed/` the server scripts and the agent (`*.pem` is gitignored and never committed)
- `docs/` the published site: the landing page and the two live demos
- `notes/` the long-form documentation
- `tools/` the build and release scripts

</details>

<details>
<summary><b>Build from source</b></summary>

Rust stable and Node cover the desktop build; the APK also needs Android SDK 36, NDK 28 and JDK 23.

- Windows: `npm run build` in `desktop/ui-next`, then a Tauri build with the signing environment set.
- Android: `bash tools/build-apk.sh`.
- A full release across both platforms and the site: `notes/HANDOFF.md` has the sequence.

</details>

## Docs

[`notes/HANDOFF.md`](notes/HANDOFF.md) the full system map, [`notes/ARCHITECTURE.md`](notes/ARCHITECTURE.md) file by file, [`notes/STATUS.md`](notes/STATUS.md) what works and what comes next, [`AGENTS.md`](AGENTS.md) the working rules, [`SECURITY.md`](SECURITY.md) how to report a vulnerability privately.

## License

MIT, see [LICENSE](LICENSE). Built by Yousef Mohiey.
