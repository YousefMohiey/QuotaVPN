import { motion } from "motion/react"
import { Power } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"

export type DialState = "idle" | "connecting" | "on"

/** The one button that matters, kept to the owner's reference: a flat disc
    with one thin ring, the state told by colour alone (grey idle, green
    connected, brand while connecting). No glass, no glow, no arc. */
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
  const label = state === "connecting" ? t("working") : state === "on" ? t("disconnect") : t("connect")

  return (
    <motion.button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      whileTap={disabled ? undefined : { scale: 0.985 }}
      transition={{ type: "spring", stiffness: 460, damping: 32 }}
      className={cn(
        "relative grid size-[204px] shrink-0 place-items-center rounded-full border bg-[rgb(255_255_255/0.02)] transition-[background-color,border-color,color] duration-300 disabled:opacity-70",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.12)] text-txt hover:border-[var(--brand-line)]",
      )}
    >
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
