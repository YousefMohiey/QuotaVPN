# QuotaVPN security audit

Run 2026-11-15, against `master` at `edd1f84` (the tree after the 0.6.10/0.6.11
revert). Read-only: nothing in the repository was modified and no target code was
executed. Four independent reviewers covered secrets/supply chain, network and
credentials, the LAN/elevated surface, and the local/update surface. Every finding
below was re-checked against source by the lead before it was written here.

Severity scale: critical = unauthenticated or trivial path to admin code execution
or full credential theft. high = defeats an explicit control with real
consequences. medium = real boundary break, narrow blast radius or unusual
preconditions. low = minimal effect. nv = needs validation, not confirmed.

## Fix status (updated after the repair pass)

| finding | status |
|---|---|
| C-1 app elevated from a user-writable directory | **fixed for the install path** (`perMachine`); the window still runs elevated, which is the next step |
| C-2 engine, wintun, Ookla executed with no check | **fixed**: SHA-256 pinned in source, verified before extract |
| C-3 WinDivert DLL loaded unverified | **fixed**: hash checked before `LoadLibrary`; dev-only candidate paths |
| H-1 SSH host key never verified | **fixed**: trust on first use, then pinned |
| H-2 TLS disabled on both transports | **not fixed**: needs the server to expose a cert to pin |
| H-3 signing key exposed to build-time code | **fixed**: secrets scoped to their steps, CLI pinned |
| H-4 one embedded key, server ignores the device secret | **not fixed**: needs a decision about the live agent |
| H-5 a file write drives the elevated packet loop | **fixed**: addresses parsed, lists and TTL bounded |
| M-1 feed rollback | **not fixed**: needs a signed feed |
| M-2 `--clobber` replaces published bytes | **fixed** |
| M-3 recovery record forges ARP | **fixed**: record must match the live neighbour table |
| M-4 uninstall keeps privileged tasks | **fixed** |
| M-5 elevated script in a user-writable directory | **fixed**: encoded command, no file |
| M-6 router login over an unverified certificate | **not fixed**: needs certificate pinning for the router |
| M-7 `app_icons` arbitrary path | **fixed** |
| M-8 `open_page` command injection | **fixed** |
| M-9 card UUID into a root heredoc | **fixed**: validated unconditionally |
| F-08 DHCP names unbounded | **fixed** |

Shipped as v0.6.12. 55 tests pass; the Windows paths type-check for the target.

## The short answer

**There is no backdoor, no phone-home, no exfiltration, and no evidence of
compromise.** No private key, token, keystore or password was ever committed, in
the working tree or anywhere in the 2,786 objects of the git object database,
including unreachable blobs. The updater is signature-gated with a public key that
is compiled in and has never changed across 90 revisions of the config. The
Android tunnel service is not exported. No secret reaches any log.

**The design, however, has holes.** Three of them let someone who already has a
non-administrator foothold on the PC become Administrator. One lets whoever can
replace a GitHub release asset run code as Administrator on every install. Two
mean the tunnel itself provides no authentication against a network attacker,
which matters precisely in the environment this app was built for.

Nothing found suggests anyone has used any of it. These are latent weaknesses, not
an incident.

## Critical

### C-1 The app runs elevated from a directory the user can write

- `res/quotacards.manifest:18` requests `requireAdministrator`, so the whole app,
  WebView reviewed code and every command included, runs at high integrity.
- `desktop/src-tauri/tauri.conf.json` `bundle.windows.nsis.installMode` is
  `currentUser`, so the executable lives under the user's own profile.
- `src/vpn.rs:1671` registers a scheduled task whose `<Command>` is that same
  executable, at `HighestAvailable` (`:1650`), started by `schtasks /Run`
  (`src/vpn.rs:1863-1871`).

Consequence: any process running as that user replaces the executable, then starts
the task. `schtasks /Run` on your own task needs no elevation and shows no prompt,
so a medium-integrity attacker gets Administrator. The user does not have to
approve anything.

Smallest fix: install `perMachine` into an admin-owned directory, register the
tasks against that path, and drop the app manifest to `asInvoker`, leaving
elevation to the helper tasks alone.

### C-2 The tunnel engine is downloaded and executed as Administrator with no integrity check

- `src/vpn.rs:19-22` points at a release asset in this repository
  (`engine-awg-1141.zip`).
