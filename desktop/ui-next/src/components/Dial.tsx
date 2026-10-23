import { AnimatePresence, motion, useReducedMotion } from "motion/react"
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
  const calm = useReducedMotion()
  const label = state === "connecting" ? t("working") : state === "on" ? t("disconnect") : t("connect")

  return (
    <div className="relative grid shrink-0 place-items-center" style={{ width: size, height: size }}>
      {/* two rings leave the rim when a session comes up, one behind the other */}
      <AnimatePresence>
        {state === "on" && !calm &&
          [0, 1].map((i) => (
            <motion.span
              key={"rim" + i}
              aria-hidden
              className="pointer-events-none absolute inset-0 rounded-full border border-[var(--green-line)]"
              initial={{ opacity: 0.8, scale: 0.93 }}
              animate={{ opacity: 0, scale: 1.28 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.9, delay: i * 0.14, ease: [0.22, 1, 0.36, 1] }}
            />
          ))}
      </AnimatePresence>

      <motion.button
        type="button"
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
        // The press answer is feedback, not decoration: it stays even when
        // Windows asks for reduced motion, where the looping rings below drop.
        whileTap={disabled ? undefined : { scale: 0.93 }}
        animate={state === "on" && !calm ? { scale: [1, 1.045, 1] } : { scale: 1 }}
        transition={{ type: "spring", stiffness: 460, damping: 26, mass: 0.7 }}
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
      <motion.span
        aria-hidden
        className="pointer-events-none absolute inset-0 rounded-full"
        initial={false}
        animate={{
          boxShadow: state === "on" ? "inset 0 0 48px rgb(40 160 110 / 0.16)" : "inset 0 0 0 rgb(0 0 0 / 0)",
        }}
        transition={{ duration: 0.5 }}
      />

      {state === "connecting" && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite] motion-reduce:animate-none motion-reduce:opacity-60"
        />
      )}
      {/* the power over the word, sized to the circle it lives in */}
      <span className={cn("relative flex flex-col items-center", compact ? "gap-1.5" : "gap-3.5")}>
        <Power className={compact ? "size-6" : "size-10"} strokeWidth={1.6} aria-hidden />
        {compact ? null : <span className="font-display text-[15px] font-semibold tracking-[0.01em]">{label}</span>}
      </span>
      </motion.button>
    </div>
  )
}
