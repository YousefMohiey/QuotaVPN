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
        "relative grid size-[172px] place-items-center rounded-full border transition-[background-color,border-color,box-shadow] duration-300 disabled:opacity-70",
        /* the glass: a light translucent pane, frosted by the scene behind
           it, with a lit top edge and a soft floor shadow */
        "bg-[rgb(255_255_255/0.055)] backdrop-blur-[16px] backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.16),inset_0_-18px_36px_rgb(0_0_0/0.22),0_18px_44px_rgb(0_0_0/0.35)]",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.16)] text-txt hover:border-[var(--brand-line)] hover:bg-[rgb(255_255_255/0.08)]",
      )}
    >
      <svg aria-hidden viewBox="0 0 100 100" className="pointer-events-none absolute -inset-[5px] size-[calc(100%+10px)]">
        <circle
          cx="50"
          cy="50"
          r="47"
          className={cn(
            "dial-arc",
            state === "on" ? "dial-arc-on" : state === "connecting" ? "dial-arc-connecting" : "",
          )}
        />
      </svg>
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
