# Signing and Smart App Control

The friend's PC blocks QuotaVPN before it installs. This note records what
actually causes that, which routes exist, and what each route fixes. It also
holds the tooling added for the free routes.

## What Smart App Control actually enforces

SAC is a Microsoft-managed App Control policy (`{0283AC0F-...}` in enforcement,
`{1283AC0F-...}` in evaluation). Microsoft's own description of the decision:

> If the app intelligence service is unable to make a prediction, then Smart App
> Control will still allow an app to run if it is signed with a certificate
> issued by a certificate authority (CA) within the Trusted Root Program.

So the trust anchor is the Windows trusted root list, not the local certificate
store. A self-signed certificate that we install ourselves is invisible to SAC:
the machine trusts it, SAC does not. That is why "sign it with a fake publisher
cert" does not unblock the friend.

Three more facts that shape the answer:

- SAC has no allow-list, no "run anyway", and no per-file exception.
- SAC checks every executable and DLL it loads, not the installer only. Signing
  the setup exe and leaving `sing-box.exe` unsigned still fails at launch.
- Since the April 2026 update (KB5083769) SAC can be turned off and back on from
  Windows Security without reinstalling Windows.

## Routes

| Route | Cost | Publisher shown | SAC | Real | Notes |
| --- | --- | --- | --- | --- | --- |
| Turn SAC off | 0 | Unknown | off | yes | Working fix today, reversible |
| Self-signed cert | 0 | CN of our cert | still blocks | cosmetic | UAC name, AV heuristics, dev testing |
| WDAC supplement | 0 | whatever | SAC stays on, supplement allows | yes here | Per PC, by file name, unofficial |
| Public OV cert | 99 USD and up | verified name | allows | yes | The durable answer |
| SignPath Foundation | 0 | SignPath Foundation | allows | if accepted | Won't take this project right now |

A borrowed or leaked certificate only helps if it is unexpired and not revoked.
The well-known leaked keys (D-Link, Realtek, MSI, AnyDesk) are all revoked by the
CAs after disclosure, so a signature with them still fails verification and SAC
still blocks. There is no "free trusted cert" shortcut.

## Route A: turn SAC off (what the friend should do now)

1. Double-click `tools/sac/sac-off.reg`, accept UAC.
2. Reboot. `citool.exe -r` in an elevated prompt sometimes saves the reboot; a
   restart always does.
3. Install and run QuotaVPN.

Check it:

```powershell
powershell -ExecutionPolicy Bypass -File tools/sac/sac-state.ps1
```

The value must read `0 (Off)`. While it stays off, everything installs normally.
Defender antivirus keeps running; only app control is gone.

If the PC needs SAC back, `sac-on.reg` restores enforcement. Be clear with the
friend that turning it back on blocks QuotaVPN again on the next launch, because
the certificate situation has not changed.

`sac-eval.reg` is the middle setting. It loads the evaluation policy, which logs
and usually lets unsigned code through. This PC sits in that state and installed
0.7.6-exp.4 fine, but "usually" is not "always", so use Off for a PC that has to
work.

## Route B: local certificate (cosmetic, useful for testing)

```powershell
powershell -ExecutionPolicy Bypass -File tools/make-test-cert.ps1 -Publisher "YousefMohiey" `
  -PfxPath C:\Tools\yousef-codesign.pfx -CerPath C:\Tools\yousef-codesign.cer
