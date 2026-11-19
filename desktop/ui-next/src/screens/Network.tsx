import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Activity, ArrowDown, ArrowUp, Ban, Copy, Gauge, Loader2, RefreshCw, Square } from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { copyText } from "@/lib/clipboard"
import * as api from "@/lib/ipc"
import type { LanDevice, LanStatus } from "@/lib/ipc"
import { cn } from "@/lib/utils"

// The Network page: watch every device at once, cap one, block one.
//
// Shape: header with the two page-level actions, a totals strip that only
// exists while the watch is on, then one card with the roster. While watching,
// every row grows two live columns (down/up) fed by one snapshot per second, so
// the list itself is the monitor. Capping and blocking act on a single device,
// which is the only thing the engine allows.
type Rate = { down: number; up: number }

export function Network() {
  const { t } = useI18n()
  const [list, setList] = useState<LanDevice[]>([])
  const [scanning, setScanning] = useState(true)
  const [scanError, setScanError] = useState("")
  const [query, setQuery] = useState("")
  const [busy, setBusy] = useState("")
  const [note, setNote] = useState("")
  const [copied, setCopied] = useState("")

  const [watching, setWatching] = useState(false)
  const [rates, setRates] = useState<Record<string, Rate>>({})
  const [total, setTotal] = useState<Rate>({ down: 0, up: 0 })

  const [session, setSession] = useState<LanStatus | null>(null)
  const [capFor, setCapFor] = useState("")
  const [capDown, setCapDown] = useState("")
  const [capUp, setCapUp] = useState("")

  const nameRank = (src?: string) =>
    !src || src === "-" || src === "vendor" || src === "guess" ? 0 : 1
  // One row per device: the remembered list and a fresh scan can both answer
  // for the same phone, and VPN adapters (Radmin 26/8, APIPA) are not devices
  // on this network at all.
  const clean = (rows: LanDevice[]) => {
    const seen = new Set<string>()
    return rows.filter((d) => {
      if (/^(26[.]|169[.]254[.]|0[.]|127[.])/.test(d.ip)) return false
      const key = d.mac || d.ip
      if (seen.has(key)) return false
      seen.add(key)
      return true
    })
  }

  const applyList = useCallback((next: LanDevice[]) => {
    setList((prev) =>
      clean(next).map((n) => {
        const p = prev.find((x) => x.mac === n.mac)
        if (p && nameRank(p.source) > nameRank(n.source)) {
          return { ...n, name: p.name, source: p.source }
        }
        return n
      }),
    )
  }, [])

  const scan = useCallback(async () => {
    setScanning(true)
    setScanError("")
    try {
      applyList(await api.lanDevices())
    } catch (e) {
      setScanError(typeof e === "string" ? e : String(e))
    } finally {
      setScanning(false)
    }
  }, [applyList])

  useEffect(() => {
    let alive = true
    void api.lanKnown().then((d) => alive && setList(clean(d))).catch(() => undefined)
    void api.lanStatus().then((s) => alive && setSession(s)).catch(() => undefined)
    void api
      .lanMonitorActive()
      .then((w) => alive && setWatching(w))
      .catch(() => undefined)
    void scan()
    return () => {
      alive = false
    }
  }, [scan])

  // One snapshot a second: totals, per-device rates and the session in a single
  // round trip. Rates are deltas between ticks, because the counters are
  // cumulative since the watch began.
  const prev = useRef<{ at: number; rows: Record<string, Rate> }>({ at: 0, rows: {} })
  useEffect(() => {
    if (!watching) {
      setRates({})
      setTotal({ down: 0, up: 0 })
      prev.current = { at: 0, rows: {} }
      return
    }
    let alive = true
    const tick = async () => {
      try {
        const snap = await api.networkSnapshot()
        if (!alive) return
        setWatching(snap.watching)
        const now = Date.now()
        const secs = prev.current.at ? Math.max(0.2, (now - prev.current.at) / 1000) : 0
        const next: Record<string, Rate> = {}
        let down = 0
        let up = 0
        for (const row of snap.traffic) {
          const key = row.mac || row.ip
          const before = prev.current.rows[key]
          const dDown = before ? Math.max(0, row.down - before.down) : 0
          const dUp = before ? Math.max(0, row.up - before.up) : 0
          // bytes per second over the tick, as KB/s
          const r = secs
            ? { down: dDown / secs / 1024, up: dUp / secs / 1024 }
            : { down: 0, up: 0 }
          next[key] = r
          down += r.down
          up += r.up
          prev.current.rows[key] = { down: row.down, up: row.up }
        }
        prev.current.at = now
        setRates(next)
        setTotal({ down, up })
      } catch {
        /* a missed tick is a missed tick */
      }
    }
    void tick()
    const id = window.setInterval(() => void tick(), 1000)
    return () => {
      alive = false
      window.clearInterval(id)
    }
  }, [watching])

  useEffect(() => {
    if (!session) return
    let alive = true
    const id = window.setInterval(() => {
      void api
        .lanStatus()
        .then((s) => alive && setSession(s))
        .catch(() => undefined)
    }, 1000)
    return () => {
      alive = false
      window.clearInterval(id)
    }
  }, [session?.mac])

  const watchAll = useCallback(async () => {
    setBusy("watch")
    setNote("")
    try {
      if (watching) {
        await api.lanMonitorStop()
        setWatching(false)
      } else {
        await api.lanMonitorStart()
        setWatching(true)
      }
    } catch (e) {
      setNote(typeof e === "string" ? e : String(e))
    } finally {
      setBusy("")
    }
  }, [watching])

  const start = useCallback(async (d: LanDevice, blocked: boolean, cap = false) => {
    setBusy(d.mac)
    setNote("")
    try {
      await api.lanStart(d.ip, d.mac, blocked)
      const s = await api.lanStatus().catch(() => null)
      setSession(
        s ?? {
          ip: d.ip,
          mac: d.mac,
          down_kbps: 0,
          up_kbps: 0,
          blocked,
          up_bytes: 0,
          down_bytes: 0,
        },
      )
      setCapFor(cap ? d.mac : "")
      if (cap) {
        setCapDown("")
        setCapUp("")
      }
    } catch (e) {
      setNote(typeof e === "string" ? e : String(e))
    } finally {
      setBusy("")
    }
  }, [])

  const stop = useCallback(async () => {
    setBusy("stop")
    try {
      await api.lanStop()
      setSession(null)
      setCapFor("")
    } finally {
      setBusy("")
    }
  }, [])

  const applyCap = useCallback(async () => {
    const down = Math.max(0, Math.round(Number(capDown) || 0)) * 8
    const up = Math.max(0, Math.round(Number(capUp) || 0)) * 8
    setBusy("cap")
    try {
      await api.lanLimits(down, up)
      const s = await api.lanStatus().catch(() => null)
      if (s) setSession(s)
      setCapFor("")
    } catch (e) {
      setNote(typeof e === "string" ? e : String(e))
    } finally {
      setBusy("")
    }
  }, [capDown, capUp])

  const copy = useCallback(async (text: string) => {
    const ok = await copyText(text).catch(() => false)
    if (!ok) return
    setCopied(text)
    window.setTimeout(() => setCopied(""), 1200)
  }, [])

  const rateOf = (d: LanDevice) => rates[d.mac] ?? rates[d.ip] ?? { down: 0, up: 0 }

  const rows = useMemo(() => {
    const ipNum = (ip: string) =>
      ip.split(".").reduce((n, part) => n * 256 + (Number(part) || 0), 0)
    const q = query.trim().toLowerCase()
    return list
      .filter(
        (d) =>
          !q ||
          d.name.toLowerCase().includes(q) ||
          d.ip.includes(q) ||
          d.mac.toLowerCase().includes(q),
      )
      .slice()
      .sort((a, b) => {
        if (a.self_pc !== b.self_pc) return a.self_pc ? -1 : 1
        if (watching) {
          const ra = rateOf(a)
          const rb = rateOf(b)
          const diff = rb.down + rb.up - (ra.down + ra.up)
          if (Math.abs(diff) > 0.5) return diff
        }
        return ipNum(a.ip) - ipNum(b.ip)
      })
  }, [list, query, watching, rates])

  const focus = useMemo(
    () => (session ? list.find((d) => d.mac === session.mac) : undefined),
    [list, session],
  )

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-3">
      <div className="flex shrink-0 items-end justify-between gap-4">
        <div className="min-w-0">
          <h1 className="text-[19px] font-semibold tracking-[-0.01em] text-txt">
            {t("netTitle")}
          </h1>
          <p className="mt-0.5 truncate text-[12.5px] text-txt2">{t("netSub")}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={() => void watchAll()}
            disabled={busy === "watch"}
            className={cn(
              "press flex h-9 items-center gap-2 rounded-[7px] border px-3 text-[12.5px] transition-colors disabled:opacity-50",
              watching
                ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                : "border-line text-txt2 hover:bg-white/[0.04] hover:text-txt",
            )}
          >
            {busy === "watch" ? (
              <Loader2 className="size-3.5 animate-spin" strokeWidth={2} aria-hidden />
            ) : watching ? (
              <Square className="size-3" strokeWidth={2.4} aria-hidden />
            ) : (
              <Activity className="size-3.5" strokeWidth={2} aria-hidden />
            )}
            {watching ? t("netStopWatch") : t("netStartWatchAll")}
          </button>
          <button
            onClick={() => void scan()}
            disabled={scanning}
            className="press flex h-9 items-center gap-2 rounded-[7px] border border-line px-3 text-[12.5px] text-txt2 transition-colors hover:bg-white/[0.04] hover:text-txt disabled:opacity-50"
          >
            {scanning ? (
              <Loader2 className="size-3.5 animate-spin" strokeWidth={2} aria-hidden />
            ) : (
              <RefreshCw className="size-3.5" strokeWidth={2} aria-hidden />
            )}
            {scanning ? t("netScanning") : t("netRefresh")}
          </button>
        </div>
      </div>

      {session && (
        <section className="shrink-0 rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-5 py-3">
          <div className="flex flex-wrap items-center gap-3">
            <span className="truncate text-[13.5px] font-semibold text-txt">
              {focus?.name && focus.name !== "-" ? focus.name : session.ip}
            </span>
            <Tag brand>{t("netWatching")}</Tag>
            {session.blocked && <Tag danger>{t("netBlocked")}</Tag>}
            <span className="font-mono text-[11.5px] text-txt2">
              {session.ip} · {session.mac.toUpperCase()}
            </span>
            <span className="ml-auto tabular-nums text-[12px] text-txt2">
              <span className="text-txt3">{t("netDown")}</span>{" "}
              {Math.round(session.down_kbps / 8)} KB/s
              <span className="mx-2 text-txt3/40">/</span>
              <span className="text-txt3">{t("netUp")}</span>{" "}
              {Math.round(session.up_kbps / 8)} KB/s
            </span>
            <button
              onClick={() => void stop()}
              disabled={busy === "stop"}
              className="press h-8 shrink-0 rounded-[7px] border border-line px-3 text-[12px] text-txt2 transition-colors hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)] disabled:opacity-50"
            >
              {t("netStopWatch")}
            </button>
          </div>

          <div className="mt-2.5 flex flex-wrap items-center gap-2 border-t border-line/60 pt-2.5">
            <button
              onClick={() => (focus ? void start(focus, !session.blocked) : void stop())}
              disabled={!!busy || !focus}
              className={cn(
                "press flex h-8 items-center gap-1.5 rounded-[7px] border px-3 text-[12px] transition-colors disabled:opacity-50",
                session.blocked
                  ? "border-line text-txt2 hover:bg-white/[0.05]"
                  : "border-[var(--red-line)] text-[var(--red)] hover:bg-[var(--red-bg)]",
              )}
            >
              <Ban className="size-3.5" strokeWidth={2} aria-hidden />
              {session.blocked ? t("netUnblock") : t("netBlock")}
            </button>
            <button
              onClick={() => setCapFor(capFor ? "" : session.mac)}
              className={cn(
                "press flex h-8 items-center gap-1.5 rounded-[7px] border px-3 text-[12px] transition-colors",
                capFor || session.down_kbps > 0 || session.up_kbps > 0
                  ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                  : "border-line text-txt2 hover:bg-white/[0.05]",
              )}
            >
              <Gauge className="size-3.5" strokeWidth={2} aria-hidden />
              {session.down_kbps > 0 || session.up_kbps > 0
                ? `${Math.round(session.down_kbps / 8)} / ${Math.round(session.up_kbps / 8)} KB/s`
                : t("netLimit")}
            </button>
            {capFor === session.mac && (
              <div className="flex flex-wrap items-center gap-2">
                <Field
                  label={t("netLimitDown")}
                  value={capDown}
                  onChange={setCapDown}
                  placeholder="0"
                />
                <Field label={t("netLimitUp")} value={capUp} onChange={setCapUp} placeholder="0" />
                <button
                  onClick={() => void applyCap()}
                  disabled={busy === "cap"}
                  className="press h-8 rounded-[7px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-3 text-[12px] text-brand-strong transition-colors disabled:opacity-50"
                >
                  {t("netLimitApply")}
                </button>
                <button
                  onClick={() => {
                    setCapDown("")
                    setCapUp("")
                    void applyCap()
                  }}
                  className="press h-8 rounded-[7px] border border-line px-3 text-[12px] text-txt2 transition-colors hover:bg-white/[0.05]"
                >
                  {t("netLimitRemove")}
                </button>
              </div>
            )}
            <span className="ml-auto hidden text-[11.5px] text-txt2 lg:block">
              {t("netLimitBody")}
            </span>
          </div>
        </section>
      )}

      {watching && (
        <div className="flex shrink-0 items-center gap-6 rounded-[var(--r-card)] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-5 py-2.5">
          <span className="text-[12px] font-medium text-brand-strong">{t("netWatching")}</span>
          <span className="flex items-center gap-1.5 text-[12.5px] tabular-nums text-txt">
            <ArrowDown className="size-3.5 text-txt3" aria-hidden />
            {total.down.toFixed(1)}
            <span className="text-[11px] text-txt3">KB/s</span>
          </span>
          <span className="flex items-center gap-1.5 text-[12.5px] tabular-nums text-txt">
            <ArrowUp className="size-3.5 text-txt3" aria-hidden />
            {total.up.toFixed(1)}
            <span className="text-[11px] text-txt3">KB/s</span>
          </span>
          <span className="ml-auto text-[11.5px] text-txt2">{t("netAllDevices")}</span>
        </div>
      )}

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <div className="flex shrink-0 items-center gap-3 border-b border-line px-5 py-3">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("netSearchDevices")}
            aria-label={t("netSearchDevices")}
            className="h-9 min-w-0 flex-1 rounded-[7px] border border-line bg-white/[0.02] px-3 text-[12.5px] text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
          />
          {watching && (
            <div className="hidden shrink-0 items-center gap-4 pr-2 text-[11px] font-semibold uppercase tracking-[0.1em] text-txt2 sm:flex">
              <span className="w-[68px] text-right">{t("netDown")}</span>
              <span className="w-[68px] text-right">{t("netUp")}</span>
            </div>
          )}
          <span className="shrink-0 text-[12px] tabular-nums text-txt3" aria-live="polite">
            {scanning ? t("netScanning") : `${rows.length} / ${list.length}`}
          </span>
        </div>

        <div className="app-scroll min-h-0 flex-1 overflow-y-auto overscroll-contain">
          {scanError ? (
            <p className="px-5 py-8 text-[12.5px] text-[var(--red)]">{scanError}</p>
          ) : rows.length === 0 ? (
            <p className="px-5 py-8 text-[12.5px] text-txt3">
              {scanning ? t("netScanning") : query ? t("netNoMatch") : t("netEmpty")}
            </p>
          ) : (
            <ul>
              {rows.map((d) => {
                const locked = d.self_pc || d.protected_always
                const active = session?.mac === d.mac
                const r = rateOf(d)
                return (
                  <li
                    key={`${d.mac}-${d.ip}`}
                    className={cn(
                      "flex items-center gap-4 border-b border-line/60 px-5 py-2.5 transition-colors last:border-b-0",
                      active ? "bg-[var(--brand-bg)]" : "hover:bg-white/[0.02]",
                    )}
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-[13px] font-medium text-txt">
                          {d.name && d.name !== "-" ? d.name : t("netPrivateName")}
                        </span>
                        {d.self_pc && <Tag>{t("netThisPc")}</Tag>}
                        {(d.source === "vendor" || d.source === "guess") && !d.self_pc && (
                          <Tag>{t("netSrcGuess")}</Tag>
                        )}
                        {active && <Tag brand>{t("netWatching")}</Tag>}
                        {active && session?.blocked && <Tag danger>{t("netBlocked")}</Tag>}
                      </div>
                      <div className="mt-0.5 flex items-center gap-2 text-[11.5px] text-txt2">
                        <button
                          onClick={() => void copy(d.ip)}
                          className="press font-mono tabular-nums transition-colors hover:text-txt2"
                          aria-label={t("netCopyIp")}
                        >
                          {d.ip}
                        </button>
                        <span className="text-txt3/40">·</span>
                        <button
                          onClick={() => void copy(d.mac.toUpperCase())}
                          className="press truncate font-mono transition-colors hover:text-txt2"
                          aria-label={t("netCopyMac")}
                        >
                          {d.mac.toUpperCase()}
                        </button>
                        {copied === d.ip || copied === d.mac.toUpperCase() ? (
                          <span className="inline-flex items-center gap-1 text-brand-strong">
                            <Copy className="size-3" aria-hidden />
                            {t("copied")}
                          </span>
                        ) : null}
                      </div>
                    </div>

                    {watching && (
                      <div className="hidden shrink-0 items-center gap-4 sm:flex">
                        <span className="w-[68px] text-right font-mono text-[12px] tabular-nums text-txt">
                          {r.down > 0.05 ? r.down.toFixed(1) : "0"}
                        </span>
                        <span className="w-[68px] text-right font-mono text-[12px] tabular-nums text-txt2">
                          {r.up > 0.05 ? r.up.toFixed(1) : "0"}
                        </span>
                      </div>
                    )}

                    {locked ? (
                      <span className="shrink-0 text-[11.5px] text-txt2">
                        {t("netProtected")}
                      </span>
                    ) : (
                      <div className="flex shrink-0 items-center gap-1.5">
                        <button
                          onClick={() => void start(d, false)}
                          disabled={!!busy}
                          className="press flex h-7 items-center gap-1 rounded-[6px] border border-[var(--brand-line)] px-2.5 text-[11.5px] text-brand-strong transition-colors hover:bg-[var(--brand-bg)] disabled:opacity-40"
                        >
                          <Activity className="size-3" strokeWidth={2} aria-hidden />
                          {t("netMonitor")}
                        </button>
                        <button
                          onClick={() => void start(d, false, true)}
                          disabled={!!busy}
                          className="press flex h-7 items-center gap-1 rounded-[6px] border border-line px-2.5 text-[11.5px] text-txt2 transition-colors hover:bg-white/[0.05] disabled:opacity-40"
                        >
                          <Gauge className="size-3" strokeWidth={2} aria-hidden />
                          {t("netLimit")}
                        </button>
                        <button
                          onClick={() => void start(d, true)}
                          disabled={!!busy}
                          aria-label={t("netBlock")}
                          title={t("netBlock")}
                          className="press grid size-7 place-items-center rounded-[6px] border border-line text-txt3 transition-colors hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)] disabled:opacity-40"
                        >
                          <Ban className="size-3.5" strokeWidth={2} aria-hidden />
                        </button>
                      </div>
                    )}
                  </li>
                )
              })}
            </ul>
          )}
        </div>
      </div>

      {note ? (
        <div
          className="shrink-0 rounded-[var(--r-card)] border border-[var(--red-line)] bg-[var(--red-bg)] px-5 py-2.5"
          role="alert"
        >
          <p className="text-[12.5px] text-[var(--red)]">{note}</p>
        </div>
      ) : (
        <p className="shrink-0 px-1 text-[11.5px] text-txt2">{t("netScanNote")}</p>
      )}
    </div>
  )
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
}) {
  return (
    <label className="flex h-8 items-center gap-1.5 rounded-[7px] border border-line bg-white/[0.02] px-2">
      <input
        value={value}
        onChange={(e) => onChange(e.target.value.replace(/[^0-9]/g, ""))}
        placeholder={placeholder}
        inputMode="numeric"
        className="w-11 bg-transparent text-right font-mono text-[12px] tabular-nums text-txt outline-none placeholder:text-txt3"
      />
      <span className="shrink-0 text-[11px] text-txt3">{label}</span>
    </label>
  )
}

function Tag({
  children,
  brand,
  danger,
}: {
  children: React.ReactNode
  brand?: boolean
  danger?: boolean
}) {
  return (
    <span
      className={cn(
        "shrink-0 rounded-full border px-2 py-px text-[11px] leading-4",
        danger
          ? "border-[var(--red-line)] bg-[var(--red-bg)] text-[var(--red)]"
          : brand
            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
            : "border-line text-txt3",
      )}
    >
      {children}
    </span>
  )
}
