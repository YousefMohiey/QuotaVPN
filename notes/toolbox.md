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

## On demand, no install

- `npx designlang <url>` — pull a live site's design tokens into DESIGN.md.
- `npx @playwright/cli` — browser automation from the command line.

## Not installed, and why

- `earshot` (audio perception), `VoiceStudio` (voice cloning),
  `LocalBG` (background removal): each pulls a PyTorch or model stack worth
  gigabytes. Install on request, not by default.
- `react-bits`, `gluestack-ui`, `lenis`, `sileo`, `motion-anything`,
  `ManimGX`: libraries, not tools. Add to a project only when that project
  needs them; none of them belong in the QuotaVPN bundle, which is size
  sensitive.
- `Turnitout Humanizer`: academic text tool, unrelated to this repo.
