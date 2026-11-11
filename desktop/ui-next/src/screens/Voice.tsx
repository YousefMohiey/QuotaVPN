import { useEffect, useRef, useState } from "react"
import { Mic } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { ValorantMark } from "@/components/ValorantMark"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { cn } from "@/lib/utils"

// The Valorant page, laid out the way the game lays out a HUD: nothing centred,
// a wordmark pinned to the top left, the control on the right, the state in a
// block of its own, and the switches on the floor. Chamfered plates, skewed
// rules and edge ticks carry the character; the palette stays graphite and blue.

const PLATE =
  "polygon(16% 0, 84% 0, 100% 16%, 100% 84%, 84% 100%, 16% 100%, 0 84%, 0 16%)"
const PANEL = "polygon(28px 0, 100% 0, 100% calc(100% - 28px), calc(100% - 28px) 100%, 0 100%, 0 28px)"
const CHIP = "polygon(14px 0, 100% 0, 100% calc(100% - 14px), calc(100% - 14px) 100%, 0 100%, 0 14px)"
const SWITCH = "polygon(9px 0, 100% 0, 100% calc(100% - 9px), calc(100% - 9px) 100%, 0 100%, 0 9px)"
const KNOB = "polygon(5px 0, 100% 0, 100% calc(100% - 5px), calc(100% - 5px) 100%, 0 100%, 0 5px)"