- `src/vpn.rs:254-278` and `:282-305` fetch it with `curl`, extract with `tar`,
  rename it to `sing-box.exe`, and run it. There is no hash, no signature and no
  size check anywhere on that path. Wintun is handled the same way.
- The app that runs it is elevated (C-1).

The same pattern reaches further than the engine: `desktop/src-tauri/src/ookla.rs:27,69-92`
downloads an Ookla `speedtest.exe` the same way and runs it at `:332`, with no
verification either. One unverified door, three executables behind it.

Consequence: whoever can replace that release asset, serve the wintun archive, or
sit on the download path gets Administrator code execution on every client that
does not already have a cached matching engine. The carefully signed updater does
not cover any of it, because these arrive through a different door.

Smallest fix: put the sha256 of each archive in the source, verify before the
rename, fail closed. Longer term, publish the engine under the same minisign key
the updater already uses and verify with the pinned public key.

### C-3 The WinDivert DLL is loaded into an elevated process from a user-writable path, unverified

- `src/windivert.rs:197` verifies only `WinDivert64.sys`. The DLL is never checked
  before `libloading::Library::new` at `:296`.
- `src/windivert.rs:235-267` tries, in order: the executable's own directory (a
  per-user install, see C-1), four parent directories' `res/windivert`, the engine
  directory under `%LOCALAPPDATA%`, the current working directory, and finally the
  bare name.
- The process loading it is elevated: `--lan-pipeline` runs under the task, and
  `pipeline_ready` is called by the Network screen.

Consequence: dropping a `WinDivert.dll` into `%LOCALAPPDATA%\quotacards\engine\windivert`
yields Administrator code execution the next time the LAN features load. A DLL's
`DllMain` runs inside `LoadLibrary`, so no race is needed.

Smallest fix: verify the DLL's Authenticode status and signer before loading, pin
an expected hash, and remove the `res/windivert`, CWD and bare-name candidates from
release builds.

This one, and C-2's sibling in `windivert::install`, are mine: the driver-fetch
design I added made an already-elevated process load bytes from a path the user
controls.

## High

### H-1 The SSH host key is never verified

`src/ssh.rs:14-19` returns `Ok(true)` for every key, and the parameter is genuinely
unused. There is no known_hosts, no fingerprint store, no pin.

The SSH private key is not transmitted (publickey auth only signs), but everything
else crossing that connection is readable and modifiable by an impostor: every card
UUID, the per-device secret, the shared Hysteria2 password, each card's WireGuard
private key, and every provisioning response the app trusts.

Smallest fix: store the server's host key fingerprint in the config on first
successful setup and compare it in `check_server_key`, returning `Ok(false)` on a
mismatch with an explicit re-trust flow.

### H-2 TLS is disabled on both transports, so the tunnel authenticates nothing

`src/vpn.rs:762` and `:818` set `"insecure": true` for Hysteria2 and VLESS; the
share links hardcode `allowInsecure=1`. The server certificate is self-signed for
ten years with CN `ea.com` (`src/server.rs:21-24`) and nothing pins it.

Consequence: an active network attacker completes the handshake with any
certificate, learns the VLESS UUID or Hysteria2 password, and reads and edits
everything the tunnel carries. The product exists to survive hostile networks, so
this is the finding that most undermines its purpose.

Smallest fix: pin the certificate's `certificate_public_key_sha256` in the card and
set `insecure: false`; rotating a pin becomes an explicit operation.

### H-3 The release signing key is exposed to build-time code

`.github/workflows/signed-release.yml:36-39` puts `TAURI_SIGNING_PRIVATE_KEY`, its
password and the SignPath token in job-level `env:`, so every step inherits them.
The key-bearing job then runs `npx --yes @tauri-apps/cli@2`, a floating major
range resolved from the registry on every run (`:63-69`, `:142`), and
`actions/checkout@v7` persists the write-scoped `GITHUB_TOKEN` (`:41`, `:24-26`).

Consequence: whoever publishes a new `@tauri-apps/cli` 2.x, or moves a mutable
action tag, inherits the ability to sign and publish updates as this project.

Smallest fix: scope those three secrets to the two signing steps, pin the CLI to an
exact version, `persist-credentials: false`, `npm ci --ignore-scripts`, and put the
job behind an `environment:` with a required reviewer.

### H-4 One embedded key is a tenant-wide credential and the server ignores the per-device secret

