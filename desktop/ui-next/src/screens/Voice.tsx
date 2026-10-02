import { useEffect, useRef, useState } from "react"
import { Check, ChevronRight, Loader2, Mic } from "lucide-react"
import { ValorantMark } from "@/components/ValorantMark"
import { Toggle } from "@/components/Toggle"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { api } from "@/lib/ipc"
import { cn } from "@/lib/utils"

// The Valorant page: header, then a Voice Chat card whose right column is the
// live connection status (and the switch), then a Launch card with the same
// two-column rhythm. The cards are translucent so the artwork stays visible
// through them. The artwork itself never takes part in layout.
// Last engine truth, shared across mounts: switching pages unmounts this
// view, and reopening on a blank false is the off/on blink.
let lastVoiceRunning: boolean | null = null

export function Voice() {
  const { t } = useI18n()
  const { card, appsMode, apps, transport } = useApp()
  const [running, setRunningState] = useState<boolean>(() => lastVoiceRunning ?? false)
  const setRunning = (v: boolean) => {
    lastVoiceRunning = v
    setRunningState(v)
  }
  const [busy, setBusy] = useState(false)
  const [msg, setMsg] = useState("")
  const [launch, setLaunch] = useState(() => {
    try {
      return localStorage.getItem("qc-voice-launch") === "1"
    } catch {
      return false
    }
  })

  useEffect(() => {
    let alive = true
    const poll = async () => {
      try {
        const st = await api.status()
        if (alive) setRunning(st.running)
      } catch {
        /* engine not reachable yet */
      }
    }
    void poll()
    const id = window.setInterval(() => void poll(), 2000)
    return () => {
      alive = false
      window.clearInterval(id)
    }
  }, [])

  const on = running && localStorage.getItem("qc-voice-active") === "1"

  const setHelper = async (next: boolean): Promise<boolean> => {
    if (busy) return false
    setBusy(true)
    setMsg("")
    try {
      if (!next) {
        // Turning the helper off must not disturb the rest of the session:
        // a merged session restarts with the same setup minus the voice
        // rules, a voice-only session just stops.
        const merged = localStorage.getItem("qc-voice-merged") === "1"
        if (merged && card) {
          const r = await api.start(card.uuid, appsMode, apps, transport, false)
          if (!r.ok) setMsg(r.msg)
        } else {
          const r = await api.stop()
          setRunning(false)
          if (!r.ok) setMsg(r.msg)
        }
        localStorage.removeItem("qc-voice-active")
        localStorage.removeItem("qc-voice-merged")
        return true
      } else {
        if (!card) {
          setMsg(t("needCard"))
          return false
        }
        // A running session keeps its configuration and gains the voice
        // rules; with nothing running this starts a voice-only session.
        const merged = running
        const r = await api.start(card.uuid, merged ? appsMode : "allow", merged ? apps : [], transport, true)
        if (r.ok) {
          localStorage.setItem("qc-voice-active", "1")
          localStorage.setItem("qc-voice-merged", merged ? "1" : "0")
          setRunning(true)
          return true
        } else {
          // A cold start can report failure a moment before the engine is
          // actually routing: check once more and adopt it when it came up.
          await new Promise((res) => window.setTimeout(res, 2500))
          const st = await api.status().catch(() => null)
          if (st?.running) {
            localStorage.setItem("qc-voice-active", "1")
            localStorage.setItem("qc-voice-merged", merged ? "1" : "0")
            setRunning(true)
            return true
          } else {
            setMsg(r.msg)
            return false
          }
        }
      }
    } catch (e) {
      setMsg(typeof e === "string" ? e : String(e))
      return false
    } finally {
      setBusy(false)
    }
  }

  // "Launch with Valorant": the flag only watches. The game itself drives
  // the helper: appearing arms it, closing disarms it when the watcher armed
  // it (a helper you switched on yourself is left alone). State flows
  // through refs so one interval covers every render.
  const watchRef = useRef({ card, on, busy })
  watchRef.current = { card, on, busy }
  const setHelperRef = useRef(setHelper)
  setHelperRef.current = setHelper
  const gameWasUp = useRef(false)
  useEffect(() => {
    const get = (k: string) => {
      try {
        return localStorage.getItem(k)
      } catch {
        return null
      }
    }
    let alive = true
    const tick = async () => {
      try {
        if (localStorage.getItem("qc-voice-launch") !== "1") {
          gameWasUp.current = false
          return
        }
      } catch {
        return
      }
      let game = false
      try {
        for (const exe of ["VALORANT-Win64-Shipping.exe"]) {
          if (await api.processRunning(exe)) {
            game = true
            break
          }
        }
      } catch {
        return
      }
      if (!alive) return
      const snapshot = watchRef.current
      if (game && !gameWasUp.current) {
        // The game just appeared: arm the helper unless it is already on.
        gameWasUp.current = true
        if (!snapshot.card || snapshot.on || snapshot.busy) return
        const armed = await setHelperRef.current(true)
        if (!armed) return
        try {
          localStorage.setItem("qc-voice-auto", "1")
        } catch {
          /* private mode */
        }
      } else if (!game && gameWasUp.current) {
        // The game just closed: stand the watcher-armed helper down. A
        // merged session keeps its setup minus the voice rules, a
        // voice-only session just stops.
        gameWasUp.current = false
        if (get("qc-voice-auto") !== "1" || !snapshot.on || snapshot.busy) return
        await setHelperRef.current(false)
        try {
          localStorage.removeItem("qc-voice-auto")
        } catch {
          /* private mode */
        }
      }
    }
    void tick()
    const id = window.setInterval(() => void tick(), 5000)
    return () => {
      alive = false
      window.clearInterval(id)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

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
    <div className="relative">
      <div className="relative z-10">
        <div className="flex items-center gap-5">
          <ValorantMark className="size-[72px] shrink-0 text-white" />
          <div className="min-w-0">
            <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("voiceTitle")}</h1>
            <p className="mt-1 text-[15px] text-txt2">{t("voiceTagline")}</p>
            <p className="mt-0.5 text-[15px] font-bold text-txt">{t("voicePingBold")}</p>
          </div>
        </div>

        <div className="mt-7 flex flex-col gap-6">
          <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
            <div className="flex flex-col gap-5 lg:flex-row lg:items-stretch">
              <div className="flex min-w-0 flex-1 items-center">
                <div className="min-w-0">
                  <h2 className="text-[16.5px] font-semibold text-txt">{t("voiceRowTitle")}</h2>
                  <p className="mt-1 max-w-[430px] text-[13.5px] leading-relaxed text-txt2">
                    {t("voiceRowBody")}
                  </p>
                  {msg && <p className="mt-2 text-[12.5px] text-txt2">{msg}</p>}
                </div>
              </div>

              <div className="flex shrink-0 lg:w-[436px] lg:border-l lg:border-line lg:pl-6">
                {/* The status panel doubles as the switch: live connection
                    state on top, the readiness note under it. */}
                <button
                  type="button"
                  role="switch"
                  aria-checked={on}
                  aria-label={t("voiceRowTitle")}
                  disabled={!card}
                  onClick={() => void setHelper(!on)}
                  className={cn(
                    "flex w-full flex-col justify-center rounded-[12px] border border-line bg-white/[0.035] p-3.5 text-left transition-colors",
                    !card ? "cursor-not-allowed opacity-60" : "hover:bg-white/[0.05]",
                  )}
                >
                  <div className="flex items-center gap-3.5">
                    <span
                      className={cn(
                        "grid size-14 shrink-0 place-items-center rounded-full border-[1.5px]",
                        on ? "border-[var(--green)] text-[var(--green)]" : "border-line text-txt3",
                      )}
                    >
                      {busy ? (
                        <Loader2 className="size-5 animate-spin" aria-hidden />
                      ) : (
                        <Mic className="size-5" aria-hidden />
                      )}
                    </span>
                    <div className="min-w-0 flex-1">
                      <h3 className="text-[15.5px] font-semibold text-txt">
                        {on ? t("voiceActive") : t("voiceOffTitle")}
                      </h3>
                      <p className="mt-0.5 flex items-center gap-1.5 truncate text-[12.5px] text-txt2">
                        <span
                          className={cn("size-1.5 shrink-0 rounded-full", on ? "bg-[var(--green)]" : "bg-txt3")}
                          aria-hidden
                        />
                        {busy
                          ? on
                            ? t("voiceDisconnecting")
                            : t("voiceConnecting")
                          : on
                            ? t("voiceConnected")
                            : t("voiceOffSub")}
                      </p>
                    </div>
                    <ChevronRight className="size-5 shrink-0 text-txt3" aria-hidden />
                  </div>
                  {on && (
                    <div className="mt-3 flex items-center gap-3 rounded-[10px] border border-[var(--green-line)] bg-[var(--green-bg)] p-3">
                      <span className="grid size-6 shrink-0 place-items-center rounded-full bg-[var(--green)] text-white">
                        <Check className="size-3.5" strokeWidth={3} aria-hidden />
                      </span>
                      <div className="min-w-0">
                        <p className="text-[13px] font-semibold text-txt">{t("voiceReadyTitle")}</p>
                        <p className="mt-0.5 text-[12.5px] text-txt2">{t("voiceReadyBody")}</p>
                      </div>
                    </div>
                  )}
                </button>
              </div>
            </div>
          </section>

          <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
            <div className="flex flex-col gap-5 lg:flex-row lg:items-stretch">
              <div className="flex min-w-0 flex-1 items-center">
                <div className="min-w-0">
                  <h2 className="text-[16.5px] font-semibold text-txt">{t("voiceLaunchTitle")}</h2>
                  <p className="mt-1 max-w-[430px] text-[13.5px] leading-relaxed text-txt2">
                    {t("voiceLaunchBody")}
                  </p>
                </div>
              </div>

              <div className="flex shrink-0 lg:w-[436px] lg:border-l lg:border-line lg:pl-6">
                <div className="flex w-full items-center gap-4 rounded-[12px] border border-line bg-white/[0.035] p-3.5">
                  <div className="min-w-0 flex-1">
                    <h3 className="text-[13.5px] font-semibold text-txt">{t("voiceLaunchTitle")}</h3>
                    <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("launchPanelBody")}</p>
                  </div>
                  <Toggle on={launch} busy={busy} onFlip={() => void flipLaunch()} label={t("voiceLaunchTitle")} />
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  )
}
