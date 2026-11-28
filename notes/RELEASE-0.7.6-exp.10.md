# QuotaVPN 0.7.6-exp.10

Windows experimental build. The phone stays at 0.7.5.

## Appearance

- Seven full colour presets: Serein, Eclipse, Slate, Nightfall, Ember, Verdant
  and Afterglow. Each one changes the surfaces, the hairlines, the text and the
  primary colour together, not just the highlight.
- Serein keeps the approved flat dark steps. The other presets use tinted
  equivalents of the same steps.
- Appearance lives only in Settings. The theme swatches at the foot of the rail
  are gone.
- The panel is a theme grid, a primary colour picker and an interface font row,
  with the current value shown next to each control.
- A custom primary colour overrides the preset accent without changing the
  preset surfaces. Picking a different preset returns it to that preset's accent.
  Button labels switch between black and white from the accent's luminance, so a
  light primary stays readable.
- The in-app Q mark now follows the theme: the ring uses the text colour and the
  tail uses the primary colour. The Windows icon and tray asset stay static.
- Old saved values map to the new presets: obsidian to Serein, mint to Verdant,
  violet to Nightfall.

## Interface font

- The interface face is now a user choice. Manrope and Archivo ship with the
  app; the installed Windows faces are listed too.
- The choice is saved and applied before the first paint. Numbers, addresses
  and latency keep the monospace face.
- Arabic and flag glyphs keep their fallback faces under every option.

## Everything from exp.9

No automatic default space, no auto-connect, Connect and Disconnect wording, the
short Spaces page, private QuotaSpace networks, sealed space passwords, manual
switching, DPAPI card restore, parallel RTT checks, log rotation, and the local
YousefMohiey signature.
