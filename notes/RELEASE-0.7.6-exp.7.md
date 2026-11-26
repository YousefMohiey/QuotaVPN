# QuotaVPN 0.7.6-exp.7

Experimental. Stable stays at 0.7.5.

## The full design pass

The app now follows the `ai-design-skills` system end to end:

- Manrope for text, Geist Mono for addresses, latency and codes. No Inter, no
  Roboto, no italics, no weight above semibold.
- One type scale. Every arbitrary pixel size is gone. Large headings use
  `text-3xl`, card titles `text-2xl`, body `text-sm` or `text-base`, captions
  `text-xs`.
- One radius scale. Every arbitrary pixel radius is gone. Cards are 16, inner
  panels 12, chips and controls 8 or 12. Nested corners follow the formula.
- One spacing scale. Margins, padding and gaps use 2, 4, 8, 12, 16, 24, 32 or
  larger.
- Phosphor icons, one weight. The Lucide set is gone from every screen.
- Flat surfaces only: `#000000` for the window, `#181818` for panels, `#1F1F1F`
  for raised areas, `#272727` for selected chips. No background gradients. The
  only gradient is the page heading text, white to `#9B9B9B`.
- Four accent themes: Obsidian, Ember, Mint and Violet. The surfaces stay
  neutral; the accent changes the actions, chips and focus ring. The choice is
  remembered and is available in Settings > Appearance and at the foot of the
  rail.
- Hover, pressed, focus-visible and disabled states across the controls. A
  skip link focuses the main region. The Network list uses skeleton rows while
  a scan runs.
- The Arabic language name is no longer mojibake. It comes from the i18n
  dictionary.

The Spaces screen keeps the LAN-first layout from exp.6: a space chip row, the
wide LAN card with only live peers, transport and RTT, the invite and member
card, and the play-together rail. The stale identity and the offline test peer
remain filtered out.

## Direct first

The bundled OpenRad engine falls back to public STUN for its NAT mapping when
the coordination server does not advertise two UES hosts. The transport order
is direct UDP, direct TCP, then the relay. On a direct path, game traffic does
not pass through the VPS. The card says `Direct` or `Relayed` and shows the
measured RTT.

## Known limits

- Windows to Windows. The phone is still 0.7.5 and has no Spaces screen.
- The `QuotaVPN` network is public in this build. A private network with a
  password is the next step.
- A direct path depends on both NATs. When one side is symmetric or blocks UDP,
  the relay remains the fallback.
- The game matrix has not run yet.
- Smart App Control still blocks an unsigned installer. That is unchanged.

## Build

- Build stamp: to be filled after the build
- Installer SHA-256: to be filled after the build
