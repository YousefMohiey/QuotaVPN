import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import { AnimatePresence, motion } from "motion/react"
import { RefreshCw } from "lucide-react"
import { cn } from "@/lib/utils"
import { WeMark } from "./WeMark"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { fmtQuota, quotaRefresh, useQuota } from "@/state/quota"
import { WeQuotaDialog } from "./WeQuotaDialog"
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

/** Live rates from the adapter counters: combined, per direction, and peak. */
function useTraffic(rx: number, tx: number) {
  const hist = useRef<number[]>([])
  const last = useRef({ rx: 0, tx: 0, t: Date.now(), down: 0, up: 0 })

  useEffect(() => {
    const now = Date.now()
    const dt = Math.max(0.2, (now - last.current.t) / 1000)
    const down = Math.max(0, rx - last.current.rx) / dt
    const up = Math.max(0, tx - last.current.tx) / dt
    last.current = { rx, tx, t: now, down, up }
    if (rx || tx) hist.current = [...hist.current.slice(-59), down + up]
  }, [rx, tx])

  return useMemo(() => {
    const peak = hist.current.length ? Math.max(0, ...hist.current) : 0
    return {
      rate: rateText(last.current.down + last.current.up),
      down: rateText(last.current.down),
      up: rateText(last.current.up),
      peak: rateText(peak),
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rx, tx])
}

/**
 * The connection card. The dial anchors the left and glides from center to
 * its parking spot the moment Connect is pressed; the reading fades in
 * beside it. The right zone carries the WE line reading, and the strip
 * under everything swaps its three cells with the tunnel: facts while it
 * is down, the live session while it is up.
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
    // miss used to leave the strip showing the raw name for the rest of
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

  // The strip facts: where this machine appears from, read off its public
  // address. The lookup races its providers on the Rust side, and the last
  // answer is cached so a launch never starts on a dash.
  const [geo, setGeo] = useState<{ ip: string; place: string; isp: string } | null>(() => {
    try {
      const raw = localStorage.getItem("qc-geo")
      return raw ? (JSON.parse(raw) as { ip: string; place: string; isp: string }) : null
    } catch {
      return null
    }
  })
  useEffect(() => {
    let alive = true
    let timer: number | undefined
    const grab = (n: number) => {
      void netInfo()
        .then((v) => {
          if (!alive) return
          if (v && v.ip) {
            setGeo(v)
            try {
              localStorage.setItem("qc-geo", JSON.stringify(v))
            } catch {
              /* private mode */
            }
            return
          }
          if (n < 2) timer = window.setTimeout(() => grab(n + 1), 2500)
        })
        .catch(() => {
          if (alive && n < 2) timer = window.setTimeout(() => grab(n + 1), 2500)
        })
    }
    grab(0)
    return () => {
      alive = false
      if (timer) window.clearTimeout(timer)
    }
  }, [connected])
  const ipText = geo?.ip || "-"

  // The dial starts centered and parks left the moment Connect is pressed.
  // The reading zone fades in beside it only after the slide has had room
  // to travel; on the way back the zone exits first, then the dial returns.
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""
  const active = phase === "connecting" || phase === "on"

  const [showInfo, setShowInfo] = useState(false)
  useEffect(() => {
    if (!active) {
      setShowInfo(false)
      return
    }
    const id = window.setTimeout(() => setShowInfo(true), 250)
    return () => window.clearTimeout(id)
  }, [active])

  const [dialLeft, setDialLeft] = useState(false)
  useEffect(() => {
    if (active) {
      setDialLeft(true)
      return
    }
    if (phase === "stopping") {
      // Short hold only: the zone exit leads by a beat, then the dial
      // answers at once. A long hold here reads as a dead pause.
      const id = window.setTimeout(() => setDialLeft(false), 140)
      return () => window.clearTimeout(id)
    }
    setDialLeft(false)
  }, [active, phase])

  useTick(1000, connected)
  const traffic = useTraffic(rx, tx)

  return (
    <section className="flex min-h-0 flex-1 flex-col rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] p-5">
      <div className="flex min-h-0 flex-1 items-center">
        <motion.div
          layout
          transition={SPRING}
          className={cn(
            /* The reading zone is taller than the dial, so without a floor the
               row would resize as the zone comes and goes and the spring would
               wobble the dial with it. Held at the zone's own height, the row
               never changes and the glide stays flat. */
            "flex min-h-[188px] flex-1 items-center",
            dialLeft ? "justify-start gap-8" : "justify-center",
          )}
        >
          <motion.div layout transition={SPRING} className="flex shrink-0 items-center justify-center">
            <Dial state={state} onClick={toggle} disabled={busy && phase !== "connecting"} />
          </motion.div>

          <AnimatePresence mode="popLayout">
            {showInfo && (
              <motion.div
                key="info"
                initial={{ opacity: 0, x: 16 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 12 }}
                transition={{ duration: 0.32, ease: EASE_OUT, delay: 0.06 }}
                className="min-w-0 flex-1"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span
                      aria-hidden
                      className={cn(
                        "size-1.5 rounded-full",
                        connected ? "bg-[var(--green)]" : "pulse-dot bg-[var(--brand)]",
                      )}
                    />
                    <span
                      className={cn(
                        "truncate text-[12.5px]",
                        connected ? "text-[var(--green)]" : "text-txt2",
                      )}
                    >
                      {connected ? t("connected") : t("working")}
                    </span>
                  </div>
                  {connected ? (
                    <>
                      <p className="mt-2 truncate text-[22px] font-semibold leading-tight text-txt" dir="auto">
                        {name}
                      </p>
                      <p className="mt-0.5 truncate text-[12.5px] text-txt3" dir="auto">
                        {displayHost(card?.sni)}
                      </p>
                    </>
                  ) : (
                    <p className="mt-2 truncate text-[15px] font-medium text-txt" dir="auto">
                      {t("talking")}
                    </p>
                  )}
                  {status ? (
                    <div className="mt-2 select-text text-[11.5px] leading-snug text-txt3">{status}</div>
                  ) : null}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>

        <QuotaZone />
      </div>

      {/* the strip: facts while down, the live session while up */}
      <div className="mt-3 grid min-h-[64px] grid-cols-3 items-center divide-x divide-line border-t border-line pt-3">
        {connected ? (
          <>
            <Cell
              label={t("sessLabel")}
              value={sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
              sub={`↓ ${fmtBytes(rx)}   ↑ ${fmtBytes(tx)}`}
            />
            <Cell className="px-5" label={t("yourIp")} value={serverAddr || ipText} select />
            <Cell
              className="ps-5"
              label={t("liveLabel")}
              right={`${t("peakLbl")} ${traffic.peak.num} ${traffic.peak.unit}`}
              value={`${traffic.rate.num} ${traffic.rate.unit}`}
              sub={`↓ ${traffic.down.num} ${traffic.down.unit}   ↑ ${traffic.up.num} ${traffic.up.unit}`}
            />
          </>
        ) : (
          <>
            <Cell className="pe-5" label={t("yourLocation")} value={geo?.place || "-"} />
            <Cell className="px-5" label={t("yourIp")} value={ipText} select />
            <Cell className="ps-5" label={t("statusLbl")} value={t("notConnected")} />
          </>
        )}
      </div>
    </section>
  )
}

/** The WE line panel: the reading when linked, the invite when not. */
function QuotaZone() {
  const { t } = useI18n()
  const quota = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const mainUnit = useMemo(
    () => quota.units.find((u) => u.total > 0),
    [quota.units],
  )

  // The linked WE line reads itself once per launch; the tap refreshes it.
  useEffect(() => {
    void quotaRefresh()
  }, [])

  const pct =
    mainUnit && mainUnit.total > 0
      ? Math.max(0, Math.min(100, (mainUnit.remain / mainUnit.total) * 100))
      : 0

  return (
    <div className="ms-6 w-[240px] shrink-0 self-stretch border-s border-line ps-6">
      <div className="flex h-full flex-col justify-center">
        {quota.linked && mainUnit ? (
          <div>
            <div className="flex items-center gap-3">
              <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
                <WeMark className="w-[22px]" />
              </span>
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-[13.5px] font-semibold text-txt">{t("weQuota")}</h2>
                <p className="mt-0.5 truncate text-[11px] text-txt2" dir="auto" title={mainUnit.offer || "WE"}>
                  {mainUnit.offer || "WE"}
                </p>
              </div>
              <button
                type="button"
                onClick={() => void quotaRefresh(true)}
                title={t("weRefresh")}
                aria-label={t("weRefresh")}
                disabled={quota.phase === "busy"}
                className="grid size-8 shrink-0 place-items-center rounded-full border border-line bg-white/[0.02] text-txt2 transition-colors hover:border-[var(--brand-line)] hover:text-brand-strong disabled:cursor-wait"
              >
                <RefreshCw className={cn("size-3.5", quota.phase === "busy" && "animate-spin")} aria-hidden />
              </button>
            </div>
            <div className="mt-4 text-[24px] font-semibold leading-none tabular-nums text-txt">
              {fmtQuota(mainUnit.remain)}
            </div>
            <div className="mt-1.5 text-[12px] text-txt2">
              {t("weLeft")} {fmtQuota(mainUnit.total)}
            </div>
            <div className="mt-3.5 h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
              <div
                className="h-full rounded-full bg-[var(--brand-vivid)] transition-[width] duration-700 ease-out"
                style={{ width: `${pct}%` }}
              />
            </div>
            <div className="mt-2 flex items-baseline justify-between gap-2 text-[11px] text-txt3">
              <span className="truncate">
                {t("weUsed")} {fmtQuota(mainUnit.used)}
              </span>
              {quota.updatedAt ? (
                <span className="shrink-0 tabular-nums">
                  {t("weUpdated")}{" "}
                  {new Date(quota.updatedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
                </span>
              ) : null}
            </div>
            {quota.phase === "error" && quota.msgKey ? (
              <p className="mt-1.5 truncate text-[10.5px] text-[var(--red)]" dir="auto">
                {t(quota.msgKey as Parameters<typeof t>[0])}
                {quota.msgRaw ? ` (${quota.msgRaw})` : ""}
              </p>
            ) : null}
          </div>
        ) : (
          <div>
            <div className="flex items-center gap-3">
              <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
                <WeMark className="w-[22px]" />
              </span>
              <h2 className="min-w-0 truncate text-[13.5px] font-semibold text-txt">{t("weQuota")}</h2>
            </div>
            <p className="mt-2 text-[11px] leading-snug text-txt2">{t("weQuotaSub")}</p>
            <button
              type="button"
              onClick={() => setQuotaOpen(true)}
              className="mt-3 flex h-9 w-full items-center justify-center rounded-[9px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-3.5 text-[12.5px] font-medium text-[var(--brand-strong)] transition-colors hover:border-[var(--brand-strong)]"
            >
              {t("weLink")}
            </button>
            <p className="mt-2 text-[10.5px] leading-snug text-txt3">{t("weLinkHint")}</p>
          </div>
        )}
      </div>
      <WeQuotaDialog open={quotaOpen} onOpenChange={setQuotaOpen} />
    </div>
  )
}

/** One strip cell: a quiet label, then the value with its own reading. */
function Cell({
  label,
  value,
  sub,
  right,
  select,
  className,
}: {
  label: string
  value: string
  sub?: string
  right?: ReactNode
  select?: boolean
  className?: string
}) {
  return (
    <div className={cn("min-w-0", className)}>
      <div className="flex items-baseline justify-between gap-2">
        <span className="truncate text-[11px] text-txt3">{label}</span>
        {right ? <span className="shrink-0 truncate text-[10.5px] tabular-nums text-txt3">{right}</span> : null}
      </div>
      <div
        className={cn("mt-1 truncate text-[15px] font-medium leading-tight tabular-nums text-txt", select && "select-text")}
        dir="auto"
        title={value}
      >
        {value}
      </div>
      {sub ? <div className="mt-0.5 truncate text-[11px] tabular-nums text-txt3">{sub}</div> : null}
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
