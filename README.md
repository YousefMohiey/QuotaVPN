![QuotaVPN](assets/banner.svg?v=6)

# QuotaVPN

A personal VPN for Windows and Android that makes your traffic ride the data package you pick: gaming, streaming, or your general quota.

[![Latest release](https://img.shields.io/github/v/release/YousefMohiey/QuotaVPN?color=5cb28e&labelColor=0b0f18)](https://github.com/YousefMohiey/QuotaVPN/releases/latest)
![Windows 10 / 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0b0f18?logo=windows&logoColor=white)
![Android](https://img.shields.io/badge/Android-0b0f18?logo=android&logoColor=3ddc84)
![UI: English + Arabic](https://img.shields.io/badge/UI-English%20%2B%20Arabic-0b0f18)
![License: MIT](https://img.shields.io/badge/license-MIT-0b0f18)

**Website:** https://quotavpn.app/ **Download:** [latest release](https://github.com/YousefMohiey/QuotaVPN/releases/latest)

## The problem it solves

Your ISP decides which quota a connection bills to by reading the server name in the TLS handshake, at the very start of the connection. QuotaVPN puts the server name of the package you chose into that handshake, so the session bills where you want it: your gaming package, your streaming package, or the general pool.

## What you get

- **Two packages, Gamerz and Streamerz.** Each carries its own server list (EA, Riot, Steam, YouTube, Meta, Prime Video and more) plus any custom server you add.
- **Three transports, labelled honestly.** Standard keeps the package quota; WireGuard and Hysteria2 spend general quota and are your raw-speed options.
- **Per-app routing**, on both platforms: whole device, only these apps, or everything except them.
- **Speed screen** with ping, download and upload against the server, plus history.
- **English and Arabic** in both apps. The layout keeps its shape; nothing mirrors.
- **Signed self-updates.** The app downloads the signed installer and applies it for you.

## Download

| Platform | File | Notes |
|----------|------|-------|
| Windows 10/11 | `QuotaVPN_<version>_x64-setup.exe` | Installs for the current user, no admin rights. Updates itself afterwards. |
| Android | `QuotaVPN-mobile-signed.apk` | Sideload once. Updates itself from the same releases page. |

The installers are not code-signed, so Windows SmartScreen warns about an unknown publisher the first time you run one: pick **More info**, then **Run anyway**. Everything that warning is about is auditable, because this repository is the source those builds came from.

## Try it without installing

The website runs both interfaces live, built from the same source the apps ship:

- Windows interface: https://quotavpn.app/app/
- Android interface: https://quotavpn.app/phone/

Press connect, move between screens, run a speed test. Nothing is routed and nothing is installed.

## Screenshots

| Connected | Speed test | Profiles | Arabic |
|---|---|---|---|
| ![Windows app, connected](assets/app-home.png) | ![Speed screen](assets/app-speed.png) | ![Profile tiles](assets/app-valorant.png) | ![Arabic interface](assets/app-arabic.png) |

## How traffic counts

| Mode | Transport | Counts from |
|------|-----------|-------------|
| Standard | VLESS on TCP 443 with the package server name | Your package (Gamerz / Streamerz) |
| WireGuard | Raw UDP, no TLS handshake | General quota |
| Hysteria2 | UDP 443 with the same server name | General quota (ISPs read the name off TCP only) |

The engine is sing-box: on Windows it runs with wintun inside the app, on Android it runs through `VpnService` with libbox. Traffic goes through the tunnel or nowhere.

## Self-hosting the server

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

## Project layout

- `desktop/ui-next/` the Windows UI (React 19 + Tailwind v4, English and Arabic)
- `desktop/src-tauri/` the Windows backend: whole-device sing-box with wintun, process list, tray, updater
- `android/tauri-app/` the Android app: the shared Rust core with the phone UI
- `android/tauri-plugin-qctunnel/` the Kotlin `VpnService`, libbox engine, Quick Settings tile, per-app picker
- `src/` the Rust core both apps share
- `embed/` the server scripts and the agent (`*.pem` is gitignored and never committed)
- `docs/` the published site: the landing page and the two live demos
- `notes/` the long-form documentation
- `tools/` build and release scripts
- `assets/` README and site artwork

## Build from source

Rust stable and Node are enough for the desktop. The APK also needs Android SDK 36, NDK 28 and JDK 23.

- Windows: `npm run build` in `desktop/ui-next`, then a Tauri build with the signing environment set.
- Android: `bash tools/build-apk.sh`.
- A full release, both platforms and the published site: `notes/HANDOFF.md` has the sequence.

## Documentation

- `notes/HANDOFF.md` the full system map: flows, code map, build and release, traps
- `notes/ARCHITECTURE.md` how it is built, file by file
- `notes/STATUS.md` what works, known issues, roadmap
- `AGENTS.md` the short working rules, for contributors and AI assistants
- `SECURITY.md` how to report a vulnerability privately

## License

MIT, see [LICENSE](LICENSE). Built by Yousef Mohiey.
