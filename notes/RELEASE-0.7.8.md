# QuotaVPN 0.7.8 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## The space is the LAN now

This build removes the main VPN from the Spaces flow. It no longer asks you to
connect the main QuotaVPN VPN to reach a space.

- **Use this space** only makes the space active. It does not turn on the main
  VPN.
- The space card now shows **Connect directly** when another member is online,
  or **Waiting for a member** when nobody else is. The button starts the direct
  path over UDP, with the second Wintun adapter at `10.77.<space>.x/24`.
- When the direct path is up, the card shows the address and **Connected
  directly**. The main VPN is not involved.
- The relay path still exists. It is used only if you turn on the main VPN
  yourself; nothing in the space flow requires it.
- The Direct path test and the Direct LAN card are unchanged.

This is the same direction as Radmin: launch, join the network, get an address,
play. The remaining difference is that the direct connection is one click on
each PC and still pairs one peer at a time. Automatic pairing and multiple
members are the next step.

## Connect

1. Both PCs install this build and open the same space.
2. Both must have a reachable coordinator in `QC_SPACES_API`.
3. When the other member shows online, press **Connect directly** on both PCs
   within a few seconds. The lower member id initiates the handshake.
4. The card shows the `10.77.x.x` address. Ping it, then use it for Minecraft.

## Fixed from 0.7.7

- The device name defaults to the Windows computer name.
- Validation names the missing field and clears while you type.
- An unreachable coordinator is reported with its URL.

## Limits

- Windows only, one peer per direct session.
- Broadcast and multicast are not carried yet; join Minecraft by address.
- The Wintun adapter path is still unexercised on a live machine.
- The direct path depends on NAT and ISP policy. The relay is the fallback for
  networks where UDP cannot pass.