export function Voice() {
  const { t } = useI18n()
  const {
    card,
    voiceOn: on,
    voiceBusy: busy,
    voiceHealth: health,
    voiceMsg: msg,
    setVoiceHelper: setHelper,
    cancelVoice,
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

  /**
   * Like the Home dial, the plate reads one state that flips on the press, and
   * only a start shows progress: closing is a wind-down the user should not
   * have to watch.
   */
  const wanted = useRef(on)
  // The press is the state, at once. The engine run that follows is slower than
  // a finger, and reading a state that only changed when the run reported back
  // made every press feel dead for a second.
  const [shown, setShown] = useState(on)
  // Once nothing is running, the intent is whatever actually happened, so a
  // failed run cannot leave the next press pointing the wrong way.
  useEffect(() => {
    if (!busy) {
      wanted.current = on
      setShown(on)
    }
  }, [on, busy])
  const shownOn = shown
  const working = busy && shown
  const healthy = shownOn && health === "ok"
  // A card is the credential the helper dials with, so without one nothing can
  // start. The plate used to be `disabled` in that state, which made it look
  // broken: a dead control that also stays silent. It stays pressable and the
  // line under the headline says what is missing.
  const noCard = !card
  // One direction, read from the same state the plate shows. Before this the
  // headline said CONNECTING while the line under it said Disconnecting,
  // because one used the pressed state and the other the running one.
  const stateWord = working
    ? t("voiceConnecting")
    : noCard
      ? t("needCard")
      : busy
        ? t("voiceOffSub")
        : on
          ? health === "ok"
            ? t("voiceConnected")
            : health === "bad"
              ? t("voiceUnhealthy")
              : t("voiceChecking")
          : t("voiceOffSub")

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col">
      {/* the frame is the same chamfer one pixel wider, so the cut keeps an edge */}
      <div className="relative flex min-h-0 flex-1 flex-col" style={{ clipPath: PANEL, background: "var(--line-strong)" }}>
        <div className="relative flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--panel)]" style={{ clipPath: PANEL }}>
          <span aria-hidden className="pointer-events-none absolute -left-[12%] top-0 h-[58%] w-[124%] -skew-y-[7deg] bg-white/[0.022]" />
          <span
            aria-hidden
            className={cn(
              "pointer-events-none absolute -left-[12%] top-[58%] h-px w-[124%] -skew-y-[7deg] transition-colors duration-300",
              on ? "bg-[var(--green-line)]" : "bg-[var(--brand-line)]/55",
            )}
          />
          <span aria-hidden className="pointer-events-none absolute inset-y-6 left-4 flex w-3 flex-col justify-center gap-3">
            {Array.from({ length: 13 }).map((_, i) => (
              <span key={i} className={cn("block h-px bg-white/10", i % 3 === 0 ? "w-3" : "w-1.5")} />
            ))}
          </span>
          <span aria-hidden className="pointer-events-none absolute inset-y-6 right-4 flex w-3 flex-col justify-center gap-3">
            {Array.from({ length: 13 }).map((_, i) => (
              <span key={i} className={cn("ml-auto block h-px bg-white/10", i % 3 === 0 ? "w-3" : "w-1.5")} />
            ))}
          </span>

          <div className="relative flex min-h-0 flex-1 flex-col px-12 py-8">
            {/* the name sits where a HUD puts it, and the state answers it across the field */}
            <div className="flex items-start justify-between gap-6">
              <div className="min-w-0">
                <div className="flex items-center gap-3">
                  <ValorantMark className={cn("size-[22px] shrink-0", healthy ? "text-[var(--green)]" : "text-txt")} />
                  <span className="text-[15px] font-semibold uppercase leading-none tracking-[0.62em] text-txt">VALORANT</span>
                </div>
                <p className="mt-2.5 max-w-[360px] text-[11px] uppercase leading-relaxed tracking-[0.2em] text-txt2">
                  {t("voiceTagline")}
                </p>
              </div>
            </div>

            <div className="flex min-h-0 flex-1 items-center justify-between gap-10 py-4">
              {/* the state, left aligned, with a rule under it */}
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <span
                  className={cn(
                    "font-display text-[38px] font-semibold uppercase leading-none tracking-[0.01em] transition-colors duration-200",
                    healthy ? "text-[var(--green)]" : "text-txt",
                  )}
                >
                  {working && shownOn ? t("voiceConnecting") : shownOn ? t("voiceActive") : t("voiceOffTitle")}
                </span>
                <span className="max-w-[430px] text-[13px] leading-snug text-txt2">{stateWord}</span>
                {msg ? (
                  <span className="max-w-[440px] text-[12px] leading-snug text-txt3" role="status">{msg}</span>
                ) : null}
                <span aria-hidden className={cn("mt-3 h-px w-16", healthy ? "bg-[var(--green-line)]" : "bg-[var(--brand-line)]")} />
                <p className="max-w-[430px] text-[12px] leading-relaxed text-txt3">{t("launchPanelBody")}</p>
              </div>

              {/* the control: the game's shape, not a disc. It answers the
                  press the way the Home dial does, and a second press turns
                  the helper off again. */}
              <div className="relative grid size-[252px] shrink-0 place-items-center">
                {shownOn && !calm &&
                  [0, 1].map((i) => (
                    <motion.span
                      key={"cham" + i}
                      aria-hidden
                      className="pointer-events-none absolute inset-0"
                      style={{ clipPath: PLATE, border: "1px solid var(--green-line)" }}
                      initial={{ opacity: 0.75, scale: 0.94 }}
                      animate={{ opacity: 0, scale: 1.2 }}
                      transition={{ duration: 0.85, delay: i * 0.13, ease: [0.22, 1, 0.36, 1] }}
                    />
                  ))}
                <motion.button
                  type="button"
                  onClick={() => {
                    // Still coming up? A press takes it straight back down
                    // instead of stacking another engine run behind it.
                    if (busy && shownOn) {
                      wanted.current = false
                      setShown(false)
                      cancelVoice()
                      return
                    }
                    const next = !shownOn
                    wanted.current = next
                    setShown(next)
                    void setHelper(next)
                  }}
                  aria-disabled={noCard || undefined}
                  aria-pressed={shownOn}
                  aria-label={shownOn ? t("voiceDisconnect") : t("voiceRowTitle")}
                  whileTap={working ? undefined : { scale: 0.93 }}
                  transition={{ type: "spring", stiffness: 460, damping: 26, mass: 0.7 }}
                  className="group relative grid size-[252px] cursor-default select-none place-items-center outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]/50"
                  style={{ clipPath: PLATE }}
                >
                {working && (
                  <svg viewBox="0 0 100 100" className="pointer-events-none absolute inset-0 size-full" aria-hidden>
                    {/* a dim tail behind the bright head, so the light has a
                        length instead of two hard ends */}
                    <polygon
                      points="16,0 84,0 100,16 100,84 84,100 16,100 0,84 0,16"
                      fill="none"
                      stroke="var(--brand-vivid)"
                      strokeOpacity="0.22"
                      strokeWidth="2.4"
                      strokeLinecap="round"
                      strokeDasharray="150 192"
                      className="[animation:qc-trace_1.9s_linear_infinite]"
                    />
                    <polygon
                      points="16,0 84,0 100,16 100,84 84,100 16,100 0,84 0,16"
                      fill="none"
                      stroke="var(--brand-vivid)"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeDasharray="40 302"
                      className="[animation:qc-trace_1.9s_linear_infinite]"
                    />
                  </svg>
                )}
                <span aria-hidden className={cn("absolute inset-0 transition-colors duration-300", on ? "bg-[var(--green-line)]" : "bg-[var(--brand-line)]")} />
                <span aria-hidden className="absolute inset-[2px] bg-[var(--panel)] transition-colors duration-200 group-hover:bg-[#14171a]" style={{ clipPath: PLATE }} />
                <motion.span
                  aria-hidden
                  className="absolute inset-[2px]"
                  initial={false}
                  animate={{ boxShadow: on ? "inset 0 0 64px rgb(40 160 110 / 0.18)" : "inset 0 0 0 rgb(0 0 0 / 0)" }}
                  transition={{ duration: 0.45 }}
                  style={{ clipPath: PLATE }}
                />
                <span className="relative flex flex-col items-center gap-2.5">
                  {working ? (
                    <Mic className={cn("size-10", on ? "text-[var(--green)]/60" : "text-txt2/60")} aria-hidden />
                  ) : (
                    <Mic className={cn("size-12", on ? "text-[var(--green)]" : "text-txt2")} aria-hidden />
                  )}
                  {/* the plate says what a press will do, so the second press
                      is not a guess */}
                  <span
                    className={cn(
                      "text-[11.5px] font-semibold uppercase tracking-[0.2em]",
                      on ? "text-[var(--green)]/90" : "text-txt3",
                    )}
                  >
                    {shownOn ? t("stop") : t("connect")}
                  </span>
                </span>
                </motion.button>
              </div>
            </div>

            {/* the switch sits on the floor, where a HUD keeps its settings */}
            <div className="flex items-end justify-between gap-6">
              <div className="flex items-center gap-5 bg-white/[0.045] px-6 py-3" style={{ clipPath: CHIP }}>
                <span className="text-[11px] font-medium uppercase tracking-[0.2em] text-txt2">{t("voiceLaunchTitle")}</span>
                <button
                  type="button"
                  role="switch"
                  aria-checked={launch}
                  aria-label={t("voiceLaunchTitle")}
                  disabled={working}
                  onClick={() => void flipLaunch()}
                  className="press relative h-8 w-[62px] shrink-0 transition-colors duration-200"
                  style={{ clipPath: SWITCH }}
                >
                  <span aria-hidden className={cn("absolute inset-0 transition-colors duration-200", launch ? "bg-[var(--brand)]/85" : "bg-white/[0.12]")} />
                  <span
                    aria-hidden
                    className={cn(
                      "absolute bottom-1 top-1 w-6 bg-[var(--txt)] transition-[left] duration-200 ease-out",
                      launch ? "left-[calc(100%-28px)]" : "left-1",
                    )}
                    style={{ clipPath: KNOB }}
                  />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
