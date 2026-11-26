# QuotaVPN 0.7.6-exp.6

Experimental. Stable stays at 0.7.5.

## The LAN button is now the game LAN

The Connect LAN button on Spaces starts the bundled OpenRad engine. One press
creates the `OpenRad` TAP-Windows6 adapter if it is missing, points the engine
at our coordination server with the bundled modulus, registers this PC once,
starts the service and joins the `QuotaVPN` network. There is no separate
OpenRad install and no command to type.

The engine carries a real Ethernet adapter, so ARP, broadcast and multicast
work for games. The `26.x` address shown on the card is this PC on that LAN.

## Direct first

The coordination server does not run two UDP endpoint (UES) responders, so the
bundled engine now falls back to public STUN only to learn this socket's NAT
mapping. The mapped endpoint still travels through the authenticated
coordination session, and every punch is authenticated by the peer nonce, so
STUN cannot become a peer path by itself.

The transport order is direct UDP, direct TCP, then the relay. When the two
NATs allow a direct path, game traffic does not pass through the VPS. The card
labels each peer `Direct` or `Relayed` and shows the measured RTT.

## New interface

The window uses Manrope for text and Geist Mono for addresses and latency.
Surfaces are flat near-black panes over one accent halo, with hairlines instead
of a field of gradients. Four themes are available from Settings > Appearance
and from the swatches at the foot of the rail: Obsidian, Ember, Mint and
Violet. The choice is remembered locally.

Spaces is rebuilt around the LAN. The selected space is a chip at the top, the
LAN status and live peers get the wide card, the invite and member list sit in
the space card, and a play-together rail explains the three steps. The orbit
map and the button above the screen are gone.

Only connected peers are listed. An old identity left behind by a reinstall,
and an offline member of the space roster, no longer appears as another IP.
The LAN card shows this PC once, then each live peer with transport and RTT.

## Known limits

- Windows to Windows. The phone is still 0.7.5 and has no Spaces screen.
- The `QuotaVPN` network is public in this build. A private network with a
  password is the next step.
- A direct path depends on both NATs. When one side is symmetric or blocks UDP,
  the relay remains the fallback and the card says `Relayed`.
- The game matrix (Warcraft III, Age of Empires II, Terraria, Minecraft,
  OpenTTD, SMB/RDP names) has not run yet.
- Smart App Control still blocks an unsigned installer. That is unchanged.
