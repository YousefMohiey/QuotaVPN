import { motion, useReducedMotion } from "motion/react"
import { cn } from "@/lib/utils"
import { Tip } from "@/components/Tip"

/** Small segmented control with one shared indicator that slides.
    `boxed` gives it the same frame as the dropdown controls on Home. */
export function Segmented<T extends string>({
  id,
  value,
  options,
  onChange,
  className,
  fill,
  boxed,
}: {
  id: string
  value: T
  options: Array<{ value: T; label: string; hint?: string }>
  onChange: (v: T) => void
  className?: string
  fill?: boolean
  boxed?: boolean
}) {
  const calm = useReducedMotion()
  return (
    <div
      className={cn(
        /* boxed is the Home shorthand: one joined frame, three equal
           segments, the pick carried by a soft brand pill that slides */
        boxed
          ? "flex h-12 w-full items-center rounded-xl bg-white/[0.05] p-1"
          : "rounded-lg border border-line bg-[var(--surface-2)] p-0.5",
        !boxed && (fill ? "flex w-full" : "inline-flex"),
        className,
      )}
    >
      {options.map((o, i) => {
        const on = o.value === value
        // A hint on the first or last segment runs off the frame, so the
        // bubble leans inwards from that edge instead of centring on itself.
        const lean = i === 0 ? "start" : i === options.length - 1 ? "end" : "center"
        const btn = (
          <button
            key={o.value}
            type="button"
            onClick={() => onChange(o.value)}
            aria-pressed={on}
            className={cn(
              "group/seg relative transition-colors",
              boxed
                ? cn(
                    "flex h-full min-w-0 flex-1 items-center justify-center rounded-lg px-2 text-sm font-medium",
                    on ? "text-txt" : "text-txt3 hover:text-txt",
                  )
                : cn("rounded-md py-2", fill ? "min-w-0 flex-1 px-2 text-xs" : "px-3 text-xs", on ? "text-txt" : "text-txt3 hover:text-txt2"),
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className={cn(
                  "absolute inset-0",
                  boxed ? "rounded-lg border border-[var(--brand-line)] bg-[var(--brand-bg)]" : "rounded-md bg-white/[0.1]",
                )}
                transition={calm ? { duration: 0 } : { type: "spring", stiffness: 520, damping: 40 }}
              />
            )}
            <span className="relative truncate" dir="auto">{o.label}</span>
          </button>
        )
        return o.hint ? (
          <Tip key={o.value} text={o.hint} align={lean} className={cn("min-w-0", boxed ? "flex-1 self-stretch" : fill ? "flex-1" : "")}>
            {btn}
          </Tip>
        ) : (
          btn
        )
      })}
    </div>
  )
}
