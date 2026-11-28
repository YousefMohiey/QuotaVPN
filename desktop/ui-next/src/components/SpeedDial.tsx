import { useMemo } from "react"
import { motion } from "motion/react"
import { useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

// The Speedtest dial, rebuilt in the app's colours: a 270 degree arc, the nine
// original speed labels, a translucent needle and a thin gradient ring around
// GO while idle. The arc uses the theme accent in a gradient instead of Ookla's
// bitmap, and labels the needle has passed turn bright so the scale reads at a
// glance. The live numbers stay in the reading rail, as they do above the
// Speedtest gauge.

const VIEW = 286
const C = VIEW / 2
const START = -135
const SWEEP = 270

const R_ARC = 130
const STROKE = 23
const R_LABEL = 101.5
const R_NEEDLE = 85
const NEEDLE_BASE = 20
const NEEDLE_TIP = 10

const STOPS = [0, 5, 10, 50, 100, 250, 500, 750, 1000]
const FRACS = [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1]

const pt = (r: number, deg: number) => {
  const a = (deg * Math.PI) / 180
  return { x: C + r * Math.sin(a), y: C - r * Math.cos(a) }
}

const angleFor = (frac: number) => START + SWEEP * Math.max(0, Math.min(1, frac))

/** Where a speed sits on the dial: linear inside each scale segment. */
const fracFor = (v: number): number => {
  if (v <= STOPS[0]) return FRACS[0]
  for (let i = 1; i < STOPS.length; i++) {
    if (v <= STOPS[i]) {
      const span = STOPS[i] - STOPS[i - 1]
      const part = span > 0 ? (v - STOPS[i - 1]) / span : 0
      return FRACS[i - 1] + (FRACS[i] - FRACS[i - 1]) * part
    }
  }
  return 1
}

export type DialPhase = "idle" | "ping" | "download" | "upload" | "done"

export function SpeedDial({
  value,
  phase,
  runId,
  onStart,
}: {
  value: number
  phase: DialPhase
  runId: number
  onStart: () => void
}) {
  const { t } = useI18n()
  const running = phase === "ping" || phase === "download" || phase === "upload"
  const atRest = phase === "idle" || phase === "done"

  const frac = fracFor(value)
  const needleDeg = angleFor(frac)

  const arcLen = 2 * Math.PI * R_ARC * (SWEEP / 360)
  const arcStart = pt(R_ARC, START)
  const arcEnd = pt(R_ARC, START + SWEEP)
  const arcPath = `M ${arcStart.x.toFixed(2)} ${arcStart.y.toFixed(2)} A ${R_ARC} ${R_ARC} 0 1 1 ${arcEnd.x.toFixed(2)} ${arcEnd.y.toFixed(2)}`

  const needlePath = `M ${C - NEEDLE_BASE / 2},${C} L ${C + NEEDLE_BASE / 2},${C} L ${C + NEEDLE_TIP / 2},${C - R_NEEDLE} L ${C - NEEDLE_TIP / 2},${C - R_NEEDLE} Z`

  const labels = useMemo(
    () =>
      STOPS.map((stop, i) => ({
        stop,
        frac: FRACS[i],
        pos: pt(R_LABEL, angleFor(FRACS[i])),
      })),
    [],
  )

  return (
    <div className="relative h-[300px] w-[300px] [@media(max-height:660px)]:h-[172px] [@media(max-height:660px)]:w-[172px]">
      <svg
        viewBox={`0 0 ${VIEW} ${VIEW}`}
        className={cn(
          "h-full w-full transition-opacity duration-300",
          atRest ? "pointer-events-none opacity-0" : "opacity-100",
        )}
        aria-hidden
        focusable="false"
      >
        <defs>
          <linearGradient id="speed-arc" x1="0" y1="1" x2="0" y2="0">
            <stop offset="0%" stopColor="var(--brand-vivid)" />
            <stop offset="55%" stopColor="var(--brand)" />
            <stop offset="100%" stopColor="var(--brand-strong)" />
          </linearGradient>
          <linearGradient id="speed-needle" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--txt)" stopOpacity="0.85" />
            <stop offset="100%" stopColor="var(--txt)" stopOpacity="0.06" />
          </linearGradient>
        </defs>

        <path
          d={arcPath}
          fill="none"
          stroke="var(--line-strong)"
          strokeWidth={STROKE}
          opacity="0.75"
        />

        <path
          d={arcPath}
          fill="none"
          stroke="url(#speed-arc)"
          strokeWidth={STROKE}
          style={{
            strokeDasharray: arcLen,
            strokeDashoffset: arcLen * (1 - frac),
            transition: "stroke-dashoffset 200ms cubic-bezier(0.22, 1, 0.36, 1)",
            filter: "drop-shadow(0 0 10px var(--brand-mid))",
          }}
        />

        <g key={runId}>
          {labels.map(({ stop, frac: labelFrac, pos }, i) => {
            const passed = labelFrac <= frac
            return (
              <text
                key={stop}
                x={pos.x.toFixed(2)}
                y={pos.y.toFixed(2)}
                textAnchor="middle"
                dominantBaseline="middle"
                fontSize="14"
                fontWeight="700"
                fill={passed ? "var(--txt)" : "var(--txt3)"}
                fillOpacity={passed ? 1 : 0.55}
                className={runId > 0 ? "qc-lbl-flash" : undefined}
                style={{
                  animationDelay: runId > 0 ? `${i * 70}ms` : undefined,
                  transition: "fill 180ms ease, fill-opacity 180ms ease",
                }}
              >
                {stop}
              </text>
            )
          })}
        </g>

        <g
          style={{
            transform: `rotate(${needleDeg}deg)`,
            transformBox: "view-box",
            transformOrigin: `${C}px ${C}px`,
            transition: "transform 200ms cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        >
          <path d={needlePath} fill="url(#speed-needle)" />
          <circle cx={C} cy={C} r={3.5} fill="var(--txt)" fillOpacity="0.55" />
        </g>
      </svg>

      {/* the idle ring: a thin theme gradient around GO, as on Speedtest */}
      <div
        className={cn(
          "absolute inset-0 grid place-items-center transition-[opacity,transform] duration-300",
          atRest ? "scale-100 opacity-100" : "pointer-events-none scale-95 opacity-0",
        )}
      >
        <div className="relative grid size-[244px] place-items-center rounded-full border border-line [@media(max-height:660px)]:size-[190px]">
          <motion.span
            aria-hidden
            className="absolute size-[180px] rounded-full border border-[var(--brand-line)] [@media(max-height:660px)]:size-[140px]"
            animate={{ scale: [1, 1.07], opacity: [0.45, 0] }}
            transition={{ duration: 2.6, repeat: Infinity, ease: "easeOut" }}
          />
          <div
            className="relative grid size-[180px] place-items-center rounded-full p-[2px] [@media(max-height:660px)]:size-[140px]"
            style={{ background: "linear-gradient(180deg, var(--brand-vivid), var(--brand))" }}
          >
            <button
              type="button"
              onClick={onStart}
              disabled={running}
              aria-label={phase === "done" ? t("startAgain") : t("startTest")}
              title={phase === "done" ? t("startAgain") : undefined}
              className="press grid size-full place-items-center rounded-full bg-[var(--bg)] text-txt focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)] [@media(max-height:660px)]:text-lg"
            >
              <span className="text-3xl font-semibold tracking-[0.14em]">GO</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
