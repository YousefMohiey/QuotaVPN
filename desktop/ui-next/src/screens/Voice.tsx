import { useState } from "react"
import { Info, Loader2, Mic, Rocket } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { ValorantMark } from "@/components/ValorantMark"
import { Toggle } from "@/components/Toggle"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { cn } from "@/lib/utils"

// The Valorant page, built like Home: one surface, a stage that answers the
// question at a glance, and a band underneath that holds the switches and the
// readings. No rail: this page has too little to say for a column, and a tall
// column of two paragraphs is what made it look empty.
// The helper's state and the game watcher live in the app state, so leaving
// this page never pauses the watching.

export function Voice() {
  const { t } = useI18n()
  const {
    card,
    voiceOn: on,
    voiceRunning: running,
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
      <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)_auto] overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        {/* the stage: one control, its state, and the two facts that name it */}
        <section className="flex min-h-0 flex-col items-center justify-center gap-4 px-6 py-5">
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
            <span className="flex flex-col items-center gap-2.5">
              {busy ? (
                <Loader2 className={cn("size-9", calm ? "" : "animate-spin")} aria-hidden />
              ) : (
                <Mic className="size-9" aria-hidden />
              )}
              <ValorantMark className={cn("size-6", on ? "text-[var(--green)]" : "text-txt3")} />
            </span>
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
            <span className="max-w-[380px] text-[12px] leading-snug text-txt2">{stateWord}</span>
            {msg ? (
              <span className="max-w-[400px] text-[11.5px] leading-snug text-txt3" role="status">{msg}</span>
            ) : null}
          </div>

          <div className="grid w-full max-w-[520px] shrink-0 grid-cols-3 overflow-hidden rounded-[10px] bg-white/[0.03]">
            <span className="flex min-w-0 flex-col items-center gap-1 px-2 py-3">
              <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{t("voiceRowTitle")}</span>
              <span className={cn("truncate text-[14px]", healthy ? "text-[var(--green)]" : "text-txt")}>
                {on ? t("wordOn") : t("wordOff")}
              </span>
            </span>
            <span className="flex min-w-0 flex-col items-center gap-1 px-2 py-3">
              <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{t("voiceHelperLbl")}</span>
              <span className="truncate text-[14px] text-txt">{running ? t("wordOn") : t("wordOff")}</span>
            </span>
            <span className="flex min-w-0 flex-col items-center gap-0.5 px-2 py-2.5">
              <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{t("profileLbl")}</span>
              <span className="truncate text-[14px] text-txt">
                {card?.card_type === "Streamerz" ? t("kindStreamerz") : t("kindGamerz")}
              </span>
            </span>
          </div>
        </section>

        {/* the band: the switch, what the helper is doing, and the fine print */}
        <div className="grid shrink-0 grid-cols-[1.15fr_1fr] border-t border-line bg-[var(--side)]">
          <div className="flex min-w-0 flex-col justify-center gap-2 px-5 py-4">
            <span className="flex min-w-0 items-center gap-1.5 text-[11px] font-medium uppercase tracking-[0.1em] text-txt2">
              <Rocket className="size-3.5 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
              <span className="truncate">{t("voiceLaunchTitle")}</span>
            </span>
            <div className="flex min-w-0 items-center gap-4">
              <span className="min-w-0 flex-1 text-[12px] leading-snug text-txt2">{t("voiceLaunchBody")}</span>
              <Toggle on={launch} busy={busy} onFlip={() => void flipLaunch()} label={t("voiceLaunchTitle")} />
            </div>
          </div>

          <div className="flex min-w-0 items-center gap-2.5 border-s border-line px-5 py-4">
            <Info className="size-3.5 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
            <p className="min-w-0 text-[12px] leading-relaxed text-txt3">{t("launchPanelBody")}</p>
          </div>
        </div>
      </div>
    </div>
  )
}
