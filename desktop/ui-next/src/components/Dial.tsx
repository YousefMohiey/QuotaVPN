import { motion } from "motion/react"
import { Power } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"

export type DialState = "idle" | "connecting" | "on"

/** The one button that matters, kept to the owner's reference: a flat disc
    with one thin ring, the state told by colour alone (grey idle, green
    connected, brand while connecting), plus one dashed arc that turns only
    while a connection is actually being negotiated. */
export function Dial({
  state,
  onClick,
  disabled,
  size = 264,
  compact = false,
}: {
  state: DialState
  onClick: () => void
  disabled?: boolean
  size?: number
  compact?: boolean
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
      style={{ width: size, height: size }}
      className={cn(
        "relative grid shrink-0 place-items-center rounded-full border bg-[#13161a] shadow-[inset_0_1px_0_rgb(255_255_255/0.05),0_22px_48px_rgb(0_0_0/0.5),0_2px_6px_rgb(0_0_0/0.4)] transition-[background-color,border-color,color,box-shadow] duration-300 disabled:opacity-70",
        state === "on"
          ? "border-[var(--green-line)] bg-[#101713] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.14)] text-txt hover:border-[var(--brand-line)] hover:bg-[#151a20] [&_svg]:text-brand-strong/80 hover:[&_svg]:text-brand-strong",
      )}
    >
      {state === "connecting" && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite] motion-reduce:animate-none motion-reduce:opacity-60"
        />
      )}
      {/* the power over the word, sized to the circle it lives in */}
      <span className={cn("flex flex-col items-center", compact ? "gap-1.5" : "gap-3.5")}>
        <Power className={compact ? "size-6" : "size-10"} strokeWidth={1.6} aria-hidden />
        {compact ? null : <span className="font-display text-[15px] font-semibold tracking-[0.01em]">{label}</span>}
      </span>
    </motion.button>
  )
}
