# Toolbox

What is installed from the curated list, where it lives, and when to reach for
it. Anything marked on-demand needs no install: `npx` fetches it.

## Skills (loaded by Hermes, under `creative/` and `software-development/`)

| Skill | Use it for |
| :--- | :--- |
| `taste-skill` (already had) | art direction, anti-slop |
| `impeccable` (already had) | design ruleset and detector |
| `gsap-core`, `gsap-timeline`, `gsap-scrolltrigger`, `gsap-react`, `gsap-plugins`, `gsap-performance`, `gsap-frameworks`, `gsap-utils` | any GSAP work: tweens, timelines, scroll, React integration |
| `huashu-motion` | code-driven motion graphics without diffusion artifacts |
| `transitions-dev`, `transitions-polish` | page transitions, view transitions, polish passes |
| `karpathy-guidelines` | surgical edits, no scope creep |
| `ponytail`, `ponytail-audit`, `ponytail-review`, `ponytail-debt`, `ponytail-gain`, `ponytail-help` | minimal code, over-engineering review |
| `security-audit` | six-phase vulnerability audit before shipping |

## MCP servers

- `chrome-devtools` (new): 32 tools, live DOM, network, performance traces.
  Needs a session reload (`/reload-mcp`) before its tools appear.
- `playwright`, `shadcn`, `21st`, `figma` (already configured).

## Binaries

- `%LOCALAPPDATA%\Programs\xerj\xerj.exe` — hybrid code search plus MCP server.
  The MCP side needs a running node first: `xerj --insecure --data-dir <dir>`.
- `%LOCALAPPDATA%\Programs\katana\katana.exe` — web crawling, endpoint discovery.

## Python

- `weasyprint` 70.0 — HTML and CSS to paginated PDF.
- `agent-reach` 1.6.0 — JSON web access: `agent-reach read <url>`, plus
  channels for YouTube, GitHub, Reddit, X. Verified live.
- `earshot` 0.4.0 at `C:\Tools\earshot` (own venv) — audio perception:
  describe, match, rank, compare. CPU only. Verified on a 440 Hz tone.

## Desktop apps

- `VoiceStudio` — local voice cloning, `%LOCALAPPDATA%\Programs\VoiceStudio`.
- `LocalBG` — offline background removal, `C:\Tools\LocalBG`.

## On demand, no install

- `npx designlang <url>` — pull a live site's design tokens into DESIGN.md.
- `npx @playwright/cli` — browser automation from the command line.

## Not installed, and why

- `react-bits`, `gluestack-ui`, `lenis`, `sileo`, `motion-anything`,
  `ManimGX`: libraries, not tools. Add to a project only when that project
  needs them; none of them belong in the QuotaVPN bundle, which is size
  sensitive.
- `Turnitout Humanizer`: ships no local code, it is a live Streamlit app. The
  method is captured in the `academic-humanizer` skill instead.
