import { motion } from "motion/react"
import { Power } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"

export type DialState = "idle" | "connecting" | "on"

/** The one button that matters: a plain ring, state told by colour alone. */
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
        "relative grid size-[172px] place-items-center rounded-full border transition-colors duration-300 disabled:opacity-70",
        /* a hint of glass: translucent fill, one lit line, nothing else */
        "bg-white/[0.05] backdrop-blur-[14px] backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.10)]",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.14)] text-txt hover:border-[rgb(255_255_255/0.26)]",
      )}
    >
      {state === "connecting" && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite]"
        />
      )}
      <span className="flex flex-col items-center gap-2.5">
        <Power className="size-[38px]" strokeWidth={1.7} aria-hidden />
      </span>
    </motion.button>
  )
}
