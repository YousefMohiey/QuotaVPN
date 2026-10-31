# Packet driver experiment (parked)

We tried to replace Npcap with a driver of our own so the app would carry no
third party licence. The work is in the history, not in the tree.

## What was built

- An NDIS 6 lightweight filter, forked from Microsoft's MS-PL sample, with our
  own device (`\Device\QuotaVpnPkt`), service, filter GUID and INF.
- A frame ring of 512 slots with drop rather than block, four IOCTLs (bind,
  read, write, stats), and a completion path that removes injected frames.
- A build that needs only the WDK: `cl` and `link` directly, no Visual Studio
  driver integration.
- Signed with a development certificate; Inf2Cat catalog, signtool on both.

Windows accepted the package and offered to install it, which proves the
signature chain and the INF are correct.

## Why it stopped

Three separate walls, in order of cost:

1. An unsigned or self signed driver will not load on 64 bit Windows at all.
   Test signing works on a development machine, but it is a boot setting with a
   desktop watermark and is not something to ask of users.
2. Shipping needs the Microsoft Hardware Developer Program, and its entry
   requirement is an EV code signing certificate, roughly 226 to 341 dollars a
   year, plus an attestation submission per build. Azure Trusted Signing does
   not cover kernel drivers.
3. Completing the install needs a click on a Windows security prompt that
   keyboard injection cannot reach inside a VirtualBox guest without Guest
   Additions, so the last mile could not be automated here.

The driver never loaded a frame. Nothing in the app depends on it.

## If this is picked up again

    git log --all -- driver/          # the code, tap and INF included
    git show <commit>:driver/QuotaVpnPkt/tap.c

Start at the EV certificate, not at the code. Without it the rest is a
development toy.

## What ships instead

Npcap, installed by the user from npcap.com. The app detects it, explains what
it is for, and disables Block and Limit until it is there. The free edition is
limited to five systems and may not be redistributed, so a wide release would
need the OEM licence or the router based route.