The private key is compiled into the app by design (`build.rs`, `src/config.rs:4`).
The client appends a device secret to every command (`src/server.rs:164-166`), but
the checked-in agent authorizes on the SSH key alone and explicitly rejects
commands carrying an argument where none is allowed
(`embed/qc-net.sh:203-218`) - which is every command the app sends. So the
repository's agent cannot be the live one, and the "per-device" binding the client
believes in is not implemented here.

Consequence if the live agent behaves like the repository's: anyone holding a copy
of the app can enumerate all card UUIDs, read the shared Hysteria2 password, and
mint WireGuard peers for arbitrary UUIDs.

Needs validation: what the live `/usr/local/bin/qc-agent` actually checks.
`notes/STATUS.md` already says it is newer than the repo and the command set does
not line up.

### H-5 A plain file write drives the elevated packet loop

`src/lan_forward.rs:48-62` reads `%LOCALAPPDATA%\quotacards\engine\lan\policy.json`,
and the elevated `--lan-pipeline` obeys it: which addresses the driver filters,
which are blocked, their rate limits, and how long the session lives.

Consequence: an unprivileged process writes JSON and the elevated loop silently
blackholes or throttles any forwarded flow, and keeps itself alive past the app's
own limit. It cannot redirect or rewrite traffic, and the host's own packets never
reach that layer, so this is filtering and denial of service, not interception.

Smallest fix: parse the addresses as `Ipv4Addr` rather than interpolating them into
a filter string, cap the list and the TTL - and since the app is already elevated
(C-1), run the loop in-process instead of across a file.

## Medium

- **M-1 Rollback.** `latest.json` is unsigned and its `version` is trusted
  (`desktop/src-tauri/src/lib.rs:2640`, `:2697`); the signature covers installer
  bytes only. A forged feed can name an older, validly signed installer and
  downgrade a client. Fix: sign the feed, or bind the version inside the artifact.
- **M-2 Release asset replacement.** `signed-release.yml:157-162` uploads with
  `--clobber`, so anyone with repo write access can re-run a tag and silently
  replace existing installer bytes. Installed clients are not pushed it (both
  gates require a strictly higher version), so this affects new downloads only.
  Fix: drop `--clobber`.
- **M-3 The recovery record forges ARP on startup.** `src/lan_route.rs:152-156`
  reads `%LOCALAPPDATA%\quotavpn\lan_route_intent.txt` and `recover()` sends
  `arp_release` frames built from it on the next launch, with no check that the
  recorded router MAC still matches the neighbour table. A crafted file makes the
  elevated app poison an arbitrary LAN device. Fix: keep the record in admin-owned
  state and re-verify the MAC before sending.
- **M-4 Uninstall leaves privileged tasks based on a user-writable marker.**
  `desktop/src-tauri/windows/nsis-hooks.nsh:122-125` keeps the helper tasks when
  `%LOCALAPPDATA%\quotacards\updating` exists. Combined with C-1 that is
  persistence past uninstall. Fix: use a signal the user cannot forge, and delete
  the tasks unconditionally on a real uninstall.
- **M-5 Elevated scripts written to a user-writable directory.**
  `src/vpn.rs:1789` writes `uninstall-clean.ps1` into the engine directory and runs
  it elevated; `src/lan_route.rs:432` and `src/windivert.rs:185` fetch, verify and
  execute from `%TEMP%`/the engine directory without holding the verified handle.
  All are TOCTOU windows. Fix: verify and execute the same handle, or stage under
  admin-owned state.
- **M-6 Router login is sent to a gateway whose certificate is not checked.**
  `desktop/src-tauri/src/router_huawei.rs:40-48` uses
  `danger_accept_invalid_certs(true)`, and the admin password goes out
  base64-encoded (encoding, not protection). A LAN attacker impersonating the
  router captures the credentials. Fix: pin or verify, and stop calling base64
  "encryption" in the copy.
- **M-7 `app_icons` accepts an arbitrary path.** `desktop/src-tauri/src/lib.rs:1615`
  uses a `pkg` containing `/` or `\` verbatim as a path, which reaches
  `SHGetFileInfoW` at `:1757`. A `\\attacker\share\x.ico` value coerces SMB
  authentication from the elevated process. Fix: resolve names only, reject UNC and
  device paths.
- **M-8 `open_page` is a cmd.exe injection (latent).**
  `desktop/src-tauri/src/lib.rs` checks only a `https://` prefix, then runs
  `cmd /C start "" <url>`; `&calc.exe` in the URL executes. No UI caller today.
  Fix: reject anything outside a strict URL grammar, or use `ShellExecute`.
