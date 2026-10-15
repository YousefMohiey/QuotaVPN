import { useMemo } from "react"

// The dial reads like the speed test everyone knows: a tick ring and a
// needle that sweeps with the live number riding in the middle. Angles are
// measured from twelve o'clock, clockwise, with the gap at the bottom.

const VIEW = 240
const C = VIEW / 2
const START = -135
const SWEEP = 270
const TICKS = 40

const R_TICK_MAJOR_OUT = 118
const R_TICK_MAJOR_IN = 103
const R_TICK_MINOR_IN = 110
const R_LABEL = 90
const R_ARC = 102
const NEEDLE_IN = 56
const NEEDLE_OUT = 84

const pt = (r: number, deg: number) => {
  const a = (deg * Math.PI) / 180
  return { x: C + r * Math.sin(a), y: C - r * Math.cos(a) }
}

const angleFor = (t: number) => START + SWEEP * Math.max(0, Math.min(1, t))

/** A speed-test dial: the ring, the labels, a soft progress arc and the
    needle. Digits and unit ride in the middle; at rest the needle sits at
    zero and the number shows an unloaded "---". */
export function SpeedDial({
  display,
  value,
  max,
  unit,
  idle,
}: {
  display: string
  value: number
  max: number
  unit: string
  idle: boolean
}) {
  const t = max > 0 ? Math.max(0, Math.min(1, value / max)) : 0
  const needleDeg = angleFor(t)

  const ticks = useMemo(() => {
    const out: Array<{ deg: number; major: boolean; label?: string }> = []
    for (let i = 0; i <= TICKS; i++) {
      const deg = angleFor(i / TICKS)
      const major = i % (TICKS / 4) === 0
      out.push({ deg, major, label: major ? String(Math.round((max * i) / TICKS)) : undefined })
    }
    return out
  }, [max])

  const arcLen = 2 * Math.PI * R_ARC * (SWEEP / 360)
  const arcStart = pt(R_ARC, START)
  const arcEnd = pt(R_ARC, START + SWEEP)
  const arcPath = `M ${arcStart.x.toFixed(2)} ${arcStart.y.toFixed(2)} A ${R_ARC} ${R_ARC} 0 1 1 ${arcEnd.x.toFixed(2)} ${arcEnd.y.toFixed(2)}`

  return (
    <div className="relative h-[256px] w-[256px] [@media(max-height:660px)]:h-[200px] [@media(max-height:660px)]:w-[200px]">
      <svg viewBox={`0 0 ${VIEW} ${VIEW}`} className="h-full w-full" aria-hidden focusable="false">
        {/* the scale */}
        {ticks.map(({ deg, major }) => {
          const inner = major ? R_TICK_MAJOR_IN : R_TICK_MINOR_IN
          const a = pt(inner, deg)
          const b = pt(R_TICK_MAJOR_OUT, deg)
          return (
            <line
              key={deg}
              x1={a.x.toFixed(2)}
              y1={a.y.toFixed(2)}
              x2={b.x.toFixed(2)}
              y2={b.y.toFixed(2)}
              stroke={major ? "rgb(255 255 255 / 0.22)" : "rgb(255 255 255 / 0.10)"}
              strokeWidth={major ? 1.8 : 1.4}
              strokeLinecap="round"
            />
          )
        })}
        {ticks.map(({ deg, label }) => {
          if (!label) return null
          const a = pt(R_LABEL, deg)
          return (
            <text
              key={`l${deg}`}
              x={a.x.toFixed(2)}
              y={a.y.toFixed(2)}
              textAnchor="middle"
              dominantBaseline="middle"
              fontSize="10.5"
              fill="var(--color-txt2)"
            >
              {label}
            </text>
          )
        })}

        {/* how far the run has come up the scale */}
        <path
          d={arcPath}
          fill="none"
          stroke="var(--brand-vivid)"
          strokeWidth="3.5"
          strokeLinecap="round"
          opacity="0.9"
          style={{
            strokeDasharray: arcLen,
            strokeDashoffset: arcLen * (1 - t),
            transition: "stroke-dashoffset 170ms cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        />

        {/* the needle */}
        <g
          style={{
            transform: `rotate(${needleDeg}deg)`,
            transformBox: "view-box",
            transformOrigin: `${C}px ${C}px`,
            transition: "transform 170ms cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        >
          <line
            x1={C}
            y1={C - NEEDLE_IN}
            x2={C}
            y2={C - NEEDLE_OUT}
            stroke="var(--brand-vivid)"
            strokeWidth="2.5"
            strokeLinecap="round"
          />
          {/* the tail keeps the needle reading as one anchored pointer */}
          <circle cx={C} cy={C - NEEDLE_IN} r="2.2" fill="var(--brand-vivid)" />
        </g>
      </svg>

      {/* the readout in the middle */}
      <div className="pointer-events-none absolute inset-0 grid place-items-center">
        <div className="flex flex-col items-center">
          <span
            className="text-[36px] font-light leading-none tabular-nums text-txt [@media(max-height:660px)]:text-[30px]"
            style={{ letterSpacing: "-0.02em" }}
          >
            {idle ? "---" : display}
          </span>
          <span className="mt-1.5 text-[11.5px] text-txt3">{unit}</span>
        </div>
      </div>
    </div>
  )
}
