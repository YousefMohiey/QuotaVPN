import { useState } from "react"
import { Loader2, Mic } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { ValorantMark } from "@/components/ValorantMark"
import { Toggle } from "@/components/Toggle"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { cn } from "@/lib/utils"

// The Valorant page, built like Home: one surface, one control, the state under
// it, the readings below. The mark names the page, the mic is the control; they
// never sit inside each other. The helper's state and the game watcher live in
// the app state, so leaving this page never pauses the watching.

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
      <section className="flex min-h-0 flex-1 flex-col items-center justify-center gap-5 overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-6 py-6">
        <div className="flex items-center gap-2.5">
          <ValorantMark className="size-[26px] shrink-0 text-txt" />
          <span className="text-[12px] font-medium uppercase tracking-[0.14em] text-txt2">{t("voiceTitle")}</span>
        </div>

        <button
          type="button"
          onClick={() => void setHelper(!on)}
          disabled={!card || busy}
          aria-pressed={on}
          aria-label={t("voiceRowTitle")}
          className={cn(
            "press grid size-[220px] shrink-0 place-items-center rounded-full border transition-colors duration-200 disabled:cursor-wait",
            on
              ? "border-[var(--green)] text-[var(--green)]"
              : "border-[rgb(255_255_255/0.14)] text-brand-strong/80 hover:border-[var(--brand-line)] hover:bg-[#151a20] hover:text-brand-strong",
          )}
        >
          {busy ? (
            <Loader2 className={cn("size-10", calm ? "" : "animate-spin")} aria-hidden />
          ) : (
            <Mic className="size-10" aria-hidden />
          )}
        </button>

        <div className="flex min-w-0 flex-col items-center gap-1 text-center">
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
          <span className="max-w-[400px] text-[12.5px] leading-snug text-txt2">{stateWord}</span>
          {msg ? (
            <span className="max-w-[420px] text-[12px] leading-snug text-txt3" role="status">{msg}</span>
          ) : null}
        </div>

        <div className="flex items-center gap-3">
          <span className="text-[13px] text-txt2">{t("voiceLaunchTitle")}</span>
          <Toggle on={launch} busy={busy} onFlip={() => void flipLaunch()} label={t("voiceLaunchTitle")} />
        </div>

        <p className="max-w-[440px] text-center text-[12px] leading-relaxed text-txt3">{t("launchPanelBody")}</p>
      </section>
    </div>
  )
}
