![QuotaVPN](assets/banner.svg?v=7)

<h1 align="center">QuotaVPN</h1>

<p align="center">A VPN for Windows and Android that bills your traffic to the data package you pick. Gaming package, streaming package, or general quota: you choose per connection.</p>

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
  <img src="https://img.shields.io/badge/UI-English%20%2B%20Arabic-0b0f18" alt="English and Arabic interface">
  <img src="https://img.shields.io/badge/license-MIT-0b0f18" alt="MIT license">
</p>

## The idea in thirty seconds

Some ISPs sell separate quota buckets: a gaming package, a streaming package, and a general pool. They sort each connection into a bucket by reading the server name in the TLS handshake at the start of the connection.

QuotaVPN puts the server name of the package you chose into that handshake. Pick a Gamerz card and the session bills to your gaming package. Pick a Streamerz card and it bills to streaming. That is the whole product.

## Screens

<img src="assets/app-home.png" alt="QuotaVPN on Windows, connected with the Gamerz package" width="860">

| Valorant voice chat | Server list | Arabic interface |
|---|---|---|
| ![Valorant voice chat helper](assets/app-valorant-voice.png) | ![Server list](assets/app-servers.png) | ![Arabic interface](assets/app-arabic.png) |

Windows app shown. The Android app uses the same theme and the same two packages; try it live in the [Android demo](https://quotavpn.app/phone/).

## What the app does

- **Two packages, Gamerz and Streamerz.** Each has its own server list (EA, Riot, Steam, YouTube, Meta, Prime Video and more), plus any custom server name you add.
- **Three transports, labelled honestly.** Standard keeps the package quota. WireGuard and Hysteria2 spend general quota and are the raw-speed options.
- **Per-app routing on both platforms.** Whole device, only these apps, or everything except them. On Windows it matches running processes; on Android it uses the system picker.
- **Valorant voice chat helper.** If voice chat is blocked in your region, one switch moves the voice traffic through the VPN and leaves your match connection direct.
- **Speed screen.** Ping, download and upload against your own server, with history. Full runs use the official Ookla CLI; single tiles use the built-in measurer.
- **English and Arabic, both apps.** Same layout in both languages; nothing mirrors.
- **Self-updating.** Windows installs signed updates from the releases feed by itself. Android checks from Settings, downloads the APK and opens the system installer for one confirm tap.

## How each transport counts

| Mode | Transport | Bills to |
|------|-----------|----------|
| Standard | VLESS on TCP 443, with the package server name in the handshake | Your package (Gamerz / Streamerz) |
| WireGuard | Raw UDP, no TLS handshake for the ISP to read | General quota |
| Hysteria2 | UDP 443 with the same server name | General quota (ISPs read the name off TCP only) |

The engine is sing-box: on Windows it runs with wintun inside the app, on Android through `VpnService` with libbox. Traffic goes through the tunnel or nowhere.

## Install

**Windows**

1. Download the setup from the [latest release](https://github.com/YousefMohiey/QuotaVPN/releases/latest).
2. Run it. It installs for the current user, no admin rights needed.
3. Open QuotaVPN and connect.

**Android**

1. Download the APK from the [latest release](https://github.com/YousefMohiey/QuotaVPN/releases/latest).
2. Open it to install (your phone will ask you to allow this install).
3. Open QuotaVPN and connect.

Two honest notes. The installers are not code-signed yet, so Windows SmartScreen warns about an unknown publisher on first run: choose **More info**, then **Run anyway**. And "Connected" in the app means the tunnel device is up; like most clients here, it does not yet verify the handshake completed, so a dead UDP path can show Connected with no traffic moving.

## Run your own server

QuotaVPN can point at your own box instead of the default one. You bring Ubuntu 22.04 or newer (an Oracle Always Free instance is enough) and two scripts in `embed/` do the work: `qc-fresh.sh` installs Xray (VLESS on TCP 443) plus the restricted agent key, and `qc-net.sh` adds Hysteria2 (UDP 443), WireGuard (UDP 51820, with a UDP 53 redirect for ISPs that filter the default port) and the helpers.

| Direction | Protocol | Port | Used by |
|-----------|----------|------|---------|
| In | TCP | 22 | setup SSH |
| In | TCP | 443 | Standard |
| In | UDP | 443 | Hysteria2 |
| In | UDP | 51820 | WireGuard |
| In | UDP | 53 | WireGuard fallback |

Point a DuckDNS (or any) name at the box and refresh it from a cron; the app resolves it on every connect. SSH stays key-only. Full sequence, traps included: [`notes/HANDOFF.md`](notes/HANDOFF.md).

## Security, plainly

- The code is open source and this repository is what the published builds come from. No credentials, tokens or private keys live in it; `*.pem` is gitignored and the history is clean.
- The key the apps carry cannot open a shell. It runs through a forced command that can only register a device, revoke a device, manage its WireGuard peers, and hand it its transport credentials.
- Windows updates arrive over a signed feed the app verifies before installing.
- Limits, stated above: installers are unsigned (expect SmartScreen), and Connected means tunnel-up rather than handshake-verified. Found something? See [SECURITY.md](SECURITY.md); please report privately, not via a public issue.

## Project map

```text
Windows UI (React)
   |
Tauri backend (Rust)
   |
Shared Rust core (src/)
   |
sing-box + wintun (tunnel)

Android UI (web)
   |
Tauri backend (Rust)
   |
Shared Rust core (src/)
   |
VpnService (Kotlin) + libbox (tunnel)
```

- `desktop/ui-next/` Windows UI (React 19 + Tailwind v4, EN + AR)
- `desktop/src-tauri/` Windows backend: engine, process list, tray, updater
- `android/tauri-app/` Android app: shared Rust core with the phone UI
- `android/tauri-plugin-qctunnel/` Kotlin `VpnService`, libbox engine, Quick Settings tile, per-app picker
- `src/` Rust core both apps share (config, server, SSH, updater, VPN)
- `embed/` server scripts and the agent
- `docs/` the published site: landing page and the two live demos
- `notes/` long-form documentation
- `tools/` build and release scripts

## Build from source

Rust stable and Node cover the desktop build. The APK also needs Android SDK 36, NDK 28 and JDK 23.

- Windows: `npm run build` in `desktop/ui-next`, then a Tauri build with the signing environment set.
- Android: `bash tools/build-apk.sh`.
- Full release across both platforms and the site: [`notes/HANDOFF.md`](notes/HANDOFF.md) has the sequence.

## Docs

[`notes/HANDOFF.md`](notes/HANDOFF.md) is the full system map, [`notes/ARCHITECTURE.md`](notes/ARCHITECTURE.md) goes file by file, [`notes/STATUS.md`](notes/STATUS.md) tracks what works and what is next, [`AGENTS.md`](AGENTS.md) holds the working rules, [`SECURITY.md`](SECURITY.md) covers private vulnerability reports.

## License

MIT, see [LICENSE](LICENSE). Built by Yousef Mohiey.