- **M-9 Card UUID reaches a root Python heredoc unvalidated (latent).**
  `src/server.rs:179` and `:190` substitute the UUID into `uuidv = "..."` inside a
  script run with `sudo python3`; `valid_uuid` runs only on the embedded-key path
  (`:174`, `:185`), and `is_embed_key` is false in any build without the pem, which
  is every CI build. A quote in the UUID breaks out and runs as root on the
  server. Nothing in the current UI calls import, so it is not reachable yet.
  Fix: validate unconditionally and pass the UUID as argv instead of substituting.

## Low and informational

- DHCP name harvesting accepts unauthenticated LAN input with no size bound
  (`src/dhcp_names.rs`): a rogue device can impersonate any roster name or grow the
  cache without limit.
- ARP claim has no ownership check: `lan_route_device` skips the protected-device
  rules that `lan_start` enforces, so any IP/MAC pair can be claimed.
- The vendored WinDivert pair cannot be proven byte-identical to upstream offline.
  The `.sys` signature is valid (Sectigo EV, timestamped); the DLL carries no
  signature at all. Verify the hashes against the official 2.2.2-A archive once.
- `QC_FEED_URL` repoints the updater at runtime; signature checking still holds,
  so it is not an install primitive.
- `UiState` serialises per-card WireGuard private keys into the webview, and the
  CSP allows `connect-src https: http:`.
- The documented same-version hotfix path does not exist: `src/updater.rs:18` has no
  production caller and `BUILD_STAMP` is dead code, while both update gates require a
  strictly newer version. A security hotfix released under an existing version would
  silently never ship. Bump the version for every security release.
- Dead IPC commands remain callable by any script in the page: `import_card`,
  `revoke_card`, `copy_card`, `open_page` and the `app_icons` path passthrough have no
  UI caller today. Remove them, or gate them behind a capability.
- `additionalBrowserArgs` disables `msSmartScreenProtection` in an already-elevated
  browser (`tauri.conf.json:23`); remove it.
- The WebView holds the MyWE password in `localStorage` in clear
  (`desktop/ui-next/src/state/quota.ts:20,37,92`) while the app already has DPAPI for
  the router secret.
- PowerShell command strings are built by interpolation in several places
  (`src/vpn.rs`, `src/windivert.rs`, `src/lan_ctrl.rs`, `desktop/src-tauri/src/lib.rs`)
  without escaping `'`. Local values only, but the fix is two characters.

## Checked and clean

No committed secrets anywhere in history (2,786 blobs, including unreachable);
`embed/quotacards-embed.pem` was never committed and only its public half is
tracked. Updater public key pinned, compiled in, unchanged since it was introduced,
no override path anywhere. No PR-triggered workflow, no secret echoed in CI, no
`pull_request_target`. SSH auth is publickey-only. Clash API binds loopback with a
per-session secret. No credential, UUID or config in any log file. Android tunnel
service not exported, config passed by explicit intent to a `MODE_PRIVATE` store.
No shell interpolation in the WireGuard helpers; args go through argv. The NSIS
installer no longer elevates or schedules anything. The WinDivert packet parsers
are bounds-checked, and the ARP guard fails closed.

## What to do, in order

1. C-2 and C-3: verify before executing, fail closed. These are the two that let a
   remote asset or a local file become Administrator code.
2. C-1: admin-owned install directory, and stop the app from running elevated.
3. H-3: scope the signing secrets, pin the CLI.
4. H-1 and H-2: host-key pinning and certificate pinning. This is what makes the
   tunnel mean anything on a hostile network.
5. H-4: decide whether the per-device secret is real; if not, implement it or
   remove it so nobody trusts it.
6. Everything medium, before the next release.

## Needs validation, with the exact question

1. What does the live `/usr/local/bin/qc-agent` check? The repository's copy
   rejects the commands the client actually sends, so it is not what runs.
2. Does any distributed `.exe`/`.apk` embed `EMBED_KEY`? A clean checkout cannot,
   and no CI step supplies it.
3. Is `inbounds[].settings.fallbacks` routing public 443 to the local
   `qc-speed.py`, which has no authentication? The deployment decides, not the repo.
4. Which `tar` the elevated installer resolves, and whether it refuses `..` and
   absolute members.
