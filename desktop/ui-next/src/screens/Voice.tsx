import { useState } from "react"
import { Check, Info, Loader2, Mic, Rocket } from "lucide-react"
import { ValorantMark } from "@/components/ValorantMark"
import { Toggle } from "@/components/Toggle"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { cn } from "@/lib/utils"

// The Valorant page: header, then a Voice Chat card whose right column is the
// live connection status (and the switch), then a Launch card with the same
// two-column rhythm. The cards are translucent so the artwork stays visible
// through them. The artwork itself never takes part in layout.
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

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1120px] flex-col gap-3">
      <div className="flex items-center gap-4">
        <ValorantMark className="size-[44px] shrink-0 text-white" />
        <div className="min-w-0">
          <h1 className="font-display text-[22px] font-semibold leading-tight tracking-[-0.02em] text-txt">{t("voiceTitle")}</h1>
          <p className="mt-0.5 text-[12.5px] text-txt2">{t("voiceTagline")}</p>
          <p className="mt-0.5 text-[12px] font-medium text-txt2">{t("voicePingBold")}</p>
        </div>
      </div>

      <section className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1.12fr)_minmax(0,1fr)]">
        {/* left: the two controls, hairline rows in one panel */}
        <div className="flex flex-col divide-y divide-line overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
          <div className="flex min-h-0 flex-1 items-center gap-4 px-5 py-4">
            <Mic className="size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
            <div className="min-w-0 flex-1">
              <h2 className="text-[12.5px] font-medium text-txt">{t("voiceRowTitle")}</h2>
              <p className="mt-0.5 max-w-[540px] text-[12px] leading-relaxed text-txt2">{t("voiceRowBody")}</p>
            </div>
            <div className="shrink-0">
              <Toggle on={on} busy={busy} disabled={!card} onFlip={() => void setHelper(!on)} label={t("voiceRowTitle")} />
            </div>
          </div>

          <div className="flex min-h-0 flex-1 items-center gap-4 px-5 py-4">
            <Rocket className="size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
            <div className="min-w-0 flex-1">
              <h2 className="text-[12.5px] font-medium text-txt">{t("voiceLaunchTitle")}</h2>
              <p className="mt-0.5 max-w-[540px] text-[12px] leading-relaxed text-txt2">{t("voiceLaunchBody")}</p>
            </div>
            <div className="shrink-0">
              <Toggle on={launch} busy={busy} onFlip={() => void flipLaunch()} label={t("voiceLaunchTitle")} />
            </div>
          </div>

          <div className="mt-auto flex items-start gap-4 border-t border-line px-5 py-3.5">
            <Info className="size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
            <p className="max-w-[560px] text-[12px] leading-relaxed text-txt3">{t("launchPanelBody")}</p>
          </div>
        </div>

        {/* right: the live state at scale */}
        <div className="flex flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
          <div className="flex items-center gap-2.5 border-b border-line px-5 py-3">
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                on ? (health === "ok" ? "bg-[var(--green)]" : health === "bad" ? "bg-[var(--amber)]" : "bg-txt3") : "bg-txt3",
              )}
              aria-hidden
            />
            <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-txt">{t("voiceRowTitle")}</span>
            <span className="max-w-[45%] shrink-0 truncate text-end text-[12px] text-txt3">
              {busy
                ? on
                  ? t("voiceDisconnecting")
                  : t("voiceConnecting")
                : on
                  ? health === "ok"
                    ? t("voiceConnected")
                    : health === "bad"
                      ? t("voiceUnhealthy")
                      : t("voiceChecking")
                  : t("voiceOffSub")}
            </span>
          </div>

          <div className="flex flex-1 flex-col items-center justify-center gap-3 px-5 py-8 text-center">
            <span
              className={cn(
                "grid size-20 shrink-0 place-items-center rounded-full border-[1.5px]",
                on ? "border-[var(--green)] text-[var(--green)]" : "border-line text-txt3",
              )}
            >
              {busy ? <Loader2 className="size-5 animate-spin" aria-hidden /> : <Mic className="size-5" aria-hidden />}
            </span>
            <p className="font-display text-[16px] font-semibold text-txt">{on ? t("voiceActive") : t("voiceOffTitle")}</p>
            {msg && <p className="max-w-[320px] text-[12px] leading-relaxed text-txt3">{msg}</p>}
          </div>


          {on && health === "ok" && (
            <div className="mt-auto m-3 flex items-center gap-3 rounded-[10px] border border-[var(--green-line)] bg-[var(--green-bg)] p-3">
              <span className="grid size-6 shrink-0 place-items-center rounded-full bg-[var(--green)] text-white">
                <Check className="size-3.5" strokeWidth={3} aria-hidden />
              </span>
              <div className="min-w-0 text-start">
                <p className="text-[12px] font-medium text-txt">{t("voiceReadyTitle")}</p>
                <p className="mt-0.5 text-[12px] text-txt2">{t("voiceReadyBody")}</p>
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  )
}
