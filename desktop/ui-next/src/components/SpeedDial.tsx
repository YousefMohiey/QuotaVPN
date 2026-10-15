import { useMemo } from "react"
import type { LucideIcon } from "lucide-react"

// The dial the whole world knows, dressed in the app's colors: a thick
// open-bottom track, a non-linear 0-1000 scale with no ticks, a tapered
// needle pivoting from the center, and the readout riding just below the
// pivot. The covered stretch of the track lights up as the run climbs.

const VIEW = 280
const C = VIEW / 2
const START = -135
const SWEEP = 270

const R_ARC = 116
const STROKE = 19
const R_LABEL = 97
const R_TIP = 84

// Speedtest's own scale: the stops and where they sit along the sweep.
const STOPS = [0, 5, 10, 50, 100, 250, 500, 750, 1000]
const FRACS = [0, 0.035, 0.075, 0.285, 0.5, 0.64, 0.775, 0.89, 1]

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

export function SpeedDial({
  display,
  value,
  unit,
  icon: Icon,
  idle,
}: {
  display: string
  value: number
  unit: string
  icon: LucideIcon
  idle: boolean
}) {
  const frac = fracFor(value)
  const needleDeg = angleFor(frac)

  const arcLen = 2 * Math.PI * R_ARC * (SWEEP / 360)
  const arcStart = pt(R_ARC, START)
  const arcEnd = pt(R_ARC, START + SWEEP)
  const arcPath = `M ${arcStart.x.toFixed(2)} ${arcStart.y.toFixed(2)} A ${R_ARC} ${R_ARC} 0 1 1 ${arcEnd.x.toFixed(2)} ${arcEnd.y.toFixed(2)}`

  const labels = useMemo(
    () =>
      STOPS.map((s, i) => ({
        s,
        pos: pt(R_LABEL, angleFor(FRACS[i])),
      })),
    [],
  )

  return (
    <div className="relative h-[280px] w-[280px] [@media(max-height:660px)]:h-[198px] [@media(max-height:660px)]:w-[198px]">
      <svg viewBox={`0 0 ${VIEW} ${VIEW}`} className="h-full w-full" aria-hidden focusable="false">
        <defs>
          <linearGradient id="qcDialFill" x1="0" y1="1" x2="0.55" y2="0">
            <stop offset="0" stopColor="#8fbcff" />
            <stop offset="1" stopColor="#2e7bf6" />
          </linearGradient>
          <linearGradient id="qcDialNeedle" x1="0" y1="1" x2="0" y2="0">
            <stop offset="0" stopColor="#69738a" />
            <stop offset="1" stopColor="#e9edf5" />
          </linearGradient>
        </defs>

        {/* the track */}
        <path
          d={arcPath}
          fill="none"
          stroke="#243353"
          strokeWidth={STROKE}
          strokeLinecap="butt"
        />

        {/* the covered stretch, lit */}
        <path
          d={arcPath}
          fill="none"
          stroke="url(#qcDialFill)"
          strokeWidth={STROKE}
          strokeLinecap="butt"
          style={{
            strokeDasharray: arcLen,
            strokeDashoffset: arcLen * (1 - frac),
            transition: "stroke-dashoffset 170ms cubic-bezier(0.22, 1, 0.36, 1)",
            filter: "drop-shadow(0 0 6px rgb(46 123 246 / 0.45))",
          }}
        />

        {/* the scale, plain numbers the way the original does it */}
        {labels.map(({ s, pos }) => (
          <text
            key={s}
            x={pos.x.toFixed(2)}
            y={pos.y.toFixed(2)}
            textAnchor="middle"
            dominantBaseline="middle"
            fontSize="12"
            fontWeight="500"
            fill="var(--color-txt2)"
          >
            {s}
          </text>
        ))}

        {/* the needle, pivoting from the center */}
        <g
          style={{
            transform: `rotate(${needleDeg}deg)`,
            transformBox: "view-box",
            transformOrigin: `${C}px ${C}px`,
            transition: "transform 170ms cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        >
          <path
            d={`M ${C - 4} ${C} L ${C} ${C - R_TIP} L ${C + 4} ${C} Z`}
            fill="url(#qcDialNeedle)"
            stroke="#0c0d10"
            strokeOpacity="0.55"
            strokeWidth="0.75"
            strokeLinejoin="round"
            style={{ filter: "drop-shadow(0 2px 5px rgb(0 0 0 / 0.5))" }}
          />
        </g>
      </svg>

      {/* the readout riding just below the pivot, like the original */}
      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
        <div className="flex translate-y-[44px] flex-col items-center [@media(max-height:660px)]:translate-y-[30px]">
          <span
            className="text-[38px] font-light leading-none tabular-nums text-txt"
            style={{ letterSpacing: "-0.02em" }}
          >
            {idle ? "---" : display}
          </span>
          <span className="mt-2 flex items-center gap-1.5">
            <Icon className="size-3.5 text-brand-strong" strokeWidth={1.9} aria-hidden />
            <span className="text-[12px] text-txt3">{unit}</span>
          </span>
        </div>
      </div>
    </div>
  )
}
