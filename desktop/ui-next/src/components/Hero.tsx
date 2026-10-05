import { useEffect, useMemo, useRef, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { Activity, Globe, Network, type LucideIcon } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { api, netInfo } from "@/lib/ipc"
import { Dial, type DialState } from "./Dial"
import { displayHost, fmtBytes, fmtDuration, mbps } from "@/lib/format"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const

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
 * The connection card, one composition: the glass dial anchors the left,
 * and the right zone carries the state line, the action word and the host.
 * Under them the zone shows its reading: the three connection facts while
 * idle, and the live session (rate and graph, then the timers) once the
 * tunnel is up. One zone, two readings, nothing else on the card.
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

  // The dial sits left in every state, like the mockup: the word column
  // beside it tells the state and carries the session once connected.
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""
  const hostLine = [name, displayHost(card?.sni)].filter(Boolean).join(" · ")

  useTick(1000, connected)

  return (
    <section className="flex flex-1 flex-col rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
      <div className="flex flex-1 items-center gap-6">
        <div className="flex shrink-0 items-center justify-center">
          <Dial state={state} onClick={toggle} disabled={busy} />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span
              aria-hidden
              className={cn(
                "size-1.5 rounded-full",
                connected
                  ? "bg-[var(--green)]"
                  : state === "connecting"
                    ? "pulse-dot bg-[var(--brand)]"
                    : "bg-[var(--red)]",
              )}
            />
            <span className="text-[12.5px] text-txt2">
              {connected ? t("connected") : state === "connecting" ? t("working") : t("notConnected")}
            </span>
          </div>

          <button
            type="button"
            onClick={toggle}
            disabled={busy}
            className="mt-2 block max-w-full truncate text-left text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong disabled:cursor-wait"
          >
            {connected ? t("disconnect") : state === "connecting" ? t("working") : t("connect")}
          </button>
          <p className="mt-1 truncate text-[13px] text-txt3" dir="auto">
            {connected ? hostLine : t("clickToConnect")}
          </p>
          {status ? (
            <div className="mt-2 max-w-[520px] text-[11.5px] leading-snug text-txt3">{status}</div>
          ) : null}

          {/* the zone's reading: facts while idle, the live session once up */}
          <AnimatePresence mode="wait" initial={false}>
            {connected ? (
              <motion.div
                key="session"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.3, ease: EASE_OUT, delay: 0.08 }}
              >
                <div className="mt-5 flex items-stretch gap-4">
                  <div className="flex min-w-0 flex-1 flex-col justify-center">
                    <Traffic rx={rx} tx={tx} />
                  </div>
                  <div className="flex w-[212px] shrink-0 flex-col justify-center gap-2">
                    <StatTile
                      label={t("sessLabel")}
                      value={sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
                      sub={"⁨↓ " + fmtBytes(rx) + "⁩   ⁨↑ " + fmtBytes(tx) + "⁩"}
                    />
                    <StatTile label={t("srvLocation")} value={placeText} sub={serverAddr || ipText} />
                  </div>
                </div>
              </motion.div>
            ) : (
              <motion.div
                key="facts"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.3, ease: EASE_OUT }}
              >
                <div className="mt-5 grid grid-cols-3 gap-4 border-t border-line pt-4">
                  <Fact icon={Globe} label={t("srvLocation")} value={placeText} />
                  <Fact icon={Network} label={t("yourIp")} value={ipText} />
                  <Fact
                    icon={Activity}
                    label={t("statusLbl")}
                    value={connected ? t("connected") : t("notConnected")}
                    dot={connected}
                  />
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
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
    <div className="flex min-w-0 items-center gap-2.5">
      <span className="grid size-[30px] shrink-0 place-items-center rounded-[9px] border border-line bg-white/[0.03] text-brand-strong">
        <Icon className="size-[15px]" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="whitespace-nowrap text-[11px] text-txt3">{label}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[12.5px] text-txt">
          {dot !== undefined && (
            <span className={cn("size-[6px] shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-[var(--red)]")} aria-hidden />
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
      <svg viewBox="0 0 260 44" preserveAspectRatio="none" className="mt-2 h-[48px] w-full" aria-hidden>
        <path d={area} fill="var(--brand-bg)" />
        <path
          d={line}
          fill="none"
          stroke="var(--brand)"
          strokeWidth="2"
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
