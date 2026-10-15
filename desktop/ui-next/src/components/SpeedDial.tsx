import { useMemo } from "react"
import type { LucideIcon } from "lucide-react"

// The dial the whole world knows, dressed in the app's colors. Every
// geometry constant below is Ookla's own, lifted from their gauge code and
// scaled to our 300 box: ring radius 130, stroke 23, labels bold at
// radius-33, a blunt trapezoid needle (base 20, tip 10, length 85) that
// pivots from the center, the readout riding in the bottom opening.
// Scale: 0..1000 over a 270 degree sweep, label i at i/8 of the sweep, so
// 100 sits at high noon and the sides mirror; the needle slides linearly
// between the two stops it sits among. Measured off the reference app.

const VIEW = 300
const C = VIEW / 2
const START = -135
const SWEEP = 270

const R_ARC = 136.5 // ring 130 of a 286 box, scaled to 300
const STROKE = 24 // stroke width 23, scaled
const R_LABEL = 101.5 // labels at radius-33, scaled
const R_TIP = 89 // needle length 85, scaled
const NEEDLE_BASE_W = 21 // needle base width 20, scaled
const NEEDLE_TIP_W = 10.5 // needle tip width 10, scaled

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

  const b = NEEDLE_BASE_W / 2
  const t = NEEDLE_TIP_W / 2
  const needlePath = `M ${C - b} ${C} L ${C - t} ${C - R_TIP} L ${C + t} ${C - R_TIP} L ${C + b} ${C} Z`

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
          {/* their dark-theme needle: light blade at the tip, fading home */}
          <linearGradient id="qcDialNeedle" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#f2f4f9" stopOpacity="0.95" />
            <stop offset="0.65" stopColor="#c3cad6" stopOpacity="0.65" />
            <stop offset="1" stopColor="#7d879c" stopOpacity="0.35" />
          </linearGradient>
        </defs>

        {/* the track */}
        <path d={arcPath} fill="none" stroke="#2c3050" strokeWidth={STROKE} strokeLinecap="butt" />

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

        {/* the scale, numbers bold the way the original prints them */}
        {labels.map(({ s, pos }) => (
          <text
            key={s}
            x={pos.x.toFixed(2)}
            y={pos.y.toFixed(2)}
            textAnchor="middle"
            dominantBaseline="middle"
            fontSize="14.5"
            fontWeight="600"
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
            d={needlePath}
            fill="url(#qcDialNeedle)"
            style={{ filter: "drop-shadow(0 1px 5px rgb(0 0 0 / 0.5))" }}
          />
        </g>
      </svg>

      {/* the readout riding in the bottom opening, like the original */}
      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
        <div className="flex translate-y-[98px] flex-col items-center [@media(max-height:660px)]:translate-y-[64px]">
          <span
            className="text-[40px] font-mono font-medium leading-none tabular-nums text-txt [@media(max-height:660px)]:text-[27px]"
            style={{ letterSpacing: "-0.01em" }}
          >
            {idle ? "---" : display}
          </span>
          <span className="mt-2 flex items-center gap-1.5">
            <Icon className="size-4 text-brand-strong" strokeWidth={1.9} aria-hidden />
            <span className="text-[14px] text-txt2 [@media(max-height:660px)]:text-[11px]">
              {unit}
            </span>
          </span>
        </div>
      </div>
    </div>
  )
}