powershell -ExecutionPolicy Bypass -File tools/sign-release.ps1 -Thumbprint <printed thumbprint>
```

`make-test-cert.ps1` creates a 3 year self-signed code-signing certificate, keeps
the PFX and its password in `C:\Tools` (never in the repo), exports the public
`.cer`, and installs the root and the Trusted Publisher entry in the local machine
stores. `sign-release.ps1` signs every payload binary, including
`openrad\QuotaSpace.exe`, and verifies the result; the PFX path is only needed on a
machine where the certificate is not in the store.

This tree uses `CN=YousefMohiey` at `C:\Tools\yousef-codesign.pfx`, thumbprint
`BAC8A3DA8C882AA63293B311A0ED4E7117F21A18`. `tools/build-installer.sh` looks that
certificate up by subject when `QC_SIGN_THUMBPRINT` is unset, so a local installer
signs both `QuotaVPN.exe` and `QuotaSpace.exe` without extra setup. The version
resource carries `CompanyName = YousefMohiey` as well, which is what the Windows
Firewall prompt prints next to the file name.

To make a second PC show the name in UAC and the firewall prompt, import
`C:\Tools\yousef-codesign.cer` into Trusted Root and Trusted Publishers there. SAC
still blocks the app on that PC.

The publisher text is the certificate CN and the PE company name. It is free text,
so any name can be printed, but the chain still ends at a root only our PCs trust.

## Route C: WDAC supplement

This one is real, and it is what "even if not legit" looks like: keep SAC in
enforcement, attach an extra App Control policy that lets the QuotaVPN payload
run, and let SAC keep blocking everything else.

The mechanism exists on this build. The active SAC base policy IDs are
`{0283AC0F-...}` (enforce) and `{1283AC0F-...}` (evaluation), and Windows
already attaches its own supplemental policies to both (the Flight and Test
supplements, `Is Authorized: true` under `citool -lp`). The shipped policy file
at `%windir%\schemas\CodeIntegrity\ExamplePolicies\SmartAppControl.xml` carries
`Enabled:Unsigned System Integrity Policy` and `Enabled:Allow Supplemental
Policies`, so a supplement can attach and unsigned supplements are tolerated.

`tools/sac/quotavpn-supplemental.xml` is that supplement. It allows the payload
by file name in both signing scenarios and can only add allows, so it cannot
block anything.

Deploy (elevated):

```powershell
citool.exe -up tools\sac\quotavpn-supplemental.xml
citool.exe -lp    # ours must show Is Authorized: true
```

Remove:

```powershell
citool.exe -rp {7C1D2C40-9C2B-4F0E-9E1B-51F0C7A6D001}
```

Costs and caveats: the policy is unsigned and lives at machine scope, so any
administrator can replace it. The rules are by file name, so another program
called `QuotaVPN.exe` is allowed too. Microsoft can rebuild the SAC policy on a
Windows update and drop the supplement; re-deploying fixes it. Per PC, not
per app store download.

Status: schema validation passes, deployment and the enforce-mode block test are
recorded at the end of this note.

## Route D: a real certificate later

Nothing in the tree needs to change. `tools/build-installer.sh` already reads
`QC_SIGN_THUMBPRINT`, passes it to the bundler, signs the payload and the
installer through `sign-release.ps1`, and verifies each signature. Signing with
a real certificate for the first time:

1. Buy an OV code signing certificate (Certum Standard, about 99 to 209 USD, or
   any CA in the Trusted Root Program). Do not buy EV; it no longer skips
   SmartScreen.
2. Install it through the CA's client (SimplySign presents it in the store).
3. `$env:QC_SIGN_THUMBPRINT = "<thumbprint>"; bash tools/build-installer.sh`.

SmartScreen keeps warning until the app builds download reputation, which only
happens with real download volume. SAC accepts a valid signature on the first
run.

## Files that must carry the signature

- `QuotaVPN_<version>_x64-setup.exe`
- `uninstall.exe` (generated by NSIS at install time)
- `QuotaVPN.exe`
- `quotacards_desktop.dll`
- `sing-box.exe` and the engine archive, if shipped next to the app
- `WinDivert.dll` / `WinDivert64.sys`, if shipped
- `WebView2Loader.dll`, if bundled

`wintun.dll` already carries a WireGuard LLC signature and is left alone. The
TAP driver, once Spaces ships it, is signed by OpenVPN Technologies and also
stays as delivered.

## What was proven here

`make-test-cert.ps1` plus `sign-file.ps1` produced a valid RSA-3072 signature
with the Code Signing EKU and a DigiCert timestamp on a copy of `QuotaVPN.exe`,
and `signtool verify /pa` returned zero warnings and zero errors with the root
installed.
