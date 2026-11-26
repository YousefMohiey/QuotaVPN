# QuotaVPN design foundation

The app follows the `ai-design-skills` visual system: one typeface, flat dark
surfaces, one accent per theme, a closed spacing and radius scale, Phosphor
icons at one weight, and the `cubic-bezier(0.22, 1, 0.36, 1)` motion curve.

## Type

Manrope is the product voice. Geist Mono is used only for addresses, latency,
codes and other tabular data. No italics. No weight above semibold.

| Style | Scale | Size / line height | Weight |
|---|---|---|---|
| Heading 4 | `text-3xl` | 30 / 36 | Semibold |
| Heading 5 | `text-2xl` | 24 / 32 | Semibold |
| Heading 6 | `text-xl` | 20 / 28 | Semibold |
| Title 2 | `text-lg` | 18 / 28 | Medium |
| Title 3 | `text-base` | 16 / 24 | Medium |
| Title 4 | `text-sm` | 14 / 20 | Medium |
| Body 3 | `text-base` | 16 / 24 | Regular |
| Body 6 | `text-sm` | 14 / 20 | Regular |
| Button-sm | `text-sm` | 14 / 20 | Semibold |
| Caption 4 | `text-xs` | 12 / 16 | Medium |
| Caption 5 | `text-xs` | 12 / 16 | Regular |

Every text size in `src` resolves to a Tailwind step. If a size falls between
steps it snaps to the step below and takes that step's line height.

## Spacing

`0, 2, 4, 8, 12, 16, 24, 32, 40, 48, 64, 80, 96`. Main buttons use 8px
vertical and 12px horizontal padding. Cards use 16 or 24. The desktop density
stays compact; use 24 between major sections.

## Radius

Tailwind values only: 2, 4, 6, 8, 12, 16, 24, full. Nested shapes use
`inner = outer - gap` only when the gap is under 32 and the result is over 2.
A 16px card with 8px internal padding gives inner shapes 8px.

## Colour

The dark surfaces are the approved five only:

| Token | Value |
|---|---|
| window | `#000000` |
| panel | `#181818` |
| raised panel / field | `#1F1F1F` |
| selected / chip | `#272727` |
| elevated | `#313131` |

Text is `#FAFAFA`, `#B3B3B3`, `#8A8A8A`. Hairlines are white at 9% and 15%.
The accent is the only hue that changes by theme:

| Theme | Accent | Strong | Vivid |
|---|---|---|---|
| Obsidian | `#4E6FE8` | `#9DB2FF` | `#6C8BFF` |
| Ember | `#B85325` | `#FFB08A` | `#D9662F` |
| Mint | `#0B7C74` | `#7BE8DC` | `#12A89A` |
| Violet | `#6E51D9` | `#C3B2FF` | `#8B70F5` |

Accent tints (`--brand-bg`, `--brand-line`) are overlays on the approved
surfaces. Semantic green, amber and red stay constant so state is never
carried by the theme. No background gradients. The only gradient is the page
heading text, white to `#9B9B9B`.

## Icons

Phosphor, one weight: regular. `src/components/icons.tsx` maps the old names
to Phosphor and accepts `strokeWidth` only for call-site compatibility. Do not
mix Lucide, Feather or Material icons into the app.

## Motion

Base curve `cubic-bezier(0.22, 1, 0.36, 1)`. Use 500ms for layout and
position, 300ms for colour and opacity. Animate `transform` and `opacity`,
not `top`, `left`, `width` or `height`. Decorative loops may repeat; state
changes must settle. Reduced motion gets a plain fade.

## Components

- **Buttons:** primary is accent fill with `--on-brand` text. Secondary is a
  `#1F1F1F` fill with a hairline. Press uses `scale(0.97)` or a 1px translate.
  Every button has hover, focus-visible and disabled states.
- **Cards:** a flat `#181818` panel, a full 1px hairline, radius 16. No
  single-side border. Dividers inside a card are 1px rows, not card borders.
- **Chips and badges:** radius 8, `#1F1F1F` fill, caption type. State badges
  pair colour with a word.
- **Inputs:** `#1F1F1F` fill, hairline, radius 12, focus ring at 3:1.
- **Data views:** loading uses skeletons shaped like the rows; empty and error
  are composed states with one action, never blank panels.
- **Dialogs:** `#181818` at 96%, one hairline, radius 16, the base curve.
- **Themes:** written by `src/state/theme.tsx` to `data-theme` and `qc-theme`.
  Swatches live in Settings > Appearance and at the foot of the rail.

## Known exceptions

- The desktop window is dark only. A light mode needs a full token pass and is
  not shipped.
- The speed dial keeps a flat accent fill and a text-coloured needle. Data
  visualisation may use more than one stroke, but not a gradient background.
- The custom `ValorantMark` and `WeMark` are brand marks, not a second icon
  set.
