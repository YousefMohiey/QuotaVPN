import { useMemo } from "react"
import { useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

const W = 560
const H = 96

/**
 * The run's own shape: one line per sample, drawn left to right while the
 * run goes. Idle the pane simply waits: no grid, no baseline, nothing to
 * explain until there is a line to show.
 */
export function SpeedGraph({
  samples,
  accent,
  active,
  fill = false,
}: {
  samples: number[]
  accent: string
  active: boolean
  /** Stretch to the parent's height instead of the fixed strip. */
  fill?: boolean
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
    <div className={cn("relative", fill && "flex h-full w-full flex-col")}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className={cn("w-full", fill ? "min-h-0 flex-1" : "h-[96px]")}
        role="img"
        aria-label={active ? t("measuring") : t("graphLabel")}
      >
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
    </div>
  )
}
