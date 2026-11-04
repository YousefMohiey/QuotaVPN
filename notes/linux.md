# Linux

The same app, with the parts that make sense on Linux and none of the parts
that do not.

## What works

- Connect and disconnect, through the same sing-box engine the Windows build
  uses, on a real TUN device.
- The kill switch, as a real allow list. This is stronger than what Windows
  can express through its own firewall: nftables drops every packet leaving
  the machine except loopback, the tunnel interface, the server address the
  tunnel connects to, and replies to flows that already exist.
- App routing, in allow mode and block mode. Processes are matched by their
  executable path, read from `/proc`, so the picker lists what is actually
  running rather than a curated list.
- Cards, quota, history, settings, the tray.

## What is not there

- The network screen. It depends on a Windows packet capture driver; on Linux
  the page is not offered at all, rather than shown empty.
- The speed test. It ships the Windows Ookla binary.
- The Valorant voice page. Windows game, Windows helper.

The sidebar leaves those doors out when `platform` reports `linux`.

## Privileges

The tunnel needs `CAP_NET_ADMIN`. The app stays a normal user and starts the
engine once per session through `pkexec`, which asks for the password once.
There is no scheduled task on Linux and nothing is installed as root except
the app itself.

The engine is started as:

```
pkexec env HOME=<your home> /path/to/quotavpn --run-engine
```

`HOME` is carried across so root writes its pid and log into the same folder
the app watches. Closing the app writes a stop marker, the engine sees it,
tears the tunnel down and lifts the kill switch.

## Engine

`sing-box` is downloaded once on first connect into
`~/.local/share/quotacards/engine/sing-box`, version stamped, and replaced
automatically when the pinned version changes. The `.deb` declares `curl`
and `pkexec` as dependencies.

## Kill switch behaviour

It is armed by the toggle in Settings. When armed:

- Applied before the engine starts, so there is no window where traffic
  leaves unprotected.
- Kept if the engine dies on its own, which is the whole point.
- Lifted on a clean stop, on release from Settings (via `pkexec`), or on the
  next successful start.

If the machine loses power mid-session the rules go with it, because nftables
rules are not persistent by default. That is the ordinary behaviour on Linux
and it means the block cannot outlive a reboot.

## Building

CI builds the `.deb` and the AppImage on an Ubuntu runner:

```
.github/workflows/linux-build.yml
```

Locally, on any machine with the Tauri dependencies:

```
cd desktop/ui-next && npm ci && npm run build
cd desktop && npx @tauri-apps/cli@2 build --bundles deb,appimage
```

## What is proven and what is not

Proven here: the source compiles for Linux and the packages are produced.

Not proven here: a running tunnel, because there is no Linux machine on the
build box. The first real session on Linux is the test that matters.
