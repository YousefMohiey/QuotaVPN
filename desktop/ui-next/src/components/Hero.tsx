import { useEffect, useMemo, useRef, useState } from "react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"
import { useApp } from "@/state/app"
import { api, netInfo } from "@/lib/ipc"
import { Dial, type DialState } from "./Dial"
import { displayHost, fmtBytes, fmtDuration, mbps } from "@/lib/format"

/** A one-second heartbeat for the session clock; polling still drives data. */
export function useTick(ms: number, on: boolean) {
  const [, set] = useState(0)
  useEffect(() => {
    if (!on) return
    const id = window.setInterval(() => set((n) => n + 1), ms)
    return () => window.clearInterval(id)
  }, [ms, on])
}

/** Live rates from the adapter counters: combined, per direction, and peak. */
export function useTraffic(rx: number, tx: number) {
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

/** Bytes per second as the live number: Mbps once it is a real rate,
    KB/s below that. Used for both the big value and the peak. */
export function rateText(bps: number): { num: string; unit: string } {
  const m = (Math.max(0, bps) * 8) / 1e6
  if (m >= 1) return { num: mbps(m), unit: "Mbps" }
  return { num: String(Math.round(Math.max(0, bps) / 1024)), unit: "KB/s" }
}

/** Where this machine appears from, read off its public address. The lookup
    races its providers on the Rust side, and the last answer is cached so a
    launch never starts on a dash. A lookup that lands right after a
    disconnect can still ride the dying tunnel and report the exit the user
    just left, so while disconnected the answer is challenged until it stops
    matching that exit. */
export function useGeo(connected: boolean) {
  const [geo, setGeo] = useState<{ ip: string; place: string; isp: string } | null>(() => {
    try {
      const raw = localStorage.getItem("qc-geo")
      return raw ? (JSON.parse(raw) as { ip: string; place: string; isp: string }) : null
    } catch {
      return null
    }
  })
  // The address seen while the tunnel is up: after a disconnect a fresh
  // lookup must move away from it before it is trusted.
  const exitRef = useRef<string | null>(null)
  useEffect(() => {
    let alive = true
    let timer: number | undefined
    let n = 0
    const budget = connected ? 2 : 6
    const grab = () => {
      void netInfo()
        .then((v) => {
          if (!alive) return
          if (v && v.ip) {
            if (connected) exitRef.current = v.ip
            // Still reporting the exit we just left: the tunnel teardown is
            // racing this lookup. Keep asking before believing it.
            if (!connected && exitRef.current && v.ip === exitRef.current && n < budget) {
              n += 1
              timer = window.setTimeout(grab, 2500)
              return
            }
            setGeo(v)
            try {
              localStorage.setItem("qc-geo", JSON.stringify(v))
            } catch {
              /* private mode */
            }
            return
          }
          if (n < budget) {
            n += 1
            timer = window.setTimeout(grab, 2500)
          }
        })
        .catch(() => {
          if (alive && n < budget) {
            n += 1
            timer = window.setTimeout(grab, 2500)
          }
        })
    }
    // After a disconnect let the adapter teardown land before asking: an
    // immediate lookup used to ride the last packets of the tunnel.
    timer = window.setTimeout(grab, connected ? 0 : 2000)
    return () => {
      alive = false
      if (timer) window.clearTimeout(timer)
    }
  }, [connected])
  return geo
}

/** While the tunnel is up the world sees the server's address, so resolve it
    once and show the IP the user actually appears as - never the hostname. */
export function useServerAddr(connected: boolean, serverIp: string) {
  const [addr, setAddr] = useState("")
  useEffect(() => {
    if (!connected || !serverIp) {
      setAddr("")
      return
    }
    let alive = true
    let timer: number | undefined
    // The first lookup through a fresh tunnel can be cold, and a single
    // miss used to leave the panel showing the raw name for the rest of
    // the session. Keep asking for a bit before settling for the name.
    const attempt = (n: number) => {
      api
        .resolveHost(serverIp)
        .then((ip) => {
          if (!alive) return
          if (ip) setAddr(ip)
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
  return addr
}

/**
 * The connection card: the dial and its reading side by side, one layout in
 * every state. The word, the profile, and while the tunnel is up the clock,
 * the moved bytes and the live rates. Nothing travels, nothing fades.
 */
export function Hero() {
  const { t } = useI18n()
  const { phase, connected, busy, toggle, card, rx, tx, sessionStart, status } = useApp()

  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""
  const working = phase === "connecting"

  useTick(1000, connected)
  const traffic = useTraffic(rx, tx)

  return (
    <section className="flex min-h-0 flex-1 items-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] p-5">
      <div className="flex w-full items-center gap-7">
        <Dial state={state} onClick={toggle} disabled={busy && phase !== "connecting"} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span
              aria-hidden
              className={cn(
                "size-1.5 rounded-full",
                connected ? "bg-[var(--green)]" : working ? "pulse-dot bg-[var(--brand)]" : "bg-txt3",
              )}
            />
            <span
              className={cn(
                "truncate text-[13px]",
                connected ? "text-[var(--green)]" : "text-txt2",
              )}
            >
              {connected ? t("connected") : working ? t("working") : t("notConnected")}
            </span>
          </div>
          <p className="mt-3 truncate text-[18px] font-semibold leading-tight text-txt" dir="auto">
            {name || "-"}
          </p>
          <p className="mt-0.5 truncate font-mono text-[11.5px] text-txt3" dir="auto">
            {displayHost(card?.sni)}
          </p>
          {connected ? (
            <div className="mt-3 flex flex-col gap-1 text-[12px] tabular-nums text-txt2">
              <span className="truncate">
                <span className="text-txt3">{t("sessLabel")} </span>
                {sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
                <span className="text-txt3"> · ↓ {fmtBytes(rx)} ↑ {fmtBytes(tx)}</span>
              </span>
              <span className="truncate">
                <span className="text-txt3">{t("liveLabel")} · </span>↓ {traffic.down.num} {traffic.down.unit}
                {"  "}↑ {traffic.up.num} {traffic.up.unit}
              </span>
            </div>
          ) : null}
          {status ? (
            <div className="mt-2 select-text text-[11px] leading-snug text-txt3">{status}</div>
          ) : null}
        </div>
      </div>
    </section>
  )
}

/** The facts panel: where this machine sits and appears from right now. */
export function FactsCard() {
  const { t } = useI18n()
  const { connected, serverIp } = useApp()
  const geo = useGeo(connected)
  const serverAddr = useServerAddr(connected, serverIp)
  const ipText = connected ? serverAddr || geo?.ip || "-" : geo?.ip || "-"

  return (
    <section className="rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <div className="flex flex-col divide-y divide-line">
        <FactRow label={t("yourLocation")} value={geo?.place || "-"} />
        <FactRow label={t("yourIp")} value={ipText} select />
        <FactRow label={t("ispLbl")} value={geo?.isp || "-"} />
        <FactRow
          label={t("statusLbl")}
          value={connected ? t("connected") : t("notConnected")}
          dot={connected ? "on" : "off"}
        />
      </div>
    </section>
  )
}

/** One fact as a line: the name left, the reading right. */
function FactRow({
  label,
  value,
  select,
  dot,
}: {
  label: string
  value: string
  select?: boolean
  dot?: "on" | "off"
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <span className="shrink-0 text-[12px] text-txt2">{label}</span>
      <span className="flex min-w-0 items-center gap-2">
        {dot ? (
          <span
            aria-hidden
            className={cn("size-1.5 shrink-0 rounded-full", dot === "on" ? "bg-[var(--green)]" : "bg-txt3")}
          />
        ) : null}
        <span className={cn("truncate text-[13px] text-txt", select && "select-text")} dir="auto">
          {value}
        </span>
      </span>
    </div>
  )
}
