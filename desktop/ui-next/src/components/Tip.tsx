import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from "react"
import { createPortal } from "react-dom"
import { cn } from "@/lib/utils"

type Align = "center" | "start" | "end"

/**
 * A hover hint in the app's own language.
 *
 * The bubble is portalled to the body and placed from the trigger's own
 * rectangle. It used to live inside the control, which meant every panel and
 * scroll area with overflow clipped it: on Home a hint at the right edge came
 * out cut in half, and a long one ran past the window. It now flips below the
 * control when there is no room above, and its x is clamped once measured.
 */
export function Tip({
  text,
  children,
  className,
  align = "center",
}: {
  text: string
  children: ReactNode
  className?: string
  align?: Align
}) {
  const anchor = useRef<HTMLSpanElement>(null)
  const bubble = useRef<HTMLSpanElement>(null)
  const [place, setPlace] = useState<{ top: number; left: number; above: boolean } | null>(null)

  const placeNow = useCallback(() => {
    const el = anchor.current
    if (!el || !text) return
    const r = el.getBoundingClientRect()
    const above = r.top > 56
    const left = align === "start" ? r.left : align === "end" ? r.right : r.left + r.width / 2
    setPlace({ top: above ? r.top - 8 : r.bottom + 8, left, above })
  }, [text, align])

  // Clamp after paint: the width is only known once the bubble exists, and a
  // hint on the right edge of the window would otherwise sit half outside.
  useLayoutEffect(() => {
    if (!place || !bubble.current) return
    const w = bubble.current.offsetWidth
    const margin = 10
    const min = align === "center" ? w / 2 + margin : align === "end" ? w + margin : margin
    const max =
      align === "center"
        ? window.innerWidth - w / 2 - margin
        : align === "end"
          ? window.innerWidth - margin
          : window.innerWidth - w - margin
    const clamped = Math.min(Math.max(place.left, min), Math.max(min, max))
    if (Math.abs(clamped - place.left) > 0.5) setPlace({ ...place, left: clamped })
  }, [place, align])

  const hide = () => setPlace(null)

  return (
    <span
      ref={anchor}
      className={cn("relative flex w-fit max-w-full min-w-0", className)}
      onPointerEnter={placeNow}
      onPointerLeave={hide}
      onFocus={placeNow}
      onBlur={hide}
    >
      {children}
      {place && text
        ? createPortal(
            <span
              ref={bubble}
              role="tooltip"
              className={cn(
                "pointer-events-none fixed z-[80] max-w-[min(260px,calc(100vw-20px))] rounded-[7px] border border-line bg-[rgb(26_27_33/0.98)] px-2.5 py-1 text-center text-[11.5px] font-medium leading-[1.3] text-txt2 shadow-[0_10px_26px_rgb(0_0_0/0.55)]",
                place.above ? "-translate-y-full" : "",
                align === "center" ? "-translate-x-1/2" : align === "end" ? "-translate-x-full" : "",
              )}
              style={{ top: place.top, left: place.left }}
            >
              {text}
            </span>,
            document.body,
          )
        : null}
    </span>
  )
}
