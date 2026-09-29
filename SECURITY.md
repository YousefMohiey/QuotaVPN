# Security policy

## Reporting a vulnerability

Please do not open a public issue for anything security related. Use GitHub's
private reporting instead:

https://github.com/YousefMohiey/QuotaVPN/security/advisories/new

That channel stays private between you and the maintainer while the report is
looked at. If the report is valid you will be credited in the fix notes, unless
you prefer to stay anonymous.

Please include what you did, what you expected, and what happened. A proof of
concept, a log, or the exact request helps more than a description.

## What is in scope

- The Windows and Android apps in this repository.
- The project site at https://quotavpn.app/ and the demos it serves.
- The server setup scripts in `embed/` (Xray, Hysteria2, WireGuard and the
  restricted agent they install).

## What is out of scope

- Anything requiring a modified app, a rooted device or administrator rights on
  the user's own machine.
- Reports that only say a scan tool flagged the build. The installers are
  unsigned, so Windows SmartScreen and a few scanners will complain about an
  unknown publisher; that is expected and documented, not a vulnerability.
- Denial of service by flooding the public endpoints.

## Current posture

The apps and the project site are open source and the maintainer reads every
report. No credentials, tokens or private keys are stored in this repository;
the `.gitignore` and the history both enforce that.
