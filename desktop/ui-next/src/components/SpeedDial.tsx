import { useMemo } from "react"
import type { IconComponent } from "@/components/icons"
import { useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

// A radial gauge rather than a car dashboard: one slim track, one lit arc and
// a small dot at the reading. The scale keeps the nonlinear stops the speed
// test needs, but only five numbers are printed, so the face stays quiet.
//
// Interaction: GO sits in the centre while there is nothing to show; the moment
// a run starts the word fades, the labels flash white in sequence, the arc and
// dot arrive, and the live number takes the centre. The result comes back from
// the reading cards, not from a needle parked in the opening.

const VIEW = 300
const C = VIEW / 2
const START = -135
const SWEEP = 270

const R_ARC = 128
const STROKE = 16
const R_LABEL = 101.5

const STOPS = [0, 5, 10, 50, 100, 250, 500, 750, 1000]
const FRACS = [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1]
const LABEL_INDICES = [0, 2, 4, 6, 8]

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
  display,
  value,
  unit,
  icon: Icon,
  phase,
  runId,
  onStart,
}: {
  display: string
  value: number
  unit: string
  icon: IconComponent
  phase: DialPhase
  runId: number
  onStart: () => void
}) {
  const { t } = useI18n()
  const idle = phase === "idle"
  const running = phase === "ping" || phase === "download" || phase === "upload"
  const atRest = idle || phase === "done"
  const phaseLabel =
    phase === "ping" ? t("pingTitle") : phase === "download" ? t("chDown") : phase === "upload" ? t("chUp") : ""

  const frac = fracFor(value)
  const dotDeg = angleFor(frac)

  const arcLen = 2 * Math.PI * R_ARC * (SWEEP / 360)
  const arcStart = pt(R_ARC, START)
  const arcEnd = pt(R_ARC, START + SWEEP)
  const arcPath = `M ${arcStart.x.toFixed(2)} ${arcStart.y.toFixed(2)} A ${R_ARC} ${R_ARC} 0 1 1 ${arcEnd.x.toFixed(2)} ${arcEnd.y.toFixed(2)}`

  const labels = useMemo(
    () =>
      LABEL_INDICES.map((i) => ({
        value: STOPS[i],
        pos: pt(R_LABEL, angleFor(FRACS[i])),
      })),
    [],
  )

  return (
    <div className="relative h-[300px] w-[300px] [@media(max-height:660px)]:h-[172px] [@media(max-height:660px)]:w-[172px]">
      <svg viewBox={`0 0 ${VIEW} ${VIEW}`} className="h-full w-full" aria-hidden focusable="false">
        {/* the working face: track, lit stretch, scale and the reading dot */}
        <g style={{ opacity: atRest ? 0 : 1, transition: "opacity 400ms var(--ease-out)" }}>
          <path
            d={arcPath}
            fill="none"
            stroke="var(--line-strong)"
            strokeWidth={STROKE}
            strokeLinecap="round"
          />

          <path
            d={arcPath}
            fill="none"
            stroke="var(--brand)"
            strokeWidth={STROKE}
            strokeLinecap="round"
            style={{
              strokeDasharray: arcLen,
              strokeDashoffset: arcLen * (1 - frac),
              transition: "stroke-dashoffset 170ms cubic-bezier(0.22, 1, 0.36, 1)",
              filter: "drop-shadow(0 0 7px var(--brand-mid))",
            }}
          />

          <g key={runId}>
            {labels.map(({ value: label, pos }, i) => (
              <text
                key={label}
                x={pos.x.toFixed(2)}
                y={pos.y.toFixed(2)}
                textAnchor="middle"
                dominantBaseline="middle"
                fontSize="12"
                fontWeight="600"
                letterSpacing="0.02em"
                fill="var(--txt2)"
                fillOpacity="0.62"
                className={runId > 0 ? "qc-lbl-flash" : undefined}
                style={runId > 0 ? { animationDelay: `${i * 80}ms` } : undefined}
              >
                {label}
              </text>
            ))}
          </g>

          <g
            style={{
              transform: `rotate(${dotDeg}deg)`,
              transformBox: "view-box",
              transformOrigin: `${C}px ${C}px`,
              transition: "transform 170ms cubic-bezier(0.22, 1, 0.36, 1), opacity 350ms ease",
              filter: "drop-shadow(0 0 6px var(--brand-mid))",
            }}
          >
            <circle cx={C} cy={C - R_ARC} r={6.5} fill="var(--brand)" />
            <circle cx={C} cy={C - R_ARC} r={2.2} fill="var(--txt)" />
          </g>
        </g>
      </svg>

      {/* the centre: start control at rest, live reading while measuring */}
      <div className="pointer-events-none absolute inset-0">
        <button
          type="button"
          onClick={onStart}
          disabled={running}
          aria-label={phase === "done" ? t("startAgain") : t("startTest")}
          title={phase === "done" ? t("startAgain") : undefined}
          className={cn(
            "absolute left-1/2 top-1/2 grid size-[186px] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full border transition-[opacity,border-color,transform] duration-300 [@media(max-height:660px)]:size-[136px]",
            "border-[var(--line-strong)] bg-black/30 shadow-[inset_0_1px_0_rgb(255_255_255/0.10)] backdrop-blur-[14px] backdrop-saturate-150",
            "hover:border-[var(--brand-line)] active:scale-[0.985] [-webkit-tap-highlight-color:transparent]",
            atRest ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0",
          )}
        >
          <span className="text-xl font-bold tracking-[0.1em] text-txt [@media(max-height:660px)]:text-lg">GO</span>
        </button>

        <div
          className={cn(
            "absolute inset-0 grid place-items-center transition-[opacity,transform] duration-300",
            atRest ? "scale-95 opacity-0" : "scale-100 opacity-100",
          )}
        >
          <div className="flex flex-col items-center">
            <span className="text-[10px] font-semibold uppercase tracking-[0.22em] text-txt3 [@media(max-height:660px)]:hidden">
              {phaseLabel}
            </span>
            <span className="mt-1 font-display text-[42px] font-semibold leading-none tracking-[-0.04em] tabular-nums text-txt [@media(max-height:660px)]:mt-0 [@media(max-height:660px)]:text-2xl">
              {display}
            </span>
            <span className="mt-2 flex items-center gap-1.5 text-xs text-txt2">
              <Icon className="size-3.5 text-brand-strong" strokeWidth={1.9} aria-hidden />
              <span>{unit}</span>
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}
