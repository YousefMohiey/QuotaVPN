# Defender, and why three releases in a row were unusable

Short version: an unsigned app that installs scheduled tasks and loads a kernel
driver reads as malware to Windows Defender's behaviour model. Two separate
detections, both legitimate complaints about the design, both fixed by moving the
work somewhere the user can see it.

## What happened

| version | what went wrong | what it was |
|---|---|---|
| 0.6.6 | would not start: "entry point TaskDialogIndirect not found" | the CI build had no Windows manifest, so it bound comctl32 v5 |
| 0.6.7 | quarantined on download, app removed after install | the package carried `WinDivert64.sys`, a kernel driver |
| 0.6.8 | quarantined on download | `Behavior:Win32/Persistence.A!ml` |

Each fix was necessary and none was sufficient on its own.

## The two real causes

1. **A kernel driver inside an unsigned installer.** Defender treats a package
   that drops and loads a driver as malicious by design, because that is what
   malware does. The driver is now fetched by setup, from its own release, with
   its Authenticode signature checked before it is kept.

2. **Persistence installed silently at install time.** The NSIS installer spawned
   `--setup-helper` hidden and elevated, and that pass registers scheduled tasks.
   `Behavior:Win32/Persistence.A!ml` is precisely that pattern. The installer now
   registers nothing; the helper is registered the first time somebody presses
   connect, with the prompt in front of them.

## What is still true

The build is unsigned. SignPath is wired into `signed-release.yml` and skipped on
every run because the secrets are not set (`notes/code-signing.md`). Untrusted
binaries will keep getting heuristics pointed at them: signing is the durable fix,
and it also stops SmartScreen.

If a detection comes back, the file is a false positive and should be reported,
not worked around. The submission text below is what to send.

## False-positive report

<https://www.microsoft.com/en-us/wdsi/filesubmission> then "I believe this file
is incorrectly detected" (software developer).

> QuotaVPN is a personal VPN client. The installer was detected as
> Behavior:Win32/Persistence.A!ml. That behaviour is the product's own privileged
> helper: it registers one scheduled task so the tunnel can create its virtual
> adapter without asking for administrator on every connection. The same code
> ships in every build and the helper is started by the user pressing connect, in
> the app, with a Windows prompt. No third-party software is installed, nothing
> is hidden from the user, and the packet driver it downloads is the official
> WinDivert release with a verified Authenticode signature.
>
> Version: 0.6.9, installer SHA256 bf78939cce8ae1a48ec5fca2662539d016ba1c1e12fb65e89b79ea77168b0906
>
> Please re-evaluate: this is a false positive on a signed-out build of a
> legitimate VPN client.


## SignPath is gone

The workflow no longer has a signing provider. SignPath needed an application that
was never approved, its steps never ran, and the configuration sat in the file
pretending there was a path to a signature. All of it is removed: the steps, the
presence flag, the artifact configuration, the notes, and the repository variables.

Every release this project produces is unsigned. That is the honest state. SmartScreen
warns on first run and heuristic engines may quarantine the installer, because an
unsigned executable that self-updates and asks for Administrator looks like a dropper
regardless of what it does. A certificate is the only thing that changes that verdict,
and the workflow now says so in its header instead of carrying dead configuration.
