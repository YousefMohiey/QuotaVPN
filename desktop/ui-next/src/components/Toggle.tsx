import { cn } from "@/lib/utils"

/** The one app toggle: 48x28 so it sits with 36px rows, and an off track
    with enough edge to read on plain dark, not just over artwork. */
export function Toggle({
  on,
  busy,
  disabled,
  onFlip,
  label,
}: {
  on: boolean
  busy: boolean
  disabled?: boolean
  onFlip: () => void
  label: string
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled || busy}
      onClick={() => void onFlip()}
      className={cn(
        "relative h-[28px] w-[48px] shrink-0 rounded-full border transition-colors duration-200 disabled:cursor-not-allowed disabled:opacity-60",
        on ? "border-transparent bg-[var(--brand-vivid)]" : "border-white/20 bg-white/[0.09]",
      )}
    >
      <span
        aria-hidden
        className={cn(
          "absolute top-1/2 size-[20px] -translate-y-1/2 rounded-full bg-white transition-all duration-200",
          on ? "left-[24px]" : "left-[3px]",
        )}
      />
    </button>
  )
}
