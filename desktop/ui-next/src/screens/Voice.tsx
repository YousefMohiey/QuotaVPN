import { useEffect, useRef, useState } from "react"
import { ValorantMark } from "@/components/ValorantMark"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { api } from "@/lib/ipc"
import { cn } from "@/lib/utils"

// The Valorant page: the mark and two quiet rows over the Omen artwork.
// The artwork sits behind the UI and never takes part in layout.
function Toggle({
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
        "relative h-[34px] w-[60px] shrink-0 rounded-full border transition-colors duration-200 disabled:cursor-not-allowed disabled:opacity-60",
        on ? "border-transparent bg-[var(--brand)]" : "border-line bg-white/[0.06]",
      )}
    >
      <span
        aria-hidden
        className={cn(
          "absolute top-1/2 size-[26px] -translate-y-1/2 rounded-full bg-white transition-all duration-200",
          on ? "left-[30px]" : "left-[3px]",
        )}
      />
    </button>
  )
}

export function Voice() {
  const { t } = useI18n()
  const { card, appsMode, apps, transport } = useApp()
  const [running, setRunning] = useState(false)
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

  const setHelper = async (next: boolean) => {
    if (busy) return
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
      } else {
        if (!card) {
          setMsg(t("needCard"))
          return
        }
        // A running session keeps its configuration and gains the voice
        // rules; with nothing running this starts a voice-only session.
        const merged = running
        const r = await api.start(card.uuid, merged ? appsMode : "allow", merged ? apps : [], transport, true)
        if (r.ok) {
          localStorage.setItem("qc-voice-active", "1")
          localStorage.setItem("qc-voice-merged", merged ? "1" : "0")
          setRunning(true)
        } else {
          // A cold start can report failure a moment before the engine is
          // actually routing: check once more and adopt it when it came up.
          await new Promise((res) => window.setTimeout(res, 2500))
          const st = await api.status().catch(() => null)
          if (st?.running) {
            localStorage.setItem("qc-voice-active", "1")
            localStorage.setItem("qc-voice-merged", merged ? "1" : "0")
            setRunning(true)
          } else {
            setMsg(r.msg)
          }
        }
      }
    } catch (e) {
      setMsg(typeof e === "string" ? e : String(e))
    } finally {
      setBusy(false)
    }
  }

  // "Launch with Valorant": with the flag on, the helper enables itself on
  // app start once a card is available. There is no engine process watch, so
  // this arms at launch rather than at the Valorant process.
  const triedLaunch = useRef(false)
  useEffect(() => {
    if (triedLaunch.current) return
    triedLaunch.current = true
    try {
      if (localStorage.getItem("qc-voice-launch") !== "1") return
    } catch {
      return
    }
    if (!card || on) return
    void setHelper(true)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [card])

  const flipLaunch = async () => {
    const next = !launch
    setLaunch(next)
    try {
      localStorage.setItem("qc-voice-launch", next ? "1" : "0")
    } catch {
      /* private mode */
    }
    if (next && !on) await setHelper(true)
  }

  return (
    <div className="relative">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-y-0 right-0 w-[46%] min-w-[340px] select-none [mask-image:linear-gradient(to_right,transparent_0,black_42%)]"
      >
        <img
          src="/omen.jpg"
          alt=""
          className="h-full w-full object-cover object-right opacity-70"
        />
      </div>
      <div className="relative">
        <div className="flex items-center gap-5">
          <ValorantMark className="size-[76px] shrink-0 text-white" />
          <div className="min-w-0">
            <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("voiceTitle")}</h1>
            <p className="mt-1 text-[15px] text-txt2">{t("voiceTagline")}</p>
            <p className="mt-0.5 text-[15px] font-bold text-txt">{t("voicePingBold")}</p>
          </div>
        </div>

        <div className="mt-8 border-t border-line py-6">
          <div className="flex items-start justify-between gap-6">
            <div className="min-w-0">
              <h2 className="text-[17px] font-semibold text-txt">{t("voiceRowTitle")}</h2>
              <p className="mt-1 max-w-[560px] text-[14px] leading-relaxed text-txt2">{t("voiceRowBody")}</p>
              {msg && <p className="mt-2 text-[12.5px] text-txt2">{msg}</p>}
            </div>
            <div className="flex shrink-0 flex-col items-end gap-2">
              <Toggle on={on} busy={busy} disabled={!card} onFlip={() => void setHelper(!on)} label={t("voiceRowTitle")} />
              {on && (
                <span className="inline-flex items-center gap-1.5 text-[13px] font-medium text-[var(--green)]">
                  <span className="size-1.5 rounded-full bg-[var(--green)]" aria-hidden />
                  {t("voiceActive")}
                </span>
              )}
            </div>
          </div>
        </div>

        <div className="border-t border-line py-6">
          <div className="flex items-start justify-between gap-6">
            <div className="min-w-0">
              <h2 className="text-[17px] font-semibold text-txt">{t("voiceLaunchTitle")}</h2>
              <p className="mt-1 max-w-[560px] text-[14px] leading-relaxed text-txt2">{t("voiceLaunchBody")}</p>
            </div>
            <div className="shrink-0">
              <Toggle on={launch} busy={busy} onFlip={() => void flipLaunch()} label={t("voiceLaunchTitle")} />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
