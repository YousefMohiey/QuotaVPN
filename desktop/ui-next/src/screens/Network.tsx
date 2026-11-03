import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Activity,
  Ban,
  Check,
  ChevronRight,
  Gauge,
  Info,
  LayoutGrid,
  Loader2,
  Monitor,
  MoreVertical,
  RefreshCw,
  Router,
  Search,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  Square,
  Wifi,
  X,
} from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { api, type LanDevice, type LanGuard, type LanStatus } from "@/lib/ipc"
import { cn } from "@/lib/utils"
import { copyText } from "@/lib/clipboard"
import { LanLimitDialog } from "@/components/LanLimitDialog"
import { RouterLinkDialog } from "@/components/RouterLinkDialog"

/** Bytes per second as something a person reads at a glance. */
function fmtRate(bytes: number): string {
  if (bytes < 1024) return `${bytes} B/s`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB/s`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB/s`
}

type Live = { up: number; down: number }
type Filter = "all" | "protected" | "router" | "announced"
type Sort = "ip" | "traffic"

/** Who is on this network, what they are moving, and what can be done to them. */
export function Network() {
  const { t } = useI18n()
  const [devices, setDevices] = useState<LanDevice[]>([])
  const [busy, setBusy] = useState(true)
  const [failed, setFailed] = useState(false)
  const [active, setActive] = useState<LanStatus | null>(null)
  const [limitFor, setLimitFor] = useState<LanDevice | null>(null)
  const [err, setErr] = useState("")
  const [guard, setGuard] = useState<LanGuard | null>(null)
  const [routerOpen, setRouterOpen] = useState(false)
  const [routerLinked, setRouterLinked] = useState(false)
  const [driverReady, setDriverReady] = useState<boolean | null>(null)
  const [watching, setWatching] = useState(false)
  const [watchBusy, setWatchBusy] = useState(false)
  const [live, setLive] = useState<Record<string, Live>>({})
  const [filter, setFilter] = useState<Filter>("all")
  const [query, setQuery] = useState("")
  const [sort, setSort] = useState<Sort>("ip")
  const [menu, setMenu] = useState<{ ip: string; x: number; y: number } | null>(null)
  const [copied, setCopied] = useState<string | null>(null)
  const prevTraffic = useRef<Record<string, { up: number; down: number; at: number }>>({})
  const polled = useRef(0)

  const scan = useCallback(async (quiet = false) => {
    if (!quiet) setBusy(true)
    setFailed(false)
    try {
      const list = await api.lanDevices()
      setDevices(Array.isArray(list) ? list : [])
      // A missing name is being looked up in the background, so ask once more
      // shortly rather than making the user press Refresh again.
      if (!quiet && list.some((d) => d.name === "-") && polled.current < 2) {
        polled.current += 1
        window.setTimeout(() => void scan(true), 4000)
      }
    } catch {
      if (!quiet) setFailed(true)
    } finally {
      if (!quiet) setBusy(false)
    }
  }, [])

  const refreshStatus = useCallback(async () => {
    try {
      setActive(await api.lanStatus())
    } catch {
      setActive(null)
    }
  }, [])

  /** One sample of every device's counters, turned into a rate. */
  const sampleTraffic = useCallback(async () => {
    try {
      const rows = await api.lanTraffic()
      if (!Array.isArray(rows)) {
        setLive({})
        return
      }
      const now = Date.now()
      const next: Record<string, Live> = {}
      for (const row of rows) {
        const before = prevTraffic.current[row.ip]
        if (before && now > before.at) {
          const seconds = (now - before.at) / 1000
          next[row.ip] = {
            up: Math.max(0, Math.round((row.up - before.up) / seconds)),
            down: Math.max(0, Math.round((row.down - before.down) / seconds)),
          }
        } else {
          next[row.ip] = { up: 0, down: 0 }
        }
        prevTraffic.current[row.ip] = { up: row.up, down: row.down, at: now }
      }
      setLive(next)
    } catch {
      setLive({})
    }
  }, [])

  const stopAll = async () => {
    setErr("")
    try {
      await api.lanStop()
      setActive(null)
    } catch (e) {
      setErr(String(e))
    }
  }

  const toggleBlock = async (d: LanDevice) => {
    setErr("")
    setMenu(null)
    try {
      if (active?.ip === d.ip) {
        await stopAll()
        return
      }
      await api.lanStart(d.ip, d.mac, true)
      await refreshStatus()
    } catch (e) {
      setErr(String(e))
    }
  }

  const toggleProtect = async (d: LanDevice) => {
    setErr("")
    setMenu(null)
    try {
      await api.lanProtect(d.mac, !d.protected)
      await scan(true)
    } catch (e) {
      setErr(String(e))
    }
  }

  const toggleWatch = async () => {
    setErr("")
    setWatchBusy(true)
    try {
      if (watching) {
        await api.lanMonitorStop()
        setWatching(false)
        setLive({})
        prevTraffic.current = {}
      } else {
        await api.lanMonitorStart()
        prevTraffic.current = {}
        setWatching(true)
      }
    } catch (e) {
      setErr(String(e))
    } finally {
      setWatchBusy(false)
    }
  }

  const copy = async (text: string, key?: string) => {
    setMenu(null)
    // The native clipboard first: the engine's async API is refused in some
    // builds, and a copy that silently does nothing is worse than none.
    let ok = false
    try {
      ok = await api.copyToClipboard(text)
    } catch {
      ok = await copyText(text)
    }
    if (!ok) {
      setErr(t("netCopyFailed"))
      return
    }
    if (key) {
      setCopied(key)
      window.setTimeout(() => setCopied((c) => (c === key ? null : c)), 1400)
    }
  }

  useEffect(() => {
    let alive = true
    void api
      .lanKnown()
      .then((list) => {
        if (alive && Array.isArray(list) && list.length) setDevices(list)
      })
      .catch(() => undefined)
      .finally(() => {
        if (alive) {
          void scan()
          void refreshStatus()
          void api.lanMonitorActive().then(setWatching).catch(() => undefined)
        }
      })

    const tick = window.setInterval(() => {
      void api.lanGuard().then(setGuard).catch(() => undefined)
      void api.npcapReady().then(setDriverReady).catch(() => setDriverReady(null))
      void api
        .routerStatus()
        .then(async (linked) => {
          if (linked) {
            setRouterLinked(true)
            return
          }
          const ok = await api.routerAutoLink().catch(() => false)
          if (ok) {
            setRouterLinked(true)
            void scan(true)
          }
        })
        .catch(() => undefined)
      void api
        .lanMonitorActive()
        .then((on) => {
          setWatching(on)
          if (on) void sampleTraffic()
          else setLive({})
        })
        .catch(() => undefined)
    }, 1500)

    void api.lanGuard().then(setGuard).catch(() => undefined)
    void api.npcapReady().then(setDriverReady).catch(() => setDriverReady(null))
    void api
      .routerStatus()
      .then(async (linked) => {
        if (linked) {
          setRouterLinked(true)
          return
        }
        // A saved sign in means the names come back on their own.
        const ok = await api.routerAutoLink().catch(() => false)
        if (ok) {
          setRouterLinked(true)
          void scan(true)
        }
      })
      .catch(() => undefined)

    return () => {
      alive = false
      window.clearInterval(tick)
    }
  }, [scan, refreshStatus, sampleTraffic])

  const stateOf = (d: LanDevice): "blocked" | "capped" | "none" => {
    if (!active || active.ip !== d.ip) return "none"
    if (active.blocked) return "blocked"
    if (active.down_kbps > 0 || active.up_kbps > 0) return "capped"
    return "none"
  }

  const counts = useMemo(() => {
    const isProtected = (d: LanDevice) => d.protected || d.protected_always
    return {
      all: devices.length,
      protected: devices.filter(isProtected).length,
      announced: devices.filter((d) => d.source === "device").length,
      router: devices.filter((d) => d.source === "router").length,
    }
  }, [devices])

  const shown = useMemo(() => {
    const isProtected = (d: LanDevice) => d.protected || d.protected_always
    const q = query.trim().toLowerCase()
    let list = devices.filter((d) => {
      if (filter === "protected" && !isProtected(d)) return false
      if (filter === "announced" && d.source !== "device") return false
      if (filter === "router" && d.source !== "router") return false
      if (!q) return true
      return (
        d.name.toLowerCase().includes(q) ||
        d.ip.includes(q) ||
        d.mac.toLowerCase().includes(q)
      )
    })
    if (sort === "traffic") {
      const score = (d: LanDevice) => (live[d.ip]?.down ?? 0) + (live[d.ip]?.up ?? 0)
      list = [...list].sort((a, b) => score(b) - score(a))
    }
    return list
  }, [devices, filter, query, sort, live])

  // The gateway is the device the scan marks as always protected, and it is the
  // one row that is also the router itself.
  const gateway = devices.find((d) => d.protected_always && !d.self_pc)
  const gatewayLabel = (() => {
    if (!gateway || !gateway.name || gateway.name === "-") return t("netRouter")
    // A router that names itself should not be repeated inside a label.
    return /router/i.test(gateway.name)
      ? gateway.name
      : `${t("netRouter")} (${gateway.name.replace(/\s*device$/i, "")})`
  })()

  // One bar per device, measured against the busiest on the list, so a glance
  // answers who is using the connection.
  const busiest = Math.max(
    1,
    ...shown.map((d) => (live[d.ip]?.down ?? 0) + (live[d.ip]?.up ?? 0)),
  )
  const totalDown = Object.values(live).reduce((sum, v) => sum + v.down, 0)
  const totalUp = Object.values(live).reduce((sum, v) => sum + v.up, 0)

  const filters: { id: Filter; label: string; icon: typeof LayoutGrid; n: number }[] = [
    { id: "all", label: t("netAllDevices"), icon: LayoutGrid, n: counts.all },
    { id: "protected", label: t("netProtected"), icon: ShieldCheck, n: counts.protected },
    { id: "router", label: t("netRouter"), icon: Router, n: counts.router },
    { id: "announced", label: t("netAnnounced"), icon: Wifi, n: counts.announced },
  ]

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      {/* Who is on the network, and the one control that decides whether we know. */}
      <header className="flex shrink-0 items-center gap-4">
        <div className="min-w-0">
          <h1 className="truncate text-[21px] font-semibold leading-tight text-txt">{t("netTitle")}</h1>
          <p className="mt-1 flex items-center gap-2 text-[12.5px] text-txt2">
            {watching ? (
              <span className="net-live-dot size-1.5 shrink-0 rounded-full bg-[var(--green)]" aria-hidden />
            ) : null}
            <span className="truncate">{watching ? t("netWatching") : t("netIdle")}</span>
          </p>
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-2.5">
          <button
            type="button"
            onClick={() => setRouterOpen(true)}
            className={cn(
              "press flex h-10 items-center gap-2 rounded-[10px] border px-3.5 text-[13px] transition-colors",
              routerLinked
                ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                : "border-[var(--line-strong)] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
            )}
          >
            <Router className="size-4" aria-hidden />
            {routerLinked ? t("routerLinked") : t("routerNames")}
          </button>
          <button
            type="button"
            onClick={() => void scan()}
            disabled={busy}
            title={t("netRefresh")}
            aria-label={t("netRefresh")}
            className="press grid size-10 place-items-center rounded-[10px] border border-[var(--line-strong)] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <RefreshCw className="size-4" aria-hidden />}
          </button>
          <button
            type="button"
            onClick={() => void toggleWatch()}
            disabled={watchBusy || driverReady === false}
            className={cn(
              "press flex h-10 items-center gap-2 rounded-[10px] border px-4 text-[13.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45",
              watching
                ? "border-[var(--green-line)] bg-[var(--green-bg)] text-[var(--green)]"
                : "border-[var(--brand)] bg-[var(--brand)] text-white hover:brightness-110",
            )}
          >
            {watchBusy ? (
              <Loader2 className="size-4 animate-spin" aria-hidden />
            ) : watching ? (
              <Square className="size-3.5" aria-hidden />
            ) : (
              <Activity className="size-4" aria-hidden />
            )}
            {watching ? t("netStopWatch") : t("netStartWatch")}
          </button>
        </div>
      </header>

      {/* The four numbers that describe the network at a glance. */}
      <div className="grid shrink-0 grid-cols-4 divide-x divide-[var(--line)] overflow-hidden rounded-[var(--r-card)] border border-[var(--line)] bg-[var(--panel)]">
        <div className="px-5 py-3.5">
          <div className="text-[11px] uppercase tracking-[0.09em] text-txt3">{t("netDevices")}</div>
          <div className="mt-1 text-[23px] font-semibold leading-none text-txt tabular-nums">{counts.all}</div>
        </div>
        <div className="px-5 py-3.5">
          <div className="text-[11px] uppercase tracking-[0.09em] text-txt3">{t("netDownload")}</div>
          <div className="mt-1 text-[23px] font-semibold leading-none text-[var(--green)] tabular-nums">
            <span key={watching ? "on" : "off"} className="net-list-in">{watching ? fmtRate(totalDown) : "-"}</span>
          </div>
        </div>
        <div className="px-5 py-3.5">
          <div className="text-[11px] uppercase tracking-[0.09em] text-txt3">{t("netUpload")}</div>
          <div className="mt-1 text-[23px] font-semibold leading-none text-[var(--brand-strong)] tabular-nums">
            <span key={watching ? "on" : "off"} className="net-list-in">{watching ? fmtRate(totalUp) : "-"}</span>
          </div>
        </div>
        <button
          type="button"
          onClick={() => setRouterOpen(true)}
          className="press flex items-center gap-3 px-5 py-3.5 text-left transition-colors hover:bg-white/[0.02]"
        >
          <div className="min-w-0 flex-1">
            <div className="truncate text-[11px] uppercase tracking-[0.09em] text-txt3">{gatewayLabel}</div>
            <div className="mt-1 truncate text-[17px] font-semibold leading-none text-txt tabular-nums" dir="ltr">
              {gateway?.ip ?? "-"}
            </div>
          </div>
          <span className="relative grid size-2 shrink-0 place-items-center">
            {routerLinked ? <span className="size-2 rounded-full bg-[var(--green)]" /> : null}
          </span>
          <ChevronRight className="size-4 shrink-0 text-txt3" aria-hidden />
        </button>
      </div>

      {driverReady === false ? (
        <div className="flex shrink-0 items-center gap-3 rounded-[var(--r-card)] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-4 py-2.5">
          <Info className="size-4 shrink-0 text-[var(--brand-strong)]" aria-hidden />
          <span className="min-w-0 flex-1 text-[12.5px] text-txt2" dir="auto">{t("netDriverBody")}</span>
          <button
            type="button"
            onClick={() => void api.openPage("https://npcap.com/#download")}
            className="press flex h-8 shrink-0 items-center rounded-[9px] border border-[var(--brand-line)] px-2.5 text-[12px] text-[var(--brand-strong)]"
          >
            {t("netDriverGet")}
          </button>
        </div>
      ) : null}

      {guard && guard.attacker_mac ? (
        <div className="flex shrink-0 items-center gap-3 rounded-[var(--r-card)] border border-[var(--red-line)] bg-[var(--red-bg)] px-4 py-2.5">
          <ShieldAlert className="size-4 shrink-0 text-[var(--red)]" aria-hidden />
          <span className="min-w-0 flex-1 text-[12.5px] text-[var(--red)]" dir="auto">
            {t("netGuardAlert").replace("{mac}", guard.attacker_mac)}
          </span>
        </div>
      ) : null}

      {active ? (
        <div className="flex shrink-0 items-center gap-3 rounded-[var(--r-card)] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-4 py-2.5">
          <span className="shrink-0 font-mono text-[12px] text-txt" dir="ltr">{active.ip}</span>
          <span className="min-w-0 flex-1 truncate text-[12.5px] text-txt2">
            {active.blocked
              ? t("netActiveBlocked")
              : t("netActiveCapped")
                  .replace("{down}", String(Number(active.down_kbps) || 0))
                  .replace("{up}", String(Number(active.up_kbps) || 0))}
          </span>
          <button
            type="button"
            onClick={() => void stopAll()}
            className="press flex h-8 shrink-0 items-center gap-1.5 rounded-[9px] border border-[var(--line-strong)] px-2.5 text-[12px] text-txt2 hover:text-txt"
          >
            <X className="size-3.5" aria-hidden />
            {t("netRemove")}
          </button>
        </div>
      ) : null}

      {err ? (
        <div className="shrink-0 rounded-[var(--r-card)] border border-[var(--red-line)] bg-[var(--red-bg)] px-4 py-2 text-[12px] text-[var(--red)]" dir="auto">
          {err}
        </div>
      ) : null}

      {/* Narrowing the list. */}
      <div className="flex shrink-0 items-center gap-2">
        {filters.map((f) => {
          const on = filter === f.id
          return (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              className={cn(
                "press flex h-8 items-center gap-2 rounded-[8px] px-3 text-[12.5px] transition-colors",
                on
                  ? "bg-[var(--brand-bg)] text-txt shadow-[inset_0_0_0_1px_var(--brand-line)]"
                  : "bg-white/[0.03] text-txt2 hover:bg-white/[0.05] hover:text-txt",
              )}
            >
              <f.icon className="size-[14px]" aria-hidden />
              {f.label}
              <span
                className={cn(
                  "rounded-[5px] px-1.5 text-[11px] tabular-nums",
                  on ? "bg-[var(--brand-mid)] text-white" : "bg-white/[0.06] text-txt3",
                )}
              >
                {f.n}
              </span>
            </button>
          )
        })}
        <div className="ml-auto flex items-center gap-2">
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-[14px] -translate-y-1/2 text-txt3" aria-hidden />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("netSearchDevices")}
              className="h-8 w-[260px] rounded-[8px] border border-[var(--line)] bg-[var(--field)] pl-8 pr-3 text-[12.5px] text-txt outline-none placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
          </div>
          <button
            type="button"
            onClick={() => setSort(sort === "ip" ? "traffic" : "ip")}
            title={sort === "ip" ? t("netSortBusy") : t("netSortIp")}
            aria-label={sort === "ip" ? t("netSortBusy") : t("netSortIp")}
            className={cn(
              "press grid size-8 place-items-center rounded-[8px] border transition-colors",
              sort === "traffic"
                ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                : "border-[var(--line)] bg-[var(--field)] text-txt2 hover:text-txt",
            )}
          >
            <SlidersHorizontal className="size-[14px]" aria-hidden />
          </button>
        </div>
      </div>

      {/* Each device is its own card. Columns, not a grid: the count changes
          with the network, and a grid would leave empty slots on the last row. */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        {busy && devices.length === 0 ? (
          <div className="grid grid-cols-3 content-start gap-3.5" aria-hidden>
            {[0, 1, 2, 3, 4, 5].map((i) => (
              <div
                key={i}
                className="rounded-[var(--r-card)] border border-[var(--line)] bg-[var(--panel)] p-4"
              >
                <div className="flex items-center gap-3">
                  <span className="net-wait size-10 rounded-full bg-white/[0.05]" />
                  <span className="net-wait h-3 w-28 rounded bg-white/[0.05]" />
                </div>
                <span className="net-wait mt-4 block h-3 w-20 rounded bg-white/[0.05]" />
                <span className="net-wait mt-4 block h-5 w-24 rounded bg-white/[0.05]" />
              </div>
            ))}
          </div>
        ) : failed ? (
          <div className="flex h-full flex-col items-center justify-center gap-3">
            <ShieldAlert className="size-5 text-txt3" aria-hidden />
            <span className="text-[13px] text-txt3">{t("netFail")}</span>
            <button
              type="button"
              onClick={() => void scan()}
              className="press h-8 rounded-[8px] border border-[var(--line-strong)] px-3 text-[12px] text-txt2 transition-colors hover:text-txt"
            >
              {t("netRefresh")}
            </button>
          </div>
        ) : shown.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2">
            <Wifi className="size-5 text-txt3" aria-hidden />
            <span className="text-[13px] text-txt3">
              {devices.length === 0 ? t("netEmpty") : t("netNoMatch")}
            </span>
          </div>
        ) : (
          <div key={`${filter}|${query}|${sort}`} className="net-list-in grid grid-cols-3 content-start gap-3.5">
            {shown.map((d) => {
              const state = stateOf(d)
              const rate = live[d.ip]
              const protectedRow = d.protected || d.protected_always
              const total = (rate?.down ?? 0) + (rate?.up ?? 0)
              return (
                <div
                  key={d.ip}
                  className="net-row flex flex-col rounded-[var(--r-card)] border border-[var(--line)] bg-[var(--panel)] p-4 hover:bg-[var(--panel-hover)]"
                >
                  <div className="flex items-center gap-3">
                    <span
                      className={cn(
                        "grid size-10 shrink-0 place-items-center rounded-full border",
                        d.self_pc
                          ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                          : "border-[var(--line)] text-txt2",
                      )}
                    >
                      {d.self_pc ? <Monitor className="size-4" aria-hidden /> : <Wifi className="size-4" aria-hidden />}
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[14px] font-semibold text-txt" dir="auto">
                        {d.name === "private" ? t("netPrivateName") : d.name}
                      </div>
                      <button
                        type="button"
                        onClick={() => void copy(d.ip, `ip-${d.ip}`)}
                        title={t("netCopyIp")}
                        className="group flex items-center gap-1.5 rounded-[6px] font-mono text-[11.5px] text-txt2 transition-colors hover:text-txt"
                      >
                        <span dir="ltr">{d.ip}</span>
                        {copied === `ip-${d.ip}` ? (
                          <Check className="net-check-in size-3 text-[var(--green)]" aria-hidden />
                        ) : null}
                      </button>
                    </div>
                  </div>

                  <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
                    {d.self_pc ? <Badge tone="brand">{t("netThisPc")}</Badge> : null}
                    {protectedRow ? <Badge tone="green" icon={ShieldCheck}>{t("netProtected")}</Badge> : null}
                    {d.source === "device" ? <Badge>{t("netAnnounced")}</Badge> : null}
                    {d.source === "vendor" ? <Badge>{t("netHardware")}</Badge> : null}
                    {state === "blocked" ? <Badge tone="red">{t("netStateBlocked")}</Badge> : null}
                    {state === "capped" ? <Badge tone="brand">{t("netStateCapped")}</Badge> : null}
                  </div>

                  <button
                    type="button"
                    onClick={() => void copy(d.mac.toUpperCase(), `mac-${d.ip}`)}
                    disabled={!d.mac}
                    title={t("netCopyMac")}
                    className="group mt-2 flex max-w-full items-center gap-1.5 rounded-[6px] font-mono text-[11px] uppercase text-txt3 transition-colors hover:text-txt2 disabled:cursor-default"
                  >
                    <span className="truncate" dir="ltr">{d.mac || "-"}</span>
                    {copied === `mac-${d.ip}` ? (
                      <Check className="net-check-in size-3 shrink-0 text-[var(--green)]" aria-hidden />
                    ) : null}
                  </button>

                  <div className="mt-3.5 flex items-start gap-5" dir="ltr">
                    <div className="min-w-0 flex-1">
                      <div className="text-[10px] uppercase tracking-[0.08em] text-txt3">{t("netDownload")}</div>
                      <div className="mt-0.5 truncate font-mono text-[17px] font-semibold text-[var(--green)] tabular-nums">
                        {watching && rate ? fmtRate(rate.down) : "-"}
                      </div>
                      {watching && rate ? (
                        <div className="mt-1.5 h-[3px] w-full overflow-hidden rounded-full bg-white/[0.05]" aria-hidden>
                          <div
                            className="net-bar h-full origin-left rounded-full bg-white/25"
                            style={{ transform: `scaleX(${Math.min(1, total / busiest)})` }}
                          />
                        </div>
                      ) : null}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="text-[10px] uppercase tracking-[0.08em] text-txt3">{t("netUpload")}</div>
                      <div className="mt-0.5 truncate font-mono text-[17px] font-semibold text-[var(--brand-strong)] tabular-nums">
                        {watching && rate ? fmtRate(rate.up) : "-"}
                      </div>
                    </div>
                  </div>

                  <div className="mt-auto flex items-center gap-1.5 border-t border-[var(--line)] pt-2.5">
                    {d.protected_always ? (
                      <span className="size-8" />
                    ) : (
                      <button
                        type="button"
                        onClick={() => void toggleProtect(d)}
                        title={d.protected ? t("netUnprotect") : t("netProtect")}
                        aria-label={d.protected ? t("netUnprotect") : t("netProtect")}
                        className={cn(
                          "press grid size-8 place-items-center rounded-[8px] border transition-colors",
                          d.protected
                            ? "border-[var(--green-line)] bg-[var(--green-bg)] text-[var(--green)]"
                            : "border-[var(--line-strong)] text-txt2 hover:border-[var(--green-line)] hover:bg-[var(--green-bg)] hover:text-txt",
                        )}
                      >
                        <ShieldCheck className="size-[15px]" aria-hidden />
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => setLimitFor(d)}
                      disabled={driverReady === false || protectedRow}
                      className={cn(
                        "press flex h-8 items-center gap-1.5 rounded-[8px] border px-2.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                        state === "capped"
                          ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                          : "border-[var(--line-strong)] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
                      )}
                    >
                      <Gauge className="size-[14px]" aria-hidden />
                      {t("netLimit")}
                    </button>
                    <button
                      type="button"
                      onClick={() => void toggleBlock(d)}
                      disabled={driverReady === false || protectedRow}
                      className={cn(
                        "press flex h-8 items-center gap-1.5 rounded-[8px] border px-2.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                        state === "blocked"
                          ? "border-[var(--red-line)] bg-[var(--red-bg)] text-[var(--red)]"
                          : "border-[var(--line-strong)] text-txt2 hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-txt",
                      )}
                    >
                      <Ban className="size-[14px]" aria-hidden />
                      {state === "blocked" ? t("netUnblock") : t("netBlock")}
                    </button>
                    <button
                      type="button"
                      onClick={(e) => {
                        const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
                        setMenu(menu?.ip === d.ip ? null : { ip: d.ip, x: r.right, y: r.bottom })
                      }}
                      title={t("netMore")}
                      aria-label={t("netMore")}
                      className="press ml-auto grid size-8 place-items-center rounded-[8px] border border-[var(--line-strong)] text-txt2 transition-colors hover:bg-white/[0.04] hover:text-txt"
                    >
                      <MoreVertical className="size-[15px]" aria-hidden />
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      {menu ? (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setMenu(null)} aria-hidden />
          <div
            className="net-menu-in fixed z-50 min-w-[168px] overflow-hidden rounded-[10px] border border-[var(--line-strong)] bg-[var(--panel-hover)] py-1 shadow-[var(--sh-2)]"
            style={{ left: Math.max(12, menu.x - 168), top: menu.y + 6 }}
          >
            <button
              type="button"
              onClick={() => void copy(menu.ip)}
              className="block w-full px-3 py-2 text-left text-[12.5px] text-txt2 hover:bg-white/[0.05] hover:text-txt"
            >
              {t("netCopyIp")}
            </button>
            <button
              type="button"
              onClick={() => void copy(devices.find((x) => x.ip === menu.ip)?.mac.toUpperCase() ?? "")}
              className="block w-full px-3 py-2 text-left text-[12.5px] text-txt2 hover:bg-white/[0.05] hover:text-txt"
            >
              {t("netCopyMac")}
            </button>
          </div>
        </>
      ) : null}

      <RouterLinkDialog
        open={routerOpen}
        onOpenChange={setRouterOpen}
        onLinked={() => {
          setRouterLinked(true)
          void scan()
        }}
      />

      <LanLimitDialog
        device={limitFor}
        current={active && limitFor && active.ip === limitFor.ip ? active : null}
        open={limitFor !== null}
        onOpenChange={(o) => {
          if (!o) setLimitFor(null)
        }}
        onApplied={() => void refreshStatus()}
        onRemoved={() => void refreshStatus()}
      />
    </div>
  )
}

function Badge({
  children,
  tone = "quiet",
  icon: Icon,
}: {
  children: React.ReactNode
  tone?: "quiet" | "brand" | "green" | "red"
  icon?: typeof ShieldCheck
}) {
  const tones = {
    quiet: "border-[var(--line-strong)] text-txt3",
    brand: "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]",
    green: "border-[var(--green-line)] bg-[var(--green-bg)] text-[var(--green)]",
    red: "border-[var(--red-line)] bg-[var(--red-bg)] text-[var(--red)]",
  }
  return (
    <span
      className={cn(
        "flex shrink-0 items-center gap-1 rounded-[5px] border px-1.5 py-[1px] text-[11px] font-medium",
        tones[tone],
      )}
    >
      {Icon ? <Icon className="size-3" aria-hidden /> : null}
      {children}
    </span>
  )
}
