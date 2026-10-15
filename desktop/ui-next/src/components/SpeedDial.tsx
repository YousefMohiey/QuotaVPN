import { useMemo } from "react"
import type { LucideIcon } from "lucide-react"

// The dial the whole world knows, dressed in the app's colors: a thick
// open-bottom track, a non-linear 0-1000 scale with no ticks, a tapered
// needle pivoting from the center, and the readout riding just below the
// pivot. The covered stretch of the track lights up as the run climbs.
// The construction follows OpenSpeedTest (MIT): dasharray driven fill and
// round-joined strokes, so the arc paints the same way the original does.

const VIEW = 300
const C = VIEW / 2
const START = -135
const SWEEP = 270

const R_ARC = 124
const STROKE = 21
const R_LABEL = 103
const R_TIP = 96

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
    <div className="relative h-[300px] w-[300px] [@media(max-height:660px)]:h-[196px] [@media(max-height:660px)]:w-[196px]">
      <svg viewBox={`0 0 ${VIEW} ${VIEW}`} className="h-full w-full" aria-hidden focusable="false">
        <defs>
          <linearGradient id="qcDialFill" x1="0" y1="0.8" x2="1" y2="0.2">
            <stop offset="0" stopColor="#9cc8ff" />
            <stop offset="1" stopColor="#3b8bff" />
          </linearGradient>
          <linearGradient id="qcDialNeedle" x1="0" y1="1" x2="0" y2="0">
            <stop offset="0" stopColor="#59637a" />
            <stop offset="1" stopColor="#f0f3f9" />
          </linearGradient>
        </defs>

        {/* the track */}
        <path
          d={arcPath}
          fill="none"
          stroke="#2c3050"
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
            filter: "drop-shadow(0 0 5px rgb(46 123 246 / 0.45))",
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
            fontSize="14"
            fontWeight="500"
            fill="var(--color-txt)"
            fillOpacity="0.72"
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
            d={`M ${C - 3.4} ${C} L ${C} ${C - R_TIP} L ${C + 3.4} ${C} Z`}
            fill="url(#qcDialNeedle)"
            stroke="#0a0c10"
            strokeOpacity="0.6"
            strokeWidth="0.9"
            strokeLinejoin="round"
            style={{ filter: "drop-shadow(0 2px 6px rgb(0 0 0 / 0.55))" }}
          />
        </g>
      </svg>

      {/* the readout riding just below the pivot, like the original */}
      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
        <div className="flex translate-y-[48px] flex-col items-center [@media(max-height:660px)]:translate-y-[36px]">
          <span
            className="text-[46px] font-light leading-none tabular-nums text-txt"
            style={{ letterSpacing: "-0.02em" }}
          >
            {idle ? "---" : display}
          </span>
          <span className="mt-2.5 flex items-center gap-1.5">
            <Icon className="size-4 text-brand-strong" strokeWidth={1.9} aria-hidden />
            <span className="text-[12.5px] text-txt2">{unit}</span>
          </span>
        </div>
      </div>
    </div>
  )
}
