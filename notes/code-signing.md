# Signing QuotaVPN for Windows

What the unsigned build looks like today, what each option actually costs, and
exactly what to change once a certificate exists.

## The free route: SignPath Foundation

SignPath Foundation issues the certificate, holds the private key in their HSM,
and signs binaries that were provably built from this repository. For open
source projects it costs nothing, and the repository qualifies: public, MIT
licensed, released, maintained.

What it gives users: a real Authenticode signature from a publicly trusted
certificate, so the installer no longer says Unknown publisher.

What it costs besides money:

- The publisher reads **SignPath Foundation**, not your name. That is the deal,
  they vouch for the build because they cannot vouch for a stranger.
- The build has to run on GitHub Actions, on a GitHub-hosted runner. They check
  that the artifact came from a workflow on this repository and refuse
  anything built on a laptop. Public repositories get those minutes free.
- They review the application by hand, so it is not instant.

### Steps

1. Apply at https://signpath.org/apply with the repository URL, the MIT licence,
   and a line on what the app does.
2. They create an organisation, a project, a signing policy and an artifact
   configuration. Copy the organization id, project slug and policy slug.
3. Add `notes/signpath-artifact-config.xml` as the artifact configuration, or
   upload one installer and let them generate it.
4. In the GitHub repository settings:
   - secret `SIGNPATH_API_TOKEN`
   - secret `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   - variables `SIGNPATH_ORG_ID`, `SIGNPATH_PROJECT_SLUG`,
     `SIGNPATH_POLICY_SLUG`, `SIGNPATH_ARTIFACT_CONFIGURATION`
5. Push a tag. `.github/workflows/signed-release.yml` builds, signs, re-signs
   the updater manifest over the signed bytes, verifies with `signtool`, and
   attaches both files to the release.

### The application text

Short and true beats long. This is the whole thing:

> QuotaVPN is a free, open source VPN client for Windows and Android, MIT
> licensed, at https://github.com/YousefMohiey/QuotaVPN. It routes a chosen
> app's traffic through a data package the user already pays for, so a gaming or
> streaming plan can be used from a desktop. Python and Rust client, no
> telemetry, configurable servers. The Windows installer is built by the
> workflow in this repository and is the artifact that needs signing.

### The updater signature is rebuilt after signing

This is the detail that quietly breaks things. Tauri signs the installer with
the updater key while bundling, then SignPath rewrites the file to add the
Authenticode signature, and the old `.sig` no longer matches. The workflow runs
`tauri signer sign` again over the signed installer and publishes that one.


## What users see now

- The installer raises a UAC prompt that says **Publisher: Unknown**, which
  reads as "nobody vouches for this".
- A downloaded installer shows **Windows protected your PC** until enough
  people have run that exact file.

Both come from the same fact: the binaries carry no Authenticode signature.
There is nothing wrong with the code, and no amount of build hygiene fixes it.

## What signing does and does not do

- It replaces "Unknown publisher" with the name on the certificate. This is
  immediate and permanent for every file signed with that certificate.
- It makes SmartScreen reputation **accrue to the certificate** instead of to
  one file hash, so the tenth release inherits what the first nine earned.
- It does **not** remove the first-download warning for a brand new binary.
  Microsoft states plainly that a newly signed file can still warn until its
  hash and publisher build reputation, and that EV no longer short-circuits
  this (that behaviour was removed in 2024). Anyone selling EV as an instant
  fix is selling the old story.

## The options, with real figures

| Option | Cost | Available to an individual in Egypt | Publisher shown | Notes |
| --- | --- | --- | --- | --- |
| Certum Open Source Code Signing in the Cloud | from $58/year | Yes | "Open Source Developer " + your name | Needs an open source project. Signing through SimplySign, no USB token. 5000 signatures/month cap. |
| SignPath Foundation | free | Yes | SignPath Foundation | Free for OSI-licensed projects with no proprietary parts. They vouch for the build coming from the repo, so the publisher is the foundation, not you. |
| OV certificate (Sectigo, DigiCert, Comodo resellers) | $150 to $300/year | Yes | your name or company | Identity checks, private key on a token or cloud HSM. |
| EV certificate | $400+/year | Yes | your name or company | Same SmartScreen behaviour as OV since 2024. Only worth it for enterprise procurement. |
| Azure Artifact Signing (formerly Trusted Signing) | about $9.99/month | **No** | your name | Microsoft limits individuals to the USA and Canada. Organisations: USA, Canada, EU, UK. Egypt is not covered. |
| Microsoft Store | free | Yes | Microsoft | No warnings at all, but MSIX packaging does not fit an app that installs a SYSTEM scheduled task, and the Store EXE/MSI route still requires your own certificate. |

The repository is already public and MIT licensed, so both of the cheap routes
are open to it. Certum is the one that puts **your name** on the prompt.

`cognitedata/code-sign-action` does not belong on this list as a free option. It
is a wrapper that drives SimplySign, so it still needs a certificate you bought;
it only removes the manual step of connecting the signing service.

## Once a certificate exists

Nothing in the source changes. The build reads one environment variable:

```bash
export QC_SIGN_THUMBPRINT=7012A3B4C5D6E7F8...   # certificate thumbprint
export QC_SIGN_TIMESTAMP=http://timestamp.digicert.com   # optional, this is the default
bash tools/build-installer.sh
```

With the thumbprint present the bundler signs the app binary and the installer,
SHA-256, timestamped. Without it the build is exactly what it is today.

To check any binary by hand:

```powershell
powershell -File tools/sign-file.ps1 -Files .\QuotaVPN_0.4.8_x64-setup.exe
```

That signs with the thumbprint and then runs `signtool verify /pa`, so a
file that did not really get signed fails loudly.

## How the certificate appears in the store

SimplySign (Certum) and the other cloud HSM clients install a virtual card
that Windows sees as a normal certificate. Start the SimplySign desktop app,
sign in, and the certificate shows a thumbprint in `certmgr.msc` under
Personal > Certificates. That thumbprint is the value for
`QC_SIGN_THUMBPRINT`. The private key never leaves the HSM.

## Order of work, when the time comes

1. Apply for the Certificate. Certum asks for the open source project link and
   identity documents.
2. Install SimplySign, activate the card, sign in.
3. Read the thumbprint from `certmgr.msc`.
4. Set `QC_SIGN_THUMBPRINT` and run the normal build.
5. Publish, then watch SmartScreen over the first weeks. Reputation follows
   downloads; there is no switch to flip.
6. Keep the certificate alive. Every signature carries a timestamp, so copies
   already on disk keep validating after the certificate expires.

## Timestamping matters more than people think

An untimestamped signature stops being valid the day the certificate expires,
and every installed copy starts warning again. The build always timestamps, and
`tools/sign-file.ps1` does too.
