import { useMemo } from "react"
import { useI18n } from "@/lib/i18n"

const W = 560
const H = 96

/**
 * The run's own shape: one line per sample, drawn left to right while the
 * run goes. Idle it is an empty scale (hairlines only); during a run the
 * line is alive with the newest point marked; when the run ends it stays
 * put as the fingerprint of that connection.
 */
export function SpeedGraph({
  samples,
  accent,
  active,
}: {
  samples: number[]
  accent: string
  active: boolean
}) {
  const { t } = useI18n()

  const { line, area, last, has } = useMemo(() => {
    const values = samples.slice(-110)
    const peak = Math.max(1, ...values)
    const step = W / Math.max(1, values.length - 1)
    const pts = values.map((v, i) => [i * step, H - (v / peak) * (H - 12) - 6] as const)
    const d = pts.map(([x, y], i) => `${i ? "L" : "M"} ${x.toFixed(1)} ${y.toFixed(1)}`).join(" ")
    return {
      line: d,
      area: pts.length > 1 ? `${d} L ${W} ${H} L 0 ${H} Z` : "",
      last: pts.length ? pts[pts.length - 1] : null,
      has: values.length > 0,
    }
  }, [samples])

  return (
    <div className="relative">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="h-[96px] w-full"
        role="img"
        aria-label={active ? t("measuring") : t("graphLabel")}
      >
        {/* scale: three hairlines so the line reads as a measurement */}
        <g stroke="rgb(255 255 255 / 0.06)" strokeWidth="1">
          <line x1="0" y1={H / 4} x2={W} y2={H / 4} />
          <line x1="0" y1={H / 2} x2={W} y2={H / 2} />
          <line x1="0" y1={(H * 3) / 4} x2={W} y2={(H * 3) / 4} />
        </g>
        {has && area && <path d={area} fill={`color-mix(in oklab, ${accent} 14%, transparent)`} />}
        {has && (
          <path
            d={line}
            fill="none"
            stroke={accent}
            strokeWidth="1.8"
            vectorEffect="non-scaling-stroke"
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        )}
        {has && last && active && <circle cx={last[0]} cy={last[1]} r="3" fill={accent} />}
      </svg>
      {/* baseline: without it the shape reads as decoration, not a scale */}
      <div className="mt-1 h-px w-full bg-[rgb(255_255_255/0.10)]" />
    </div>
  )
}
