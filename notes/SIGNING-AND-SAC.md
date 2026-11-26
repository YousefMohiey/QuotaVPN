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
| WDAC supplement | 0 | whatever | SAC stays on but ignored the policy | tested, no | Deploys fine, never authorized |
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
powershell -ExecutionPolicy Bypass -File tools/make-test-cert.ps1
powershell -ExecutionPolicy Bypass -File tools/sign-release.ps1 -Thumbprint <printed thumbprint>
```

`make-test-cert.ps1` creates a 3 year self-signed code-signing certificate, keeps
the PFX and its password in `C:\Tools` (never in the repo), exports the public
`.cer`, and installs the root into `LocalMachine\Root` here. `sign-release.ps1`
signs every payload binary and verifies the result; the PFX path is only needed
on a machine where the certificate is not in the store.

To make a second PC show the name in UAC, import `C:\Tools\qc-testsign.cer` into
Trusted Root there. SAC still blocks the app on that PC.

The publisher text is the certificate CN. `make-test-cert.ps1 -Publisher "Some
Name"` changes it. It is free text, so any name can be printed, but the chain
still ends at a root only our PCs trust.

## Route C: WDAC supplement, tested and inert

Attempted, deployed, and measured on this PC on 26 November 2026. Short version:
the supplement installs and signs correctly, but Smart App Control never
authorizes it, so it allows nothing. Microsoft's SAC base policy carries no
`SupplementalPolicySigners` section, and there is no supported way to add our
signer to it, so a policy of ours sits outside its circle of trust.

What did work, and is kept in `tools/sac/`:

- `quotavpn-supplemental.xml` compiles, and `citool -up` accepts it against the
  real SAC base policy ID.
- Policy signing needs the Code Integrity content type `1.3.6.1.4.1.311.79.1`.
  With the `311.10.3.12` EKU OID as the content type, `citool` accepts the file
  and reports success, but the policy stays unsigned. That cost an hour.
- A policy signed by a locally rooted certificate is only authorized after a
  reboot, so the trust anchors are read at boot.
- `Tools/sac/deploy-supplemental.ps1` does compile, sign and deploy in one step.

What did not work: after the reboot, `citool -lp` reads
`Is Authorized: false`, `Is Currently Enforced: false`, with a signed policy and
an unsigned one. Under enforcement mode, an unsigned binary named exactly
`QuotaVPN.exe` is still blocked with "An Application Control policy has blocked
this file", while the installed copy only runs because the Intelligent Security
Graph knows it.

So the supplement is documentation-grade, not a fix. Making SAC plus an
exception actually work means deploying the example policy under
`%windir%\schemas\CodeIntegrity\ExamplePolicies\SmartAppControl.xml` as our own
base policy (minus `Enabled:Conditional Windows Lockdown Policy`, which Microsoft
documents as mandatory) and appending the allow rules to that. That is a real
project: signed policy, per-PC deployment, and it replaces SAC rather than
extending it.

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

Under enforcement mode on this PC, both an unsigned copy and a copy signed with
the test certificate were blocked with "An Application Control policy has
blocked this file". Only the already installed copy ran, on its Intelligent
Security Graph reputation. That is the width of what a self-signed certificate
buys: no SAC help, and a name in UAC on PCs where the root is installed.
