# QuotaVPN 0.7.7 experimental

Desktop-only test build. The phone remains on 0.7.5. The stable feed at the
repository root still points at 0.7.5; this pre-release carries its own
`latest.json`.

## Fixed in this build

- The device name now defaults to the Windows computer name, so **Create** works
  the first time instead of failing on an empty device field that only showed a
  placeholder.
- Validation now names the field that is missing, and the message clears as soon
  as you type.
- An unreachable coordinator is named in the error, with the URL it tried, so a
  wrong `QC_SPACES_API` is obvious.
- The device placeholder no longer shows a personal machine name.

## Spaces

- Create or join a private space, copy the invite code, see the roster and
  addresses.
- **Direct path** test: Direct or Relay with an RTT, over a Noise IK session and
  an encrypted UDP echo.
- **Direct LAN**: a second Wintun adapter at `10.77.<space>.x/24` carries IPv4
  packets over the direct Noise session. Stop removes the adapter.
- The hub/relay path remains the fallback and is independent of the main
  QuotaVPN tunnel.

## Coordinator

The public coordinator is not deployed. Point the app at a reachable one before
launching it:

```
setx QC_SPACES_API "http://<coordinator-ip>:8445"
```

Close and reopen the app after `setx`; sign out and back in if the app does not
pick it up. For two PCs on one LAN, run the dev coordinator on one of them:

```
python embed/qc-spaces.py --dev --host 0.0.0.0 --port 8445 --db %TEMP%\spaces.db
```

Allow TCP 8445 through Windows Firewall on that PC and use its LAN address as
`<coordinator-ip>` on both. The dev coordinator is plain HTTP: keep it on a
trusted LAN or behind a TLS tunnel.

## Updating

On 0.7.6, **Settings -> Updates -> Experimental -> Check for updates** offers
0.7.7. The installer is unsigned and will trigger SmartScreen.

## Limits

- Windows only. One peer per direct LAN session.
- Broadcast and multicast are not carried yet; join Minecraft by the space
  address.
- The Wintun adapter path is still unexercised on a live machine.
- The direct path depends on NAT and ISP policy.
