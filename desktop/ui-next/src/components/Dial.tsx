import { useEffect, useState } from "react"
import { motion } from "motion/react"
import { Power } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"

export type DialState = "idle" | "connecting" | "on"

/** The accent tokens, read off the live theme: WebView2 refuses to paint an
    SVG stroke that comes from a CSS variable (learned on the graph), so the
    arc gets its colour as a plain attribute instead. */
function useArcColors() {
  const [c, setC] = useState({ vivid: "#2e7bf6", strong: "#5b8def", green: "#5cb28e" })
  useEffect(() => {
    const cs = getComputedStyle(document.documentElement)
    const vivid = cs.getPropertyValue("--brand-vivid").trim()
    const strong = cs.getPropertyValue("--brand-strong").trim()
    const green = cs.getPropertyValue("--green").trim()
    setC((prev) => ({
      vivid: vivid || prev.vivid,
      strong: strong || prev.strong,
      green: green || prev.green,
    }))
  }, [])
  return c
}

/** The one button that matters: a glass disc with a lit edge and a soft
    float, the accent arc ringing it just outside, state told by colour. */
export function Dial({
  state,
  onClick,
  disabled,
}: {
  state: DialState
  onClick: () => void
  disabled?: boolean
}) {
  const { t } = useI18n()
  const colors = useArcColors()
  const label = state === "connecting" ? t("working") : state === "on" ? t("disconnect") : t("connect")
  const arcColor = state === "on" ? colors.green : state === "connecting" ? colors.strong : colors.vivid

  return (
    <motion.button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      whileTap={disabled ? undefined : { scale: 0.985 }}
      transition={{ type: "spring", stiffness: 460, damping: 32 }}
      className={cn(
        "relative grid size-[204px] shrink-0 place-items-center rounded-full border transition-[background-color,border-color,box-shadow] duration-300 disabled:opacity-70",
        /* the glass: a light translucent pane with a soft sheen at the top,
           a lit edge, inner thickness and a real float underneath */
        "bg-[rgb(255_255_255/0.07)] backdrop-blur-[16px] backdrop-saturate-150",
        "bg-[radial-gradient(120%_120%_at_50%_0%,rgb(255_255_255/0.13),transparent_55%)]",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.25),inset_0_-22px_44px_rgb(0_0_0/0.28),0_22px_50px_rgb(0_0_0/0.45)]",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.16)] text-txt hover:border-[var(--brand-line)] hover:bg-[rgb(255_255_255/0.08)]",
      )}
    >
      <svg
        aria-hidden
        viewBox="0 0 100 100"
        className="pointer-events-none absolute -inset-[6px] size-[calc(100%+12px)]"
      >
        <circle
          cx="50"
          cy="50"
          r="47"
          stroke={arcColor}
          className={cn("dial-arc", state === "connecting" && "dial-arc-connecting")}
        />
      </svg>
      {state === "connecting" && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite]"
        />
      )}
      {/* the power over the word, sized to the circle it lives in */}
      <span className="flex flex-col items-center gap-3">
        <Power className="size-9" strokeWidth={1.75} aria-hidden />
        <span className="text-[13.5px] font-medium tracking-[0.01em]">{label}</span>
      </span>
    </motion.button>
  )
}
