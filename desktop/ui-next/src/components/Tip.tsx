import type { ReactNode } from "react"
import { cn } from "@/lib/utils"

/** A hover hint in the app's own language: a small dark bubble with a
    hairline and a soft shadow riding above the control. Replaces the OS
    tooltip so nothing foreign appears over the surface. */
export function Tip({
  text,
  children,
  className,
}: {
  text: string
  children: ReactNode
  className?: string
}) {
  return (
    <span className={cn("group/tip relative flex w-fit max-w-full min-w-0", className)}>
      {children}
      {text ? (
        <span
          role="tooltip"
          className="pointer-events-none absolute bottom-[calc(100%+7px)] left-1/2 z-40 -translate-x-1/2 translate-y-1 whitespace-nowrap rounded-[7px] border border-line bg-[rgb(26_27_33/0.98)] px-2.5 py-1 text-[11.5px] font-medium leading-[1.25] text-txt2 opacity-0 shadow-[0_10px_26px_rgb(0_0_0/0.55)] transition-[opacity,transform] duration-150 group-hover/tip:translate-y-0 group-hover/tip:opacity-100 group-focus-within/tip:translate-y-0 group-focus-within/tip:opacity-100"
        >
          {text}
        </span>
      ) : null}
    </span>
  )
}
