import { useMemo } from "react"
import type { LucideIcon } from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

// The dial the whole world knows, dressed in the app's colors. Every
// geometry constant below is Ookla's own, lifted from their gauge code and
// scaled to our 300 box: ring radius 130, stroke 23, labels bold at
// radius-33, a blunt trapezoid needle (base 20, tip 10, length 85) that
// pivots from the center, the readout riding in the bottom opening.
// Scale: 0..1000 over a 270 degree sweep, label i at i/8 of the sweep, so
// 100 sits at high noon and the sides mirror; the needle slides linearly
// between the two stops it sits among. Measured off the reference app.
//
// Interaction follows the reference app too: while there is nothing to
// show, GO sits dead center and starts the run; the moment it starts the
// word fades, the scale numbers flash white in sequence, the needle drops
// in, and the reading lands in the opening. When the run ends GO returns.

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

export type DialPhase = "idle" | "ping" | "download" | "upload" | "done"

export function SpeedDial({
  display,
  value,
  unit,
  icon: Icon,
  phase,
  runId,
  onStart,
  onStop,
}: {
  display: string
  value: number
  unit: string
  icon: LucideIcon
  phase: DialPhase
  runId: number
  onStart: () => void
  onStop: () => void
}) {
  const { t } = useI18n()
  const idle = phase === "idle"
  const running = phase === "ping" || phase === "download" || phase === "upload"

  const frac = fracFor(value)
  const needleDeg = angleFor(frac)

  const arcLen = 2 * Math.PI * R_ARC * (SWEEP / 360)
  const arcStart = pt(R_ARC, START)
  const arcEnd = pt(R_ARC, START + SWEEP)
  const arcPath = `M ${arcStart.x.toFixed(2)} ${arcStart.y.toFixed(2)} A ${R_ARC} ${R_ARC} 0 1 1 ${arcEnd.x.toFixed(2)} ${arcEnd.y.toFixed(2)}`

  const b = NEEDLE_BASE_W / 2
  const tipW = NEEDLE_TIP_W / 2
  const needlePath = `M ${C - b} ${C} L ${C - tipW} ${C - R_TIP} L ${C + tipW} ${C - R_TIP} L ${C + b} ${C} Z`

  const labels = useMemo(
    () =>
      STOPS.map((s, i) => ({
        s,
        pos: pt(R_LABEL, angleFor(FRACS[i])),
      })),
    [],
  )

  return (
    <div
      className={cn(
        "relative h-[240px] w-[240px] [@media(max-height:660px)]:h-[156px] [@media(max-height:660px)]:w-[156px]",
        phase === "done" && "cursor-pointer",
      )}
      onClick={phase === "done" ? onStart : undefined}
      title={phase === "done" ? t("startAgain") : undefined}
    >
      <svg viewBox={`0 0 ${VIEW} ${VIEW}`} className="h-full w-full" aria-hidden focusable="false">
        <defs>
          <linearGradient id="qcDialFill" x1="0" y1="0.8" x2="1" y2="0.2">
            <stop offset="0" stopColor="#6fa3f2" />
            <stop offset="1" stopColor="#1d5bd6" />
          </linearGradient>
          {/* their dark-theme needle: light blade at the tip, fading home */}
          <linearGradient id="qcDialNeedle" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#f2f4f9" stopOpacity="0.95" />
            <stop offset="0.65" stopColor="#c3cad6" stopOpacity="0.65" />
            <stop offset="1" stopColor="#7d879c" stopOpacity="0.35" />
          </linearGradient>
        </defs>

        {/* the working face: track, lit stretch, scale and needle. Only
            present while a run or a result exists. */}
        <g style={{ opacity: idle ? 0 : 1, transition: "opacity 400ms ease" }}>
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
              filter: "drop-shadow(0 0 5px rgb(29 91 214 / 0.5))",
            }}
          />

          {/* the scale, flushed white in sequence whenever a run begins */}
          <g key={runId}>
            {labels.map(({ s, pos }, i) => (
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
                className={runId > 0 ? "qc-lbl-flash" : undefined}
                style={runId > 0 ? { animationDelay: `${i * 80}ms` } : undefined}
              >
                {s}
              </text>
            ))}
          </g>

          {/* the needle, pivoting from the center */}
          <g
            style={{
              transform: `rotate(${needleDeg}deg)`,
              transformBox: "view-box",
              transformOrigin: `${C}px ${C}px`,
              transition: "transform 170ms cubic-bezier(0.22, 1, 0.36, 1), opacity 350ms ease",
              opacity: idle ? 0 : 1,
            }}
          >
            <path
              d={needlePath}
              fill="url(#qcDialNeedle)"
              style={{ filter: "drop-shadow(0 1px 5px rgb(0 0 0 / 0.5))" }}
            />
          </g>
        </g>
      </svg>

      {/* the center: the start button wears the home page dial's dress
          (glass circle, one lit line), then a quiet stop while the run
          goes, and the reading down in the opening once measured */}
      <div className="pointer-events-none absolute inset-0">
        <button
          type="button"
          onClick={onStart}
          disabled={!idle}
          aria-label={t("startTest")}
          className={cn(
            "absolute left-1/2 top-1/2 grid size-[172px] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full border transition-[opacity,border-color,transform] duration-300 [@media(max-height:660px)]:size-[150px]",
            "border-[rgb(255_255_255/0.14)] bg-white/[0.05] shadow-[inset_0_1px_0_rgb(255_255_255/0.10)] backdrop-blur-[14px] backdrop-saturate-150",
            "hover:border-[rgb(255_255_255/0.26)] active:scale-[0.985] [-webkit-tap-highlight-color:transparent]",
            idle ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0",
          )}
        >
          <span className="text-[26px] font-bold tracking-[0.06em] text-txt">GO</span>
        </button>

        {running && (
          <button
            type="button"
            onClick={onStop}
            className="pointer-events-auto absolute left-1/2 top-[calc(50%+32px)] -translate-x-1/2 -translate-y-1/2 rounded-full px-3 py-1 text-[11.5px] font-medium text-txt3 transition-[color,background-color] duration-200 [@media(max-height:660px)]:top-[calc(50%+26px)] hover:bg-white/[0.04] hover:text-txt"
          >
            {t("stop")}
          </button>
        )}

        <div className="absolute left-1/2 top-[calc(50%+78px)] -translate-x-1/2 -translate-y-1/2 [@media(max-height:660px)]:top-[calc(50%+51px)]">
          <div
            className={cn(
              "flex flex-col items-center transition-[opacity,transform] duration-300",
              idle ? "translate-y-2 opacity-0" : "translate-y-0 opacity-100",
            )}
          >
            <span
              className="text-[32px] font-mono font-medium leading-none tabular-nums text-txt [@media(max-height:660px)]:text-[22px]"
              style={{ letterSpacing: "-0.01em" }}
            >
              {display}
            </span>
            <span className="mt-2 flex items-center gap-1.5">
              <Icon className="size-3.5 text-brand-strong" strokeWidth={1.9} aria-hidden />
              <span className="text-[11.5px] text-txt2 [@media(max-height:660px)]:text-[9.5px]">
                {unit}
              </span>
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}
