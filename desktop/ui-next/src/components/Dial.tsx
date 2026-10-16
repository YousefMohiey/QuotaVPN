import { motion } from "motion/react"
import { Power } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"

export type DialState = "idle" | "connecting" | "on"

/** The one button that matters: a solid key with one lit edge, state told
    by the ring and the word, no glass and no glow. */
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
        "relative grid size-[204px] shrink-0 place-items-center rounded-full border transition-colors duration-300 disabled:opacity-70",
        /* solid surface with one lit edge and one shaded edge: a real key,
           depth without glass */
        "bg-[linear-gradient(180deg,#20242c,#14161c)] shadow-[0_20px_44px_rgb(0_0_0/0.5),0_2px_8px_rgb(0_0_0/0.4),inset_0_1px_0_rgb(255_255_255/0.10),inset_0_-2px_6px_rgb(0_0_0/0.35)]",
        state === "on"
          ? "border-[rgb(92_178_142/0.65)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.14)] text-txt hover:border-[rgb(255_255_255/0.28)]",
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
