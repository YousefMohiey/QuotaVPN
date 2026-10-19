import { useState } from "react"
import { Loader2, Mic } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { ValorantMark } from "@/components/ValorantMark"
import { Toggle } from "@/components/Toggle"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { cn } from "@/lib/utils"

// The Valorant page: one surface, laid out like a targeting field. The theme is
// carried by geometry, not by costume: two rotated planes cut the field, corner
// ticks frame it, and the control sits inside a diamond reticle that takes the
// state colour. No glow, no red, no game chrome.
// The helper's state and the game watcher live in the app state, so leaving
// this page never pauses the watching.

export function Voice() {
  const { t } = useI18n()
  const {
    card,
    voiceOn: on,
    voiceBusy: busy,
    voiceHealth: health,
    voiceMsg: msg,
    setVoiceHelper: setHelper,
  } = useApp()
  const calm = useReducedMotion()
  const [launch, setLaunch] = useState(() => {
    try {
      // Missing flag means on: only an explicit off switch stands the
      // auto-connect down, and the Rust-side watcher reads it the same way.
      return localStorage.getItem("qc-voice-launch") !== "0"
    } catch {
      return true
    }
  })

  const flipLaunch = async () => {
    // The flag never touches the helper itself: only the game appearing
    // arms it, and only the game closing stands it down.
    const next = !launch
    setLaunch(next)
    try {
      localStorage.setItem("qc-voice-launch", next ? "1" : "0")
      if (!next) localStorage.removeItem("qc-voice-auto")
    } catch {
      /* private mode */
    }
  }

  const healthy = on && health === "ok"
  const stateWord = busy
    ? on
      ? t("voiceDisconnecting")
      : t("voiceConnecting")
    : on
      ? health === "ok"
        ? t("voiceConnected")
        : health === "bad"
          ? t("voiceUnhealthy")
          : t("voiceChecking")
      : t("voiceOffSub")

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col">
      <section className="relative flex min-h-0 flex-1 flex-col items-center justify-center gap-5 overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-6 py-6">
        {/* the field: two rotated planes, and ticks on the corners */}
        <span
          aria-hidden
          className="pointer-events-none absolute left-1/2 top-1/2 size-[520px] -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-[48px] border border-white/[0.03]"
        />
        <span
          aria-hidden
          className="pointer-events-none absolute left-1/2 top-1/2 size-[400px] -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-[34px] border border-white/[0.045]"
        />
        <span aria-hidden className="pointer-events-none absolute left-5 top-5 size-4 border-l border-t border-[rgb(255_255_255/0.14)]" />
        <span aria-hidden className="pointer-events-none absolute right-5 top-5 size-4 border-r border-t border-[rgb(255_255_255/0.14)]" />
        <span aria-hidden className="pointer-events-none absolute bottom-5 left-5 size-4 border-b border-l border-[rgb(255_255_255/0.14)]" />
        <span aria-hidden className="pointer-events-none absolute bottom-5 right-5 size-4 border-b border-r border-[rgb(255_255_255/0.14)]" />

        <div className="relative flex items-center gap-2.5">
          <ValorantMark className={cn("size-[18px] shrink-0", healthy ? "text-[var(--green)]" : on ? "text-brand-strong" : "text-txt2")} />
          <span className="text-[12px] font-medium uppercase tracking-[0.22em] text-txt2">{t("voiceTitle")}</span>
          <span aria-hidden className={cn("size-1.5 rotate-45", healthy ? "bg-[var(--green)]" : on ? "bg-brand-strong" : "bg-txt3")} />
        </div>
        <p className="relative -mt-2.5 max-w-[440px] text-center text-[12px] leading-snug text-txt3">{t("voiceTagline")}</p>

        {/* the control sits inside a diamond that carries the state */}
        <div className="relative mt-1 grid size-[272px] shrink-0 place-items-center">
          <span
            aria-hidden
            className={cn(
              "absolute inset-0 rotate-45 rounded-[30px] border transition-colors duration-300",
              on ? "border-[var(--green-line)]" : "border-[var(--brand-line)]",
            )}
          />
          <span
            aria-hidden
            className={cn(
              "absolute inset-[26px] rotate-45 rounded-[22px] border transition-colors duration-300",
              on ? "border-[var(--green-line)]/60" : "border-white/[0.06]",
            )}
          />
          <button
            type="button"
            onClick={() => void setHelper(!on)}
            disabled={!card || busy}
            aria-pressed={on}
            aria-label={t("voiceRowTitle")}
            className={cn(
              "press relative grid size-[196px] shrink-0 place-items-center rounded-full border bg-[var(--panel)] transition-colors duration-200 disabled:cursor-wait",
              on
                ? "border-[var(--green-line)] text-[var(--green)]"
                : "border-[rgb(255_255_255/0.16)] text-brand-strong/85 hover:border-[var(--brand-line)] hover:text-brand-strong",
            )}
          >
            {busy ? (
              <Loader2 className={cn("size-10", calm ? "" : "animate-spin")} aria-hidden />
            ) : (
              <Mic className="size-10" aria-hidden />
            )}
          </button>
        </div>

        <div className="relative flex min-w-0 flex-col items-center gap-1 text-center">
          <motion.span
            key={on ? (healthy ? "healthy" : "armed") : "off"}
            initial={calm ? { opacity: 0 } : { opacity: 0, y: 2 }}
            animate={calm ? { opacity: 1 } : { opacity: 1, y: 0 }}
            transition={{ duration: calm ? 0.12 : 0.18, ease: [0.23, 1, 0.32, 1] }}
            className={cn(
              "truncate font-display text-[26px] font-semibold leading-tight tracking-[-0.02em]",
              healthy ? "text-[var(--green)]" : "text-txt",
            )}
          >
            {on ? t("voiceActive") : t("voiceOffTitle")}
          </motion.span>
          <span className="max-w-[420px] text-[12.5px] leading-snug text-txt2">{stateWord}</span>
          {msg ? (
            <span className="max-w-[440px] text-[12px] leading-snug text-txt3" role="status">{msg}</span>
          ) : null}
        </div>

        <div className="relative mt-1 flex items-center gap-3">
          <Toggle on={launch} busy={busy} onFlip={() => void flipLaunch()} label={t("voiceLaunchTitle")} />
          <span className="text-[13px] text-txt2">{t("voiceLaunchTitle")}</span>
        </div>

        <p className="relative max-w-[460px] text-center text-[12px] leading-relaxed text-txt3">{t("launchPanelBody")}</p>
      </section>
    </div>
  )
}
