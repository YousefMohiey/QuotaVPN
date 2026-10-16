import { motion } from "motion/react"
import { cn } from "@/lib/utils"

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
  return (
    <div
      className={cn(
        /* boxed is the Home shorthand: one joined frame, three equal
           segments, the pick carried by a soft brand pill that slides */
        boxed
          ? "flex h-10 w-full items-center rounded-[10px] border border-line bg-white/[0.02] p-0.5"
          : "rounded-[10px] border border-line bg-white/[0.02] p-0.5",
        !boxed && (fill ? "flex w-full" : "inline-flex"),
        className,
      )}
    >
      {options.map((o) => {
        const on = o.value === value
        return (
          <button
            key={o.value}
            type="button"
            onClick={() => onChange(o.value)}
            aria-pressed={on}
            title={o.hint}
            className={cn(
              "relative transition-colors",
              boxed
                ? cn(
                    "flex h-full min-w-0 flex-1 items-center justify-center rounded-[8px] px-1.5 text-[12px]",
                    on ? "text-txt" : "text-txt2 hover:text-txt",
                  )
                : cn("rounded-[8px] py-1.5", fill ? "min-w-0 flex-1 px-1.5 text-[12.5px]" : "px-3 text-[12.5px]", on ? "text-txt" : "text-txt3 hover:text-txt2"),
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className={cn(
                  "absolute inset-0",
                  boxed ? "rounded-[8px] border border-[var(--brand-line)] bg-[var(--brand-bg)]" : "rounded-[8px] bg-white/[0.1]",
                )}
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}
            <span className="relative truncate" dir="auto">{o.label}</span>
          </button>
        )
      })}
    </div>
  )
}
