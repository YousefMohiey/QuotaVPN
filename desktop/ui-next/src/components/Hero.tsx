import { useEffect, useMemo, useRef, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { ArrowUpDown, Globe, MapPin, type LucideIcon } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { api, netInfo } from "@/lib/ipc"
import { Dial, type DialState } from "./Dial"
import { displayHost, fmtBytes, fmtDuration, mbps } from "@/lib/format"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const
const SPRING = { type: "spring", stiffness: 320, damping: 34 } as const

/** A one-second heartbeat for the session clock; polling still drives data. */
function useTick(ms: number, on: boolean) {
  const [, set] = useState(0)
  useEffect(() => {
    if (!on) return
    const id = window.setInterval(() => set((n) => n + 1), ms)
    return () => window.clearInterval(id)
  }, [ms, on])
}

/**
 * The connection card: the dial parks left the moment Connect is pressed
 * and the reading column takes its place behind the same hairline divider
 * the Valorant cards use. The footer strip carries the three connection
 * facts, so the page is two calm cards instead of a stack of boxes.
 */
export function Hero() {
  const { t } = useI18n()
  const { phase, connected, busy, toggle, card, rx, tx, sessionStart, serverIp, status } = useApp()
  const [serverAddr, setServerAddr] = useState("")

  // While the tunnel is up the world sees the server's address, so resolve it
  // once and show the IP the user actually appears as - never the hostname.
  useEffect(() => {
    if (!connected || !serverIp) {
      setServerAddr("")
      return
    }
    let alive = true
    let timer: number | undefined
    // The first lookup through a fresh tunnel can be cold, and a single
    // miss used to leave this tile showing the raw name for the rest of
    // the session. Keep asking for a bit before settling for the name.
    const attempt = (n: number) => {
      api
        .resolveHost(serverIp)
        .then((ip) => {
          if (!alive) return
          if (ip) setServerAddr(ip)
          else if (n < 5) timer = window.setTimeout(() => attempt(n + 1), 3000)
        })
        .catch(() => {
          if (alive && n < 5) timer = window.setTimeout(() => attempt(n + 1), 3000)
        })
    }
    attempt(0)
    return () => {
      alive = false
      if (timer) window.clearTimeout(timer)
    }
  }, [connected, serverIp])

  // Footer facts: the exit address while connected, plus the server's
  // location (remembered across sessions so it survives a disconnect).
  const [exit, setExit] = useState<{ ip: string; place: string; isp: string } | null>(null)
  const [srvPlace, setSrvPlace] = useState<string>(() => {
    try {
      return localStorage.getItem("qc-srv-place") || ""
    } catch {
      return ""
    }
  })
  useEffect(() => {
    let alive = true
    void netInfo()
      .then((n) => {
        if (alive && n) setExit({ ip: n.ip, place: n.place, isp: n.isp })
      })
      .catch(() => {
        /* the facts fall back to what we already know */
      })
    return () => {
      alive = false
    }
  }, [connected])
  useEffect(() => {
    if (!connected || !exit?.place) return
    setSrvPlace(exit.place)
    try {
      localStorage.setItem("qc-srv-place", exit.place)
    } catch {
      /* private mode */
    }
  }, [connected, exit])
  const placeText = connected ? exit?.place || srvPlace || displayHost(serverIp) : srvPlace || "-"
  const ipText = exit?.ip || "-"

  // The dial parks left from the moment Connect is pressed, and starts its
  // way back the moment Disconnect is pressed, not when the engine answers.
  const active = phase === "connecting" || phase === "on"
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""

  // The info panel waits for the dial to land first. Mounting both at once
  // is what made the motion feel fast and rough, so the panel fades in
  // only after the slide has had room to travel.
  const [showInfo, setShowInfo] = useState(false)
  useEffect(() => {
    if (!active) {
      setShowInfo(false)
      return
    }
    const id = window.setTimeout(() => setShowInfo(true), 250)
    return () => window.clearTimeout(id)
  }, [active])

  // Disconnect plays in order too: the panel exits first and the dial only
  // starts home once the exit is nearly done, never both at once.
  const [dialLeft, setDialLeft] = useState(false)
  useEffect(() => {
    if (active) {
      setDialLeft(true)
      return
    }
    if (phase === "stopping") {
      // Short hold only: the panel exit leads by a beat, then the dial
      // answers at once. A long hold here reads as a dead pause.
      const id = window.setTimeout(() => setDialLeft(false), 140)
      return () => window.clearTimeout(id)
    }
    setDialLeft(false)
  }, [active, phase])

  useTick(1000, connected)

  return (
    <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
      <div className="flex min-h-[190px] items-center">
        <motion.div
          layout
          transition={SPRING}
          className={cn("flex w-full items-center", dialLeft ? "justify-start gap-7" : "justify-center")}
        >
          <motion.div layout transition={SPRING} className="flex shrink-0 flex-col items-center text-center">
            <Dial state={state} onClick={toggle} disabled={busy} />
            {!connected && (
              <div className="mt-3 max-w-[190px] truncate text-center text-[12.5px] text-txt2" dir="auto">{name}</div>
            )}
            {status ? (
              <div className={cn("max-w-[280px] text-center text-[11.5px] leading-snug text-txt3", connected ? "mt-3" : "mt-2")}>
                {status}
              </div>
            ) : null}
          </motion.div>

          <AnimatePresence mode="popLayout">
            {showInfo && (
              <motion.div
                key="info"
                initial={{ opacity: 0, x: 16 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 12 }}
                transition={{ duration: 0.32, ease: EASE_OUT, delay: 0.06 }}
                className="min-w-0 flex-1 lg:border-l lg:border-line lg:pl-6"
              >
                <div className="flex items-center gap-2">
                  <span
                    aria-hidden
                    className={cn(
                      "size-1.5 rounded-full",
                      connected ? "bg-[var(--green)]" : "pulse-dot bg-[var(--brand)]",
                    )}
                  />
                  <span className="text-[12.5px] text-txt2">
                    {connected ? t("connected") : phase === "on" ? t("connecting") : t("working")}
                  </span>
                  {card && (
                    <span className="rounded-full border border-line-strong px-2 py-[3px] text-[11px] text-txt3">
                      {card.card_type === "Streamerz" ? t("kindStreamerz") : t("kindGamerz")}
                    </span>
                  )}
                </div>

                <h1 className="mt-2 truncate text-[22px] font-semibold tracking-[-0.01em] text-txt">{name}</h1>
                <p className="mt-1 truncate text-[12.5px] text-txt3">
                  {connected ? displayHost(card?.sni) : t("talking")}
                </p>

                <AnimatePresence>
                  {connected && (
                    <motion.div
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.36, ease: EASE_OUT, delay: 0.12 }}
                    >
                      <div className="mt-3 flex items-stretch gap-3">
                        <div className="flex min-w-0 flex-1 flex-col justify-center">
                          <Traffic rx={rx} tx={tx} />
                        </div>
                        <div className="flex w-[200px] shrink-0 flex-col justify-center gap-2">
                          <StatTile
                            label={t("sessLabel")}
                            value={sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
                            sub={"⁨↓ " + fmtBytes(rx) + "⁩   ⁨↑ " + fmtBytes(tx) + "⁩"}
                          />
                          <StatTile label={t("yourIp")} value={serverAddr || displayHost(serverIp)} sub={exit?.isp || undefined} />
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>
      </div>

      {/* the connection facts, a quiet strip inside the same card */}
      <div className="mt-5 grid grid-cols-3 gap-4 border-t border-line pt-4">
        <Fact icon={Globe} label={t("srvLocation")} value={placeText} />
        <Fact icon={MapPin} label={t("yourIp")} value={ipText} />
        <Fact
          icon={ArrowUpDown}
          label={t("statusLbl")}
          value={connected ? t("connected") : t("notConnected")}
          dot={connected}
        />
      </div>
    </section>
  )
}

/** One footer fact: icon tile, quiet label, one value line. */
function Fact({
  icon: Icon,
  label,
  value,
  dot,
}: {
  icon: LucideIcon
  label: string
  value: string
  dot?: boolean
}) {
  return (
    <div className="flex min-w-0 items-center gap-3">
      <span className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-line bg-white/[0.03] text-brand-strong">
        <Icon className="size-4" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="text-[11px] text-txt3">{label}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[13px] text-txt">
          {dot !== undefined && (
            <span className={cn("size-1.5 shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-txt3")} aria-hidden />
          )}
          <span className="truncate" dir="auto">
            {value}
          </span>
        </div>
      </div>
    </div>
  )
}

function StatTile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="rounded-[12px] border border-line bg-white/[0.02] px-3 py-2">
      <div className="flex items-baseline justify-between gap-2">
        <span className="shrink-0 text-[11px] text-txt3">{label}</span>
        <span className="truncate text-[13px] font-medium tabular-nums text-txt">{value}</span>
      </div>
      {sub && <div className="mt-0.5 truncate text-[11px] tabular-nums text-txt3">{sub}</div>}
    </div>
  )
}

/** Live rate + rolling sparkline from the adapter counters. The block
    reads like the reference: label row with the peak, the big number,
    then the graph. */
function Traffic({ rx, tx }: { rx: number; tx: number }) {
  const { t } = useI18n()
  const hist = useRef<number[]>([])
  const last = useRef({ rx: 0, tx: 0, t: Date.now() })

  useEffect(() => {
    const now = Date.now()
    const dt = Math.max(0.2, (now - last.current.t) / 1000)
    const delta = Math.max(0, rx - last.current.rx) + Math.max(0, tx - last.current.tx)
    last.current = { rx, tx, t: now }
    if (last.current.rx || last.current.tx) {
      hist.current = [...hist.current.slice(-59), delta / dt]
    }
  }, [rx, tx])

  const { line, area, rate, peak } = useMemo(() => {
    const values = hist.current.length ? hist.current : [0, 0]
    const max = Math.max(1, ...values)
    const w = 260
    const h = 44
    const step = w / Math.max(1, values.length - 1)
    const pts = values.map((v, i) => [i * step, h - (v / max) * (h - 8) - 3] as const)
    const d = pts
      .map(([x, y], i) => `${i ? "L" : "M"} ${x.toFixed(1)} ${y.toFixed(1)}`)
      .join(" ")
    const current = values[values.length - 1] || 0
    return {
      line: d,
      area: `${d} L ${w} ${h} L 0 ${h} Z`,
      rate: rateText(current),
      peak: rateText(Math.max(0, ...values)),
    }
  }, [rx, tx])

  return (
    <div>
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-[11.5px] tracking-[0.04em] text-txt3">{t("liveLabel")}</span>
        <span className="text-[11.5px] tabular-nums text-txt3">
          {t("peakLbl")} {peak.num} {peak.unit}
        </span>
      </div>
      <div className="mt-1 flex items-baseline gap-1.5">
        <span className="text-[20px] font-semibold leading-none tabular-nums text-txt">{rate.num}</span>
        <span className="text-[12px] text-txt2">{rate.unit}</span>
      </div>
      <svg viewBox="0 0 260 44" preserveAspectRatio="none" className="mt-1.5 h-[30px] w-full" aria-hidden>
        <path d={area} fill="var(--brand-bg)" />
        <path
          d={line}
          fill="none"
          stroke="var(--brand)"
          strokeWidth="1.6"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
      </svg>
    </div>
  )
}

/** Bytes per second as the live number: Mbps once it is a real rate,
    KB/s below that. Used for both the big value and the peak. */
function rateText(bps: number): { num: string; unit: string } {
  const m = (Math.max(0, bps) * 8) / 1e6
  if (m >= 1) return { num: mbps(m), unit: "Mbps" }
  return { num: String(Math.round(Math.max(0, bps) / 1024)), unit: "KB/s" }
}
