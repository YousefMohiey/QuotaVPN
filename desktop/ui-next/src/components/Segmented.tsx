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
    <div className={cn("rounded-[13px] border border-line bg-white/[0.02] p-[3px]", fill ? "flex w-full" : "inline-flex", className)}>
      {options.map((o) => {
        const on = o.value === value
        const prevOn = options[options.indexOf(o) - 1]?.value === value
        return (
          <button
            key={o.value}
            type="button"
            onClick={() => onChange(o.value)}
            aria-pressed={on}
            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",
              fill && "flex-1",
              "not-first:border-l not-first:border-line not-first:rounded-l-none",
              on || prevOn ? "border-l-transparent" : "",
              on ? "text-white" : "text-txt3 hover:text-txt2",
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className="seg-on absolute inset-0 rounded-[10px]"
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
