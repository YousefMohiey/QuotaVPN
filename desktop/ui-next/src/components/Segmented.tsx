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
  options: Array<{ value: T; label: string }>
  onChange: (v: T) => void
  className?: string
  fill?: boolean
  boxed?: boolean
}) {
  return (
    <div
      className={cn(
        "rounded-[10px] border border-line bg-white/[0.02]",
        boxed ? "flex h-12 w-full items-center p-1" : "p-0.5",
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
            className={cn(
              "relative rounded-[8px] transition-colors",
              boxed
                ? cn("relative flex h-full min-w-0 flex-1 items-center justify-center px-0.5 text-[12px]", on ? "text-white" : "text-txt2 hover:text-txt")
                : cn("py-1.5", fill ? "min-w-0 flex-1 px-1.5 text-[12.5px]" : "px-3 text-[12.5px]", on ? "text-txt" : "text-txt3 hover:text-txt2"),
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className={cn(
                  "absolute inset-0 rounded-[8px]",
                  boxed ? "border border-[var(--brand)] bg-[var(--brand-mid)]" : "bg-white/[0.1]",
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
