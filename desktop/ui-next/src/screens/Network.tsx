import { useCallback, useEffect, useRef, useState } from "react"
import {
  Activity,
  Ban,
  Gauge,
  Info,
  Loader2,
  Monitor,
  RefreshCw,
  Router,
  ShieldAlert,
  ShieldCheck,
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

/** Who is on this network, what they are moving, and what is being done to them. */
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
  const prevTraffic = useRef<Record<string, { up: number; down: number; at: number }>>({})
  const polled = useRef(0)

  const scan = useCallback(async (quiet = false) => {
    if (!quiet) setBusy(true)
    setFailed(false)
    try {
      const list = await api.lanDevices()
      setDevices(list)
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

  useEffect(() => {
    let alive = true
    void api
      .lanKnown()
      .then((list) => {
        if (alive && list.length) setDevices(list)
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

  // A name means different things depending on where it came from, and the
  // difference matters: a router name is exact, a hardware one is the maker.
  const sourceChip = (d: LanDevice) => {
    if (d.self_pc) return null
    const key =
      d.source === "router" ? "netSrcRouter" : d.source === "device" ? "netSrcDevice" : d.source === "vendor" ? "netSrcGuess" : ""
    if (!key) return null
    return (
      <span className="shrink-0 rounded-[5px] border border-line px-1.5 py-[1px] text-[11px] text-txt3">
        {t(key)}
      </span>
    )
  }

  const capLine = active
    ? active.blocked
      ? t("netActiveBlocked")
      : t("netActiveCapped").replace("{down}", String(active.down_kbps)).replace("{up}", String(active.up_kbps))
    : ""

  const totalDown = Object.values(live).reduce((sum, v) => sum + v.down, 0)
  const totalUp = Object.values(live).reduce((sum, v) => sum + v.up, 0)

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col">
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        {/* The one control this screen exists for, and what it is producing. */}
        <div className="flex shrink-0 items-center gap-5 border-b border-line px-6 py-5">
          <span
            className={cn(
              "grid size-11 shrink-0 place-items-center rounded-full border",
              watching
                ? "border-[var(--green)]/45 bg-[var(--green)]/10 text-[var(--green)]"
                : "border-white/[0.11] bg-white/[0.03] text-txt2",
            )}
          >
            <Activity className="size-5" aria-hidden />
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[15px] font-medium text-txt">
              {watching ? t("netWatching") : t("netIdle")}
            </div>
            <div className="mt-1 flex min-w-0 items-center gap-3 text-[12px] text-txt2">
              <span className="truncate">{t("netCount").replace("{n}", String(devices.length))}</span>
              {watching ? (
                <span className="shrink-0 font-mono text-[12px] tabular-nums" dir="ltr">
                  <span className="text-[var(--green)]">v {fmtRate(totalDown)}</span>
                  <span className="px-1.5 text-txt3">/</span>
                  <span className="text-[var(--brand)]">^ {fmtRate(totalUp)}</span>
                </span>
              ) : null}
            </div>
          </div>
          <button
            type="button"
            onClick={() => setRouterOpen(true)}
            className={cn(
              "press flex h-10 shrink-0 items-center gap-1.5 rounded-[10px] px-3 text-[13px] transition-colors",
              routerLinked
                ? "border border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand)]"
                : "border border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
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
            className="press grid size-10 shrink-0 place-items-center rounded-[10px] border border-line bg-white/[0.02] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <RefreshCw className="size-4" aria-hidden />}
          </button>
          <button
            type="button"
            onClick={() => void toggleWatch()}
            disabled={watchBusy || driverReady === false}
            className={cn(
              "press flex h-10 shrink-0 items-center gap-2 rounded-[10px] px-4 text-[13.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45",
              watching
                ? "border border-[var(--green)]/45 bg-[var(--green)]/10 text-[var(--green)]"
                : "bg-[var(--brand)] text-white hover:brightness-110",
            )}
          >
            {watchBusy ? (
              <Loader2 className="size-4 animate-spin" aria-hidden />
            ) : watching ? (
              <X className="size-4" aria-hidden />
            ) : (
              <Activity className="size-4" aria-hidden />
            )}
            {watching ? t("netStopWatch") : t("netStartWatch")}
          </button>
        </div>

        {driverReady === false ? (
          <div className="flex shrink-0 items-center gap-3 border-b border-line bg-[var(--brand-bg)] px-6 py-2.5">
            <Info className="size-4 shrink-0 text-[var(--brand)]" aria-hidden />
            <span className="min-w-0 flex-1 text-[12.5px] text-txt2" dir="auto">
              {t("netDriverBody")}
            </span>
            <button
              type="button"
              onClick={() => void api.openPage("https://npcap.com/#download")}
              className="press flex h-8 shrink-0 items-center gap-1.5 rounded-[9px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-2.5 text-[12px] text-[var(--brand)]"
            >
              {t("netDriverGet")}
            </button>
          </div>
        ) : null}

        {guard && guard.attacker_mac ? (
          <div className="flex shrink-0 items-center gap-3 border-b border-line bg-[var(--red)]/10 px-6 py-2.5">
            <ShieldAlert className="size-4 shrink-0 text-[var(--red)]" aria-hidden />
            <span className="min-w-0 flex-1 text-[12.5px] text-[var(--red)]" dir="auto">
              {t("netGuardAlert").replace("{mac}", guard.attacker_mac)}
            </span>
          </div>
        ) : null}

        {active ? (
          <div className="flex shrink-0 items-center gap-3 border-b border-line bg-[var(--brand-bg)] px-6 py-2.5">
            <span className="shrink-0 font-mono text-[12px] text-txt" dir="ltr">
              {active.ip}
            </span>
            <span className="min-w-0 flex-1 truncate text-[12.5px] text-txt2">{capLine}</span>
            <button
              type="button"
              onClick={() => void stopAll()}
              className="press flex h-8 shrink-0 items-center gap-1.5 rounded-[9px] border border-line bg-white/[0.02] px-2.5 text-[12px] text-txt2 hover:text-txt"
            >
              <X className="size-3.5" aria-hidden />
              {t("netRemove")}
            </button>
          </div>
        ) : null}

        {err ? (
          <div className="shrink-0 border-b border-line bg-[var(--red)]/10 px-6 py-2 text-[12px] text-[var(--red)]" dir="auto">
            {err}
          </div>
        ) : null}

        <div className="min-h-0 flex-1 overflow-y-auto">
          {busy && devices.length === 0 ? (
            <div className="grid h-full place-items-center text-[13px] text-txt3">{t("netScanning")}</div>
          ) : failed ? (
            <div className="grid h-full place-items-center px-6 text-center text-[13px] text-txt3">{t("netFail")}</div>
          ) : devices.length === 0 ? (
            <div className="grid h-full place-items-center px-6 text-center text-[13px] text-txt3">{t("netEmpty")}</div>
          ) : (
            <ul className="flex flex-col">
              {devices.map((d) => {
                const state = stateOf(d)
                const rate = live[d.ip]
                return (
                  <li
                    key={d.ip}
                    className="flex h-[64px] items-center gap-4 border-b border-line px-6 last:border-b-0"
                  >
                    <span
                      className={cn(
                        "grid size-9 shrink-0 place-items-center rounded-full border",
                        d.self_pc
                          ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand)]"
                          : "border-white/[0.11] bg-[rgb(255_255_255/0.03)] text-txt2",
                      )}
                    >
                      {d.self_pc ? <Monitor className="size-4" aria-hidden /> : <Wifi className="size-4" aria-hidden />}
                    </span>

                    <span className="min-w-0 flex-[1.6]">
                      <span className="flex min-w-0 items-center gap-2">
                        <span className="truncate text-[14px] font-medium text-txt" dir="auto">
                          {d.name === "private" ? t("netPrivateName") : d.name}
                        </span>
                        {d.self_pc ? (
                          <span className="shrink-0 rounded-[5px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-1.5 py-[1px] text-[11px] font-medium text-[var(--brand)]">
                            {t("netThisPc")}
                          </span>
                        ) : null}
                        {d.protected ? (
                          <span className="flex shrink-0 items-center gap-1 rounded-[5px] border border-[var(--green)]/40 bg-[var(--green)]/10 px-1.5 py-[1px] text-[11px] font-medium text-[var(--green)]">
                            <ShieldCheck className="size-3" aria-hidden />
                            {t("netProtected")}
                          </span>
                        ) : null}
                        {sourceChip(d)}
                        {state === "blocked" ? (
                          <span className="shrink-0 rounded-[5px] border border-[var(--red)]/45 bg-[var(--red)]/10 px-1.5 py-[1px] text-[11px] font-medium text-[var(--red)]">
                            {t("netStateBlocked")}
                          </span>
                        ) : null}
                        {state === "capped" ? (
                          <span className="shrink-0 rounded-[5px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-1.5 py-[1px] text-[11px] font-medium text-[var(--brand)]">
                            {t("netStateCapped")}
                          </span>
                        ) : null}
                      </span>
                      <span className="mt-0.5 block truncate font-mono text-[12px] text-txt3" dir="ltr">
                        {d.ip}
                      </span>
                    </span>

                    <span className="hidden w-[150px] shrink-0 font-mono text-[12px] uppercase text-txt3 md:block" dir="ltr">
                      {d.mac}
                    </span>

                    {/* The reading this screen is for: what that device is moving now. */}
                    <span className="w-[168px] shrink-0 text-right font-mono text-[12.5px] tabular-nums" dir="ltr">
                      {watching && rate ? (
                        <>
                          <span className="text-[var(--green)]">v {fmtRate(rate.down)}</span>
                          <span className="px-1.5 text-txt3">/</span>
                          <span className="text-[var(--brand)]">^ {fmtRate(rate.up)}</span>
                        </>
                      ) : (
                        <span className="text-txt3">-</span>
                      )}
                    </span>

                    <span className="flex shrink-0 items-center gap-2">
                      {d.protected_always ? null : (
                        <button
                          type="button"
                          onClick={() => void toggleProtect(d)}
                          title={d.protected ? t("netUnprotect") : t("netProtect")}
                          className={cn(
                            "press grid size-8 place-items-center rounded-[9px] border transition-colors",
                            d.protected
                              ? "border-[var(--green)]/45 bg-[var(--green)]/10 text-[var(--green)]"
                              : "border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
                          )}
                        >
                          <ShieldCheck className="size-3.5" aria-hidden />
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => setLimitFor(d)}
                        disabled={driverReady === false || d.protected}
                        title={t("netLimit")}
                        className={cn(
                          "press flex h-8 items-center gap-1.5 rounded-[9px] border px-2.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                          state === "capped"
                            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand)]"
                            : "border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
                        )}
                      >
                        <Gauge className="size-3.5" aria-hidden />
                        {t("netLimit")}
                      </button>
                      <button
                        type="button"
                        onClick={() => void toggleBlock(d)}
                        disabled={driverReady === false || d.protected}
                        className={cn(
                          "press flex h-8 items-center gap-1.5 rounded-[9px] border px-2.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                          state === "blocked"
                            ? "border-[var(--red)]/50 bg-[var(--red)]/10 text-[var(--red)]"
                            : "border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
                        )}
                      >
                        <Ban className="size-3.5" aria-hidden />
                        {state === "blocked" ? t("netUnblock") : t("netBlock")}
                      </button>
                    </span>
                  </li>
                )
              })}
            </ul>
          )}
        </div>
      </div>

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
