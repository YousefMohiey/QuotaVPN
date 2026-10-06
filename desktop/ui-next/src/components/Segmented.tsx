import { motion } from "motion/react"
import { cn } from "@/lib/utils"

/** Small segmented control with one shared indicator that slides. */
export function Segmented<T extends string>({
  id,
  value,
  options,
  onChange,
  className,
  fill,
}: {
  id: string
  value: T
  options: Array<{ value: T; label: string }>
  onChange: (v: T) => void
  className?: string
  fill?: boolean
}) {
  return (
    <div className={cn("rounded-[10px] border border-line bg-white/[0.02] p-0.5", fill ? "flex w-full" : "inline-flex", className)}>
      {options.map((o) => {
        const on = o.value === value
        return (
          <button
            key={o.value}
            type="button"
            onClick={() => onChange(o.value)}
            aria-pressed={on}
            className={cn(
              "relative rounded-[8px] px-3 py-1.5 text-[12.5px] transition-colors",
              fill && "flex-1",
              on ? "text-txt" : "text-txt3 hover:text-txt2",
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className="absolute inset-0 rounded-[8px] bg-white/[0.1]"
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}
            <span className="relative" dir="auto">{o.label}</span>
          </button>
        )
      })}
    </div>
  )
}
