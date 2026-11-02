import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Ban,
  ChevronRight,
  Cpu,
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
import { LanLimitDialog } from "@/components/LanLimitDialog"
import { RouterLinkDialog } from "@/components/RouterLinkDialog"

/** Bytes per second as something a person reads at a glance. */
function fmtRate(bytes: number): string {
  if (bytes < 1024) return `${bytes} B/s`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB/s`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB/s`
}

type Live = { up: number; down: number }
type Filter = "all" | "protected" | "announced" | "hardware"
type Sort = "ip" | "traffic"

/** The column grid, shared by the header row and every device row so the two
 *  can never drift apart. */
const GRID =
  "grid grid-cols-[minmax(0,1fr)_88px_122px_118px_208px] items-center gap-3"

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

  const copy = async (text: string) => {
    setMenu(null)
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      setErr(t("netCopyFailed"))
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
      hardware: devices.filter((d) => d.source === "vendor").length,
    }
  }, [devices])

  const shown = useMemo(() => {
    const isProtected = (d: LanDevice) => d.protected || d.protected_always
    const q = query.trim().toLowerCase()
    let list = devices.filter((d) => {
      if (filter === "protected" && !isProtected(d)) return false
      if (filter === "announced" && d.source !== "device") return false
      if (filter === "hardware" && d.source !== "vendor") return false
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
  const gatewayLabel = gateway
    ? `${t("netRouter")} (${gateway.name.replace(/\s*device$/i, "")})`
    : t("netRouter")

  const totalDown = Object.values(live).reduce((sum, v) => sum + v.down, 0)
  const totalUp = Object.values(live).reduce((sum, v) => sum + v.up, 0)

  const filters: { id: Filter; label: string; icon: typeof LayoutGrid; n: number }[] = [
    { id: "all", label: t("netAllDevices"), icon: LayoutGrid, n: counts.all },
    { id: "protected", label: t("netProtected"), icon: ShieldCheck, n: counts.protected },
    { id: "announced", label: t("netAnnounced"), icon: Wifi, n: counts.announced },
    { id: "hardware", label: t("netHardware"), icon: Cpu, n: counts.hardware },
  ]

  return (
    <div className="flex h-full min-h-0 flex-col gap-5">
      {/* Title and the one control the screen exists for. */}
      <header className="flex shrink-0 items-center gap-4">
        <span
          className={cn(
            "grid size-12 shrink-0 place-items-center rounded-[13px] border",
            watching
              ? "border-[var(--green-line)] bg-[var(--green-bg)] text-[var(--green)]"
              : "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]",
          )}
        >
          <Activity className="size-[22px]" aria-hidden />
        </span>
        <div className="min-w-0">
          <h1 className="truncate text-[22px] font-semibold leading-tight text-txt">{t("netTitle")}</h1>
          <p className="mt-0.5 truncate text-[13px] text-txt2">
            {watching ? t("netWatching") : t("netIdle")}
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
                : "border-[var(--line-strong)] bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
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
            className="press grid size-10 place-items-center rounded-[10px] border border-[var(--line-strong)] bg-white/[0.02] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait disabled:opacity-60"
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
      <div className="grid shrink-0 grid-cols-4 divide-x divide-[var(--line)] rounded-[var(--r-card)] border border-[var(--line)] bg-[var(--panel)]">
        <div className="flex items-center gap-3.5 px-5 py-4">
          <span className="grid size-9 shrink-0 place-items-center rounded-[10px] bg-white/[0.04] text-txt2">
            <Monitor className="size-4" aria-hidden />
          </span>
          <div className="min-w-0">
            <div className="text-[19px] font-semibold leading-tight text-txt tabular-nums">{counts.all}</div>
            <div className="truncate text-[12px] text-txt3">{t("netDevices")}</div>
          </div>
        </div>
        <div className="flex items-center gap-3.5 px-5 py-4">
          <ArrowDown className="size-[18px] shrink-0 text-[var(--green)]" aria-hidden />
          <div className="min-w-0">
            <div className="truncate text-[19px] font-semibold leading-tight text-txt tabular-nums">
              {watching ? fmtRate(totalDown) : "-"}
            </div>
            <div className="truncate text-[12px] text-txt3">{t("netDownload")}</div>
          </div>
        </div>
        <div className="flex items-center gap-3.5 px-5 py-4">
          <ArrowUp className="size-[18px] shrink-0 text-[var(--brand-strong)]" aria-hidden />
          <div className="min-w-0">
            <div className="truncate text-[19px] font-semibold leading-tight text-txt tabular-nums">
              {watching ? fmtRate(totalUp) : "-"}
            </div>
            <div className="truncate text-[12px] text-txt3">{t("netUpload")}</div>
          </div>
        </div>
        <button
          type="button"
          onClick={() => setRouterOpen(true)}
          className="press flex items-center gap-3.5 px-5 py-4 text-left transition-colors hover:bg-white/[0.02]"
        >
          <span className="relative grid size-9 shrink-0 place-items-center rounded-[10px] bg-white/[0.04] text-txt2">
            <Router className="size-4" aria-hidden />
            {routerLinked ? (
              <span className="absolute -right-0.5 -top-0.5 size-2 rounded-full bg-[var(--green)] ring-2 ring-[var(--panel)]" />
            ) : null}
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[15px] font-semibold leading-tight text-txt" dir="ltr">
              {gateway?.ip ?? "-"}
            </div>
            <div className="truncate text-[12px] text-txt3">{gatewayLabel}</div>
          </div>
          <ChevronRight className="size-4 shrink-0 text-txt3" aria-hidden />
        </button>
      </div>

      {driverReady === false ? (
        <div className="flex shrink-0 items-center gap-3 rounded-[var(--r-card)] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-4 py-2.5">
          <Info className="size-4 shrink-0 text-[var(--brand-strong)]" aria-hidden />
          <span className="min-w-0 flex-1 text-[12.5px] text-txt2" dir="auto">
            {t("netDriverBody")}
          </span>
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
          <span className="shrink-0 font-mono text-[12px] text-txt" dir="ltr">
            {active.ip}
          </span>
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

      {/* Narrowing the list, then the list itself. */}
      <div className="flex shrink-0 items-center gap-2">
        <div className="flex items-center gap-2">
          {filters.map((f) => {
            const on = filter === f.id
            return (
              <button
                key={f.id}
                type="button"
                onClick={() => setFilter(f.id)}
                className={cn(
                  "press flex h-9 items-center gap-2 rounded-[9px] px-3 text-[12.5px] transition-colors",
                  on
                    ? "bg-[var(--brand-bg)] text-txt shadow-[inset_0_0_0_1px_var(--brand-line)]"
                    : "bg-white/[0.03] text-txt2 hover:bg-white/[0.05] hover:text-txt",
                )}
              >
                <f.icon className="size-[15px]" aria-hidden />
                {f.label}
                <span
                  className={cn(
                    "rounded-[5px] px-1.5 py-[1px] text-[11px] tabular-nums",
                    on ? "bg-[var(--brand-mid)] text-white" : "bg-white/[0.06] text-txt3",
                  )}
                >
                  {f.n}
                </span>
              </button>
            )
          })}
        </div>
        <div className="ml-auto flex items-center gap-2">
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-[15px] -translate-y-1/2 text-txt3" aria-hidden />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("netSearchDevices")}
              className="h-9 w-[280px] rounded-[9px] border border-[var(--line)] bg-[var(--field)] pl-9 pr-3 text-[12.5px] text-txt outline-none placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
          </div>
          <button
            type="button"
            onClick={() => setSort(sort === "ip" ? "traffic" : "ip")}
            title={sort === "ip" ? t("netSortBusy") : t("netSortIp")}
            aria-label={sort === "ip" ? t("netSortBusy") : t("netSortIp")}
            className={cn(
              "press grid size-9 place-items-center rounded-[9px] border transition-colors",
              sort === "traffic"
                ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                : "border-[var(--line)] bg-[var(--field)] text-txt2 hover:text-txt",
            )}
          >
            <SlidersHorizontal className="size-[15px]" aria-hidden />
          </button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-[var(--line)] bg-[var(--panel)]">
        <div className={cn(GRID, "shrink-0 border-b border-[var(--line)] px-5 py-3 text-[11px] uppercase tracking-[0.08em] text-txt3")}>
          <span>{t("netColDevice")}</span>
          <span>{t("netColIp")}</span>
          <span>{t("netColMac")}</span>
          <span>{t("netColTraffic")}</span>
          <span className="text-right">{t("netColActions")}</span>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {busy && devices.length === 0 ? (
            <div className="grid h-full place-items-center text-[13px] text-txt3">{t("netScanning")}</div>
          ) : failed ? (
            <div className="grid h-full place-items-center px-6 text-center text-[13px] text-txt3">{t("netFail")}</div>
          ) : shown.length === 0 ? (
            <div className="grid h-full place-items-center px-6 text-center text-[13px] text-txt3">
              {devices.length === 0 ? t("netEmpty") : t("netNoMatch")}
            </div>
          ) : (
            <ul className="flex flex-col">
              {shown.map((d) => {
                const state = stateOf(d)
                const rate = live[d.ip]
                const protectedRow = d.protected || d.protected_always
                return (
                  <li
                    key={d.ip}
                    className={cn(
                      GRID,
                      "h-16 border-b border-[var(--line)] px-5 last:border-b-0",
                    )}
                  >
                    <span className="flex min-w-0 items-center gap-3">
                      <span
                        className={cn(
                          "grid size-8 shrink-0 place-items-center rounded-full border",
                          d.self_pc
                            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                            : "border-[var(--line)] bg-white/[0.03] text-txt2",
                        )}
                      >
                        {d.self_pc ? <Monitor className="size-[15px]" aria-hidden /> : <Wifi className="size-[15px]" aria-hidden />}
                      </span>
                      <span className="flex min-w-0 flex-col gap-1">
                        <span className="flex min-w-0 items-center gap-1.5">
                          <span className="truncate text-[13.5px] font-medium text-txt" dir="auto">
                            {d.name === "private" ? t("netPrivateName") : d.name}
                          </span>
                          {d.self_pc ? (
                            <Badge tone="brand">{t("netThisPc")}</Badge>
                          ) : null}
                          {protectedRow ? (
                            <Badge tone="green" icon={ShieldCheck}>
                              {t("netProtected")}
                            </Badge>
                          ) : null}
                          {d.source === "device" ? <Badge>{t("netAnnounced")}</Badge> : null}
                          {d.source === "vendor" ? <Badge>{t("netHardware")}</Badge> : null}
                          {state === "blocked" ? <Badge tone="red">{t("netStateBlocked")}</Badge> : null}
                          {state === "capped" ? <Badge tone="brand">{t("netStateCapped")}</Badge> : null}
                        </span>
                      </span>
                    </span>

                    <span className="truncate font-mono text-[12.5px] text-txt2" dir="ltr">
                      {d.ip}
                    </span>
                    <span className="truncate font-mono text-[12px] uppercase text-txt3" dir="ltr">
                      {d.mac || "-"}
                    </span>

                    <span className="flex flex-col gap-0.5 font-mono text-[11.5px] tabular-nums" dir="ltr">
                      {watching && rate ? (
                        <>
                          <span className="flex items-center gap-1 text-[var(--green)]">
                            <ArrowDown className="size-3 shrink-0" aria-hidden />
                            <span className="truncate">{fmtRate(rate.down)}</span>
                          </span>
                          <span className="flex items-center gap-1 text-[var(--brand-strong)]">
                            <ArrowUp className="size-3 shrink-0" aria-hidden />
                            <span className="truncate">{fmtRate(rate.up)}</span>
                          </span>
                        </>
                      ) : (
                        <span className="text-txt3">-</span>
                      )}
                    </span>

                    <span className="flex items-center justify-end gap-1.5">
                      {d.protected_always ? <span className="size-8 shrink-0" /> : (
                        <button
                          type="button"
                          onClick={() => void toggleProtect(d)}
                          title={d.protected ? t("netUnprotect") : t("netProtect")}
                          aria-label={d.protected ? t("netUnprotect") : t("netProtect")}
                          className={cn(
                            "press grid size-8 place-items-center rounded-[8px] border transition-colors",
                            d.protected
                              ? "border-[var(--green-line)] bg-[var(--green-bg)] text-[var(--green)]"
                              : "border-[var(--line)] bg-white/[0.02] text-txt2 hover:border-[var(--green-line)] hover:text-txt",
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
                          "press flex h-8 items-center gap-1.5 rounded-[8px] border px-2 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                          state === "capped"
                            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand-strong)]"
                            : "border-[var(--line)] bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:text-txt",
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
                          "press flex h-8 items-center gap-1.5 rounded-[8px] border px-2 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                          state === "blocked"
                            ? "border-[var(--red-line)] bg-[var(--red-bg)] text-[var(--red)]"
                            : "border-[var(--line)] bg-white/[0.02] text-txt2 hover:border-[var(--red-line)] hover:text-txt",
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
                        className="press grid size-8 place-items-center rounded-[8px] border border-[var(--line)] bg-white/[0.02] text-txt2 transition-colors hover:text-txt"
                      >
                        <MoreVertical className="size-[15px]" aria-hidden />
                      </button>
                    </span>
                  </li>
                )
              })}
            </ul>
          )}
        </div>
      </div>

      {menu ? (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setMenu(null)} aria-hidden />
          <div
            className="fixed z-50 min-w-[168px] overflow-hidden rounded-[10px] border border-[var(--line-strong)] bg-[var(--panel-hover)] py-1 shadow-[var(--sh-2)]"
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
              onClick={() => void copy(devices.find((x) => x.ip === menu.ip)?.mac ?? "")}
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
