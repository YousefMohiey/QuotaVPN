# QuotaVPN 0.7.6 experimental

Desktop-only test build. The phone remains on 0.7.5. The stable feed `latest.json`
at the repository root is not changed and still points at 0.7.5. This is a
GitHub pre-release with its own feed asset, not the normal update feed.

## Spaces

- A new Spaces place in the rail: create or join a private space, copy the
  invite code, see the roster and addresses.
- **Direct path** test: reports Direct or Relay with an RTT. It uses a Noise IK
  session over UDP and an encrypted echo.
- **Direct LAN**: starts a second Wintun adapter at `10.77.<space>.x/24` and
  carries IPv4 packets over the direct Noise session. Stop removes the adapter.
- The existing hub/relay path remains available. The direct LAN is independent
  of the main QuotaVPN tunnel.
- Start the direct LAN on both PCs at the same time. The lower member id is the
  initiator.

## Updating from 0.7.5

The release carries a signed `latest.json` feed asset. On 0.7.5, choose
**Settings -> Updates -> Experimental**, then **Check for updates**. The
experimental channel lists pre-releases and will offer `0.7.6`. The stable
channel remains 0.7.5 and is not offered this build.

The installer can also be downloaded and run directly; the in-app update is the
same signed installer.

## Before testing

The coordinator is not deployed on the public server. The build defaults to
`https://quotavpn.duckdns.org:8445`, which is not live. Set `QC_SPACES_API` to a
reachable coordinator before launching the app. A user environment variable
survives the elevated launch better than a variable set only in one terminal:

```
setx QC_SPACES_API "http://<coordinator-ip>:8445"
```

Close and reopen the app after `setx`. Both PCs must use the same coordinator.

For two PCs on the same LAN, run the dev coordinator on one of them:

```
python embed/qc-spaces.py --dev --host 0.0.0.0 --port 8445 --db %TEMP%\spaces.db
```

Allow TCP 8445 through Windows Firewall on that PC and use its LAN address in
`QC_SPACES_API`. No VPS is needed for this test. For two PCs on different
networks, the coordinator must be reachable by both; a public host or tunnel is
required. The direct data path is peer-to-peer; only signaling uses the
coordinator.

The dev coordinator is plain HTTP. Keep it on a trusted LAN or behind a TLS
tunnel. Never expose the plain listener to the internet: the invite and bearer
token travel over it.

## Limits

- Windows only. The phone has no Spaces screen yet.
- One peer per direct LAN session.
- The Wintun adapter path has not been exercised on a live machine yet. If
  adapter creation fails, the error is shown in the Spaces screen.
- Broadcast and multicast are not carried yet, so Minecraft must be joined by
  the space address, not through the LAN list.
- The direct path depends on NAT and ISP policy. Relay is the fallback.
- The installer and the app are unsigned and will trigger SmartScreen.

## Verify

- Install over 0.7.5; the app version shows `0.7.6`.
- Create a space on PC A, join from PC B.
- Run **Test direct path** on both PCs at the same time.
- Run **Start direct LAN** on both PCs at the same time.
- Check `Get-NetAdapter` for the `QuotaSpace...` adapter and the `10.77.x.x`
  address.
- Ping the peer's space address.
