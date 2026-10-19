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
          ? "flex h-9 w-full items-center rounded-[8px] bg-white/[0.045] p-0.5"
          : "rounded-[7px] border border-line bg-white/[0.02] p-0.5",
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
              "group/seg relative transition-colors",
              boxed
                ? cn(
                    "flex h-full min-w-0 flex-1 items-center justify-center rounded-[7px] px-2 text-[12.5px] font-medium",
                    on ? "text-txt" : "text-txt3 hover:text-txt2",
                  )
                : cn("rounded-[6px] py-1.5", fill ? "min-w-0 flex-1 px-1.5 text-[12px]" : "px-3 text-[12px]", on ? "text-txt" : "text-txt3 hover:text-txt2"),
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className={cn(
                  "absolute inset-0",
                  boxed ? "rounded-[7px] border border-[var(--brand-line)] bg-[var(--brand-bg)]" : "rounded-[6px] bg-white/[0.1]",
                )}
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}
            <span className="relative truncate" dir="auto">{o.label}</span>
            {o.hint ? (
              <span
                role="tooltip"
                className="pointer-events-none absolute bottom-[calc(100%+7px)] end-0 z-40 translate-y-1 whitespace-nowrap rounded-[7px] border border-line bg-[rgb(26_27_33/0.98)] px-2.5 py-1 text-[11.5px] font-medium leading-[1.25] text-txt2 opacity-0 shadow-[0_10px_26px_rgb(0_0_0/0.55)] transition-[opacity,transform] duration-150 group-hover/seg:translate-y-0 group-hover/seg:opacity-100 group-focus-visible/seg:translate-y-0 group-focus-visible/seg:opacity-100"
              >
                {o.hint}
              </span>
            ) : null}
          </button>
        )
      })}
    </div>
  )
}
