import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Activity,
  Ban,
  Check,
  ChevronRight,
  Gauge,
  Info,
  Loader2,
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
import { useI18n, type StrKey } from "@/lib/i18n"
import { api, type Destination, type LanDevice, type LanGuard, type LanStatus, type LanTraffic, type NetworkSnapshot } from "@/lib/ipc"
import { cn } from "@/lib/utils"
import { copyText } from "@/lib/clipboard"
import { Tip } from "@/components/Tip"
import { LanLimitDialog } from "@/components/LanLimitDialog"
import { RouterLinkDialog } from "@/components/RouterLinkDialog"

/** Where a row's name came from. The marker stays the quiet word beside the
    addresses that the screen already used; the title says it in a sentence.
    The winning source is decided in Rust, so this only has to name it. */
const NAME_SOURCE: Record<string, { label: StrKey; tip: StrKey } | undefined> = {
  dhcp: { label: "netSrcDhcp", tip: "netSrcTipDhcp" },
  mdns: { label: "netSrcMdns", tip: "netSrcTipMdns" },
  netbios: { label: "netSrcNetbios", tip: "netSrcTipNetbios" },
  router: { label: "netSrcRouter", tip: "netSrcTipRouter" },
  ping: { label: "netSrcPing", tip: "netSrcTipPing" },
  device: { label: "netSrcDevice", tip: "netSrcTipDevice" },
  vendor: { label: "netSrcGuess", tip: "netSrcTipVendor" },
}

/** Sources that mean the device gave the name itself. */
const ANNOUNCED = new Set(["dhcp", "mdns", "netbios", "device"])

/** Bytes per second as something a person reads at a glance. */
function fmtRate(bytes: number): string {
  if (bytes < 1024) return `${bytes} B/s`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB/s`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB/s`
}

/** A total, not a rate: what one destination has cost the session so far. */
function fmtBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/** The figure and its unit, so the number can carry the size and the unit
    can stay small beside it. */
function splitRate(bytes: number): [string, string] {
  const [num, unit = ""] = fmtRate(bytes).split(" ")
  return [num, unit]
}

type Live = { up: number; down: number; stale?: boolean }

// The rate divisor is the MEASURED interval between two counter samples, and it
// has to be clamped at both ends. Below the floor, two samples taken a few
// milliseconds apart (a manual refresh landing on top of the tick) divided by a
// near-zero window and read as a spike. Above the ceiling, a window stretched by
// a sleeping machine turns a smoothed average into something presented as the
// current rate, so past the stale threshold the row reports nothing instead.
const MIN_WINDOW_S = 0.2
const MAX_WINDOW_S = 30
const STALE_WINDOW_S = 8
type Filter = "all" | "protected" | "router" | "announced"
type Sort = "ip" | "traffic"

/** One grid string for the column titles and every row: two hand-written
    lists drift the moment a column is retuned. */
const ROW = "grid grid-cols-[minmax(0,1fr)_74px_74px_88px] items-center gap-2.5"

/** A traffic bucket belongs to a hardware address, not to an address the lease
    may hand to somebody else tomorrow. A device the neighbour table never
    resolved has no MAC, and its bucket is keyed by its address instead. */
const rateKey = (d: { mac: string; ip: string }): string =>
  d.mac ? d.mac.toLowerCase().replace(/-/g, ":") : d.ip

/**
 * Network: the roster is the screen. One surface holds it: the devices and
 * their live rates on the left, the watch stage on the right, and one flat
 * band under the surface carries the controls that narrow the roster.
 */
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
  // One device can be routed through the tunnel with nothing installed on it. The state
  // is read back from the backend rather than assumed, because a claim can be refused
  // (no elevation, own MAC in the neighbour table) and the button must not lie.
  const [routed, setRouted] = useState<string | null>(null)
  const [routing, setRouting] = useState(false)
  const [watchBusy, setWatchBusy] = useState(false)
  const [live, setLive] = useState<Record<string, Live>>({})
  // Where the bytes went, not just how many: the backend refreshes this every
  // two seconds on its own, so it is polled slower than the counters.
  const [dests, setDests] = useState<Destination[]>([])
  const [filter, setFilter] = useState<Filter>("all")
  const [query, setQuery] = useState("")
  const [sort, setSort] = useState<Sort>("ip")
  const [copied, setCopied] = useState<string | null>(null)
  const prevTraffic = useRef<Record<string, { up: number; down: number; at: number }>>({})
  const polled = useRef(0)

  useEffect(() => {
    void api.lanRoutedDevice().then((r) => setRouted(r?.ip ?? null)).catch(() => {})
  }, [])

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

  /** Turn one snapshot's counters into rates. The divisor is the MEASURED
      interval between two samples, clamped at both ends: below the floor two
      samples a few milliseconds apart read as a spike, and past the stale
      threshold the row reports nothing rather than an old average dressed up
      as the current rate. */
  const applyTraffic = useCallback((rows: LanTraffic[] | null | undefined) => {
    if (!Array.isArray(rows)) {
      setLive({})
      return
    }
    const now = Date.now()
    const next: Record<string, Live> = {}
    for (const row of rows) {
      const key = row.mac ? row.mac.toLowerCase().replace(/-/g, ":") : row.ip
      const before = prevTraffic.current[key]
      if (before && now > before.at) {
        const measured = (now - before.at) / 1000
        const seconds = Math.min(MAX_WINDOW_S, Math.max(MIN_WINDOW_S, measured))
        next[key] = {
          up: Math.max(0, Math.round((row.up - before.up) / seconds)),
          down: Math.max(0, Math.round((row.down - before.down) / seconds)),
          stale: measured > STALE_WINDOW_S,
        }
      } else {
        next[key] = { up: 0, down: 0 }
      }
      prevTraffic.current[key] = { up: row.up, down: row.down, at: now }
    }
    setLive(next)
  }, [])

  /** One snapshot for the whole surface: guard, driver, router session, watch
      state and every device's counters. One round trip, so the roster and the
      rail are the same reading and a rename cannot race a rate sample. */
  const pull = useCallback(async (): Promise<NetworkSnapshot | null> => {
    let snap: NetworkSnapshot
    try {
      snap = await api.networkSnapshot()
    } catch {
      // Leave the last reading on screen: a failed poll is not evidence that
      // the driver went missing or the watch stopped.
      return null
    }
    setGuard(snap.guard)
    setDriverReady(snap.driver_ready)
    setWatching(snap.watching)
    if (snap.watching) applyTraffic(snap.traffic)
    else setLive({})
    if (snap.router_linked) {
      setRouterLinked(true)
      return snap
    }
    // A saved sign in means the router's names come back on their own, and the
    // roster has to be rescanned for them to show up.
    const ok = await api.routerAutoLink().catch(() => false)
    if (ok) {
      setRouterLinked(true)
      void scan(true)
    }
    return snap
  }, [applyTraffic, scan])

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

  // Route one device, or hand it back. The backend returns only after forwarding is
  // confirmed, so a success is a claim in force rather than one attempted; on failure the
  // state is re-read so the badge cannot claim something that is not true.
  const toggleRoute = async (d: LanDevice) => {
    setRouting(true)
    setErr("")
    try {
      if (routed === d.ip) {
        await api.lanUnrouteDevice()
        setRouted(null)
      } else {
        if (routed) await api.lanUnrouteDevice()
        await api.lanRouteDevice(d.ip, d.mac)
        setRouted(d.ip)
      }
    } catch (e) {
      setErr(String(e))
      const now = await api.lanRoutedDevice().catch(() => null)
      setRouted(now?.ip ?? null)
    } finally {
      setRouting(false)
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

  const copy = async (text: string, key?: string) => {
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
        }
      })

    // One snapshot per tick. The five separate calls that used to fan out here
    // are why this interval could not be slowed down.
    const tick = window.setInterval(() => {
      void pull()
    }, 1500)

    // Slower than the tick on purpose: the backend already refreshes its own
    // picture every two seconds, so asking faster only re-reads one value.
    const destTick = window.setInterval(() => {
      void api
        .tunnelDestinations()
        .then((rows) => {
          if (alive) setDests(Array.isArray(rows) ? rows : [])
        })
        .catch(() => undefined)
    }, 6000)
    void api
      .tunnelDestinations()
      .then((rows) => {
        if (alive) setDests(Array.isArray(rows) ? rows : [])
      })
      .catch(() => undefined)

    // The same snapshot the tick makes, taken straight away so the guard, the
    // driver and the router state are on screen at first paint.
    void pull().then((snap) => {
      if (!alive || !snap || !snap.watching) return
      // A first sample has no previous counters to diff against, so take one
      // more a beat later: the rates are then a second old instead of waiting
      // for the next tick.
      window.setTimeout(() => {
        if (alive) void pull()
      }, 220)
    })

    return () => {
      alive = false
      window.clearInterval(tick)
      window.clearInterval(destTick)
    }
  }, [scan, refreshStatus, pull])

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
      announced: devices.filter((d) => ANNOUNCED.has(d.source)).length,
      router: devices.filter((d) => d.source === "router").length,
    }
  }, [devices])

  const shown = useMemo(() => {
    const isProtected = (d: LanDevice) => d.protected || d.protected_always
    const q = query.trim().toLowerCase()
    let list = devices.filter((d) => {
      if (filter === "protected" && !isProtected(d)) return false
      if (filter === "announced" && !ANNOUNCED.has(d.source)) return false
      if (filter === "router" && d.source !== "router") return false
      if (!q) return true
      return (
        d.name.toLowerCase().includes(q) ||
        d.ip.includes(q) ||
        d.mac.toLowerCase().includes(q)
      )
    })
    if (sort === "traffic") {
      const score = (d: LanDevice) =>
        (live[rateKey(d)]?.down ?? 0) + (live[rateKey(d)]?.up ?? 0)
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

  // One bar per device, and the measure is its share of everything moving on
  // this screen. Scaling to the busiest device instead reads full whenever
  // exactly one device is doing anything, which is a confident 100% about
  // almost nothing. Below the noise floor the bar is empty rather than
  // scaling up to a full one.
  const IDLE_BPS = 100 * 1024
  const totalOnScreen = shown.reduce(
    (sum, d) => sum + (live[rateKey(d)]?.down ?? 0) + (live[rateKey(d)]?.up ?? 0),
    0,
  )
  const totalDown = Object.values(live).reduce((sum, v) => sum + v.down, 0)
  const totalUp = Object.values(live).reduce((sum, v) => sum + v.up, 0)

  const filters: { id: Filter; label: string; n: number }[] = [
    { id: "all", label: t("netAllDevices"), n: counts.all },
    { id: "protected", label: t("netProtected"), n: counts.protected },
    { id: "router", label: t("netRouter"), n: counts.router },
    { id: "announced", label: t("netAnnounced"), n: counts.announced },
  ]

  // A total built from samples we have already called stale would be an average
  // presented as a live figure, so the rail drops to a dash with the rows.
  const allStale = Object.keys(live).length > 0 && Object.values(live).every((v) => v.stale)
  const [downNum, downUnit] = watching && !allStale ? splitRate(totalDown) : ["-", ""]
  const [upNum, upUnit] = watching && !allStale ? splitRate(totalUp) : ["-", ""]

  /* The row actions are one flat trio: no frame inside the surface, the
     state carried by colour and the icon itself. */
  const actionCls =
    "press grid size-7 place-items-center rounded-[7px] text-txt2 transition-colors enabled:hover:bg-white/[0.06] enabled:hover:text-txt disabled:cursor-not-allowed disabled:opacity-35"

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {/* One surface, two zones: the roster and its rates on the left, the
          watch stage one tone step down on the right. */}
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)] shadow-[inset_0_1px_0_rgb(255_255_255/0.045)]">
        <section
          aria-label={t("netTitle")}
          aria-busy={busy}
          className="flex min-h-0 min-w-0 flex-col"
        >
          {/* One header line does three jobs: how many devices, the name of
              each reading column, and the rescan. */}
          <div className={cn(ROW, "shrink-0 border-b border-line px-4 py-2.5")}>
            <span className="truncate text-left text-[11px] text-txt3">{t("netDevices")}</span>
            <span className="text-end text-[11px] text-txt3">{t("netDownload")}</span>
            <span className="text-end text-[11px] text-txt3">{t("netUpload")}</span>
            <span className="flex justify-end">
              <Tip text={busy ? t("netScanning") : t("netRefresh")} align="end">
                <button
                  type="button"
                  onClick={() => void scan()}
                  disabled={busy}
                  aria-label={t("netRefresh")}
                  className="press grid size-7 place-items-center rounded-[7px] text-txt2 transition-colors enabled:hover:bg-white/[0.06] enabled:hover:text-txt disabled:cursor-wait"
                >
                  {busy ? (
                    <Loader2 className="size-[15px] animate-spin" aria-hidden />
                  ) : (
                    <RefreshCw className="size-[15px]" aria-hidden />
                  )}
                </button>
              </Tip>
            </span>
          </div>

          <div className="app-scroll flex min-h-0 flex-1 flex-col overflow-y-auto">
            {busy && devices.length === 0 ? (
              <div aria-hidden>
                {[0, 1, 2, 3, 4, 5].map((i) => (
                  <div
                    key={i}
                    className={cn(
                      ROW,
                      "min-h-[58px] flex-1 border-b border-line px-4 py-3.5 last:border-b-0",
                    )}
                  >
                    <span className="flex flex-col gap-2">
                      <span className="net-wait h-3 w-28 rounded bg-white/[0.05]" />
                      <span className="net-wait h-2.5 w-40 rounded bg-white/[0.035]" />
                    </span>
                    <span className="net-wait ms-auto h-3 w-10 rounded bg-white/[0.05]" />
                    <span className="net-wait ms-auto h-3 w-10 rounded bg-white/[0.05]" />
                  </div>
                ))}
              </div>
            ) : failed ? (
              <div className="flex h-full flex-col items-center justify-center gap-3">
                <ShieldAlert className="size-5 text-txt3" aria-hidden />
                <span className="text-[12px] text-txt2">{t("netFail")}</span>
                <button
                  type="button"
                  onClick={() => void scan()}
                  className="press h-8 rounded-[8px] px-2.5 text-[12px] text-brand-strong transition-colors hover:bg-[var(--brand-bg)]"
                >
                  {t("netRefresh")}
                </button>
              </div>
            ) : shown.length === 0 ? (
              <div className="flex h-full flex-col items-center justify-center gap-2.5">
                <Wifi className="size-5 text-txt3" aria-hidden />
                <span className="text-[12px] text-txt2">
                  {devices.length === 0 ? t("netEmpty") : t("netNoMatch")}
                </span>
              </div>
            ) : (
              <div key={`${filter}|${query}|${sort}`} className="net-list-in flex min-h-0 flex-1 flex-col">
                {shown.map((d) => {
                  const state = stateOf(d)
                  const rate = live[rateKey(d)]
                  const protectedRow = d.protected || d.protected_always
                  const total = (rate?.down ?? 0) + (rate?.up ?? 0)
                  const share = totalOnScreen > IDLE_BPS ? Math.min(1, total / totalOnScreen) : 0
                  // A row one sweep away from being dropped keeps its place but
                  // states no rate: a number from a device the latest sample did
                  // not prove would be the same confident guess this screen was
                  // built to stop making.
                  const quiet = d.live === false
                  const src = NAME_SOURCE[d.source]
                  const [dn, dnu] = watching && rate && !rate.stale && !quiet ? splitRate(rate.down) : ["-", ""]
                  const [un, unu] = watching && rate && !rate.stale && !quiet ? splitRate(rate.up) : ["-", ""]
                  return (
                    <div
                      key={d.ip}
                      title={quiet ? t("netQuiet") : undefined}
                      className={cn(
                        ROW,
                        "net-row relative min-h-[58px] max-h-[112px] flex-1 border-b border-line px-4 py-2.5 last:border-b-0 hover:bg-white/[0.02]",
                      )}
                    >
                      {/* identity: the name, then the two addresses under it,
                          both of them one press from the clipboard */}
                      <div className="flex min-w-0 flex-col gap-0.5">
                        <span className="flex min-w-0 items-center gap-1.5">
                          <span className="truncate text-[14px] font-semibold text-txt" dir="auto">
                            {d.name === "private" ? t("netPrivateName") : d.name}
                          </span>
                          {d.self_pc ? (
                            <span className="shrink-0 rounded-[5px] bg-[var(--brand-bg)] px-1.5 py-px text-[11px] font-medium text-brand-strong">
                              {t("netThisPc")}
                            </span>
                          ) : null}
                          {routed === d.ip ? (
                            <span className="shrink-0 rounded-[5px] bg-[var(--green-bg)] px-1.5 py-px text-[11px] font-medium text-[var(--green)]">
                              {t("netViaTunnel")}
                            </span>
                          ) : null}
                          {state === "blocked" ? (
                            <span className="shrink-0 text-[11px] font-medium text-[var(--red)]">
                              {t("netStateBlocked")}
                            </span>
                          ) : state === "capped" ? (
                            <span className="shrink-0 text-[11px] font-medium text-brand-strong">
                              {t("netStateCapped")}
                            </span>
                          ) : null}
                        </span>
                        <span className="flex min-w-0 items-center gap-1">
                          {!d.self_pc ? (
                            <button
                              type="button"
                              disabled={routing}
                              onClick={() => void toggleRoute(d)}
                              title={t("netViaTunnelHint")}
                              className="shrink-0 rounded-[6px] border border-line px-1.5 py-px text-[11px] font-medium text-txt2 transition-colors hover:border-[var(--brand-line)] hover:text-brand-strong disabled:opacity-50"
                            >
                              {routed === d.ip ? t("netUnroute") : t("netRoute")}
                            </button>
                          ) : null}
                          <button
                            type="button"
                            onClick={() => void copy(d.ip, `ip-${d.ip}`)}
                            title={t("netCopyIp")}
                            aria-label={`${t("netCopyIp")} ${d.ip}`}
                            className="press inline-flex h-6 shrink-0 items-center gap-1 rounded-[5px] font-mono text-[11px] text-txt2 transition-colors hover:text-txt"
                          >
                            <span dir="ltr">{d.ip}</span>
                            {copied === `ip-${d.ip}` ? (
                              <Check className="net-check-in size-3 shrink-0 text-[var(--green)]" aria-hidden />
                            ) : null}
                          </button>
                          <span className="shrink-0 text-[11px] text-txt3" aria-hidden>
                            ·
                          </span>
                          <button
                            type="button"
                            onClick={() => void copy(d.mac.toUpperCase(), `mac-${d.ip}`)}
                            disabled={!d.mac}
                            title={t("netCopyMac")}
                            aria-label={`${t("netCopyMac")} ${d.mac}`}
                            className="press inline-flex h-6 shrink-0 items-center gap-1 rounded-[5px] font-mono text-[11px] uppercase text-txt3 transition-colors enabled:hover:text-txt2 disabled:cursor-default"
                          >
                            <span dir="ltr">{d.mac || "-"}</span>
                            {copied === `mac-${d.ip}` ? (
                              <Check className="net-check-in size-3 shrink-0 text-[var(--green)]" aria-hidden />
                            ) : null}
                          </button>
                          {src ? (
                            <>
                              <span className="shrink-0 text-[11px] text-txt3" aria-hidden>
                                ·
                              </span>
                              <span
                                className="min-w-0 truncate text-[11px] text-txt3"
                                title={t(src.tip)}
                              >
                                {t(src.label)}
                              </span>
                            </>
                          ) : null}
                        </span>
                        {/* this device's share of everything moving on this
                            screen, which is the same quantity the two figures
                            beside it add up to */}
                        <span
                          className="mt-2 h-[4px] w-full max-w-[220px] overflow-hidden rounded-full bg-white/[0.06]"
                          title={t("netShareOfScreen")}
                          aria-hidden
                        >
                          <span
                            className="net-bar block h-full origin-left rounded-full bg-white/35"
                            style={{ transform: `scaleX(${share})` }}
                          />
                        </span>
                      </div>

                      {/* the two rates, the figure in the numeral voice and
                          the unit quiet beside it */}
                      <span className="flex items-baseline justify-end gap-1">
                        <span
                          className={cn(
                            "font-display text-[17px] font-semibold leading-[1.25] tabular-nums",
                            watching && rate ? "text-[var(--green)]" : "text-txt3",
                          )}
                        >
                          {dn}
                        </span>
                        <span className="text-[11px] text-txt3">{dnu}</span>
                      </span>
                      <span className="flex items-baseline justify-end gap-1">
                        <span
                          className={cn(
                            "font-display text-[17px] font-semibold leading-[1.25] tabular-nums",
                            watching && rate ? "text-brand-strong" : "text-txt3",
                          )}
                        >
                          {un}
                        </span>
                        <span className="text-[11px] text-txt3">{unu}</span>
                      </span>

                      {/* three actions, no frames, the state in the colour */}
                      <div className="flex items-center justify-end gap-0.5">
                        {d.protected_always ? (
                          <Tip text={t("netProtected")} align="end">
                            <span
                              role="img"
                              aria-label={t("netProtected")}
                              className="grid size-7 place-items-center text-[var(--green)]"
                            >
                              <ShieldCheck className="size-4" aria-hidden />
                            </span>
                          </Tip>
                        ) : (
                          <Tip text={d.protected ? t("netUnprotect") : t("netProtect")} align="end">
                            <button
                              type="button"
                              onClick={() => void toggleProtect(d)}
                              aria-label={d.protected ? t("netUnprotect") : t("netProtect")}
                              aria-pressed={d.protected}
                              className={cn(
                                actionCls,
                                d.protected ? "text-[var(--green)]" : "",
                              )}
                            >
                              <ShieldCheck className="size-4" aria-hidden />
                            </button>
                          </Tip>
                        )}
                        <Tip text={t("netLimit")} align="end">
                          <button
                            type="button"
                            onClick={() => setLimitFor(d)}
                            disabled={driverReady === false || protectedRow}
                            aria-label={t("netLimit")}
                            className={cn(actionCls, state === "capped" ? "text-brand-strong" : "")}
                          >
                            <Gauge className="size-4" aria-hidden />
                          </button>
                        </Tip>
                        <Tip text={state === "blocked" ? t("netUnblock") : t("netBlock")} align="end">
                          <button
                            type="button"
                            onClick={() => void toggleBlock(d)}
                            disabled={driverReady === false || protectedRow}
                            aria-label={state === "blocked" ? t("netUnblock") : t("netBlock")}
                            className={cn(actionCls, state === "blocked" ? "text-[var(--red)]" : "")}
                          >
                            <Ban className="size-4" aria-hidden />
                          </button>
                        </Tip>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        </section>

        {/* the watch stage: its own line at the top, the whole network's
            rates holding the middle, and the control with the router on the
            floor. Three anchored zones, no card inside the surface. */}
        <aside className="flex min-h-0 flex-col border-s border-line bg-[var(--side)] px-5">
          <span className="flex h-[38px] shrink-0 items-center gap-2 border-b border-line text-[12px]">
            {watching ? (
              <span className="net-live-dot size-1.5 shrink-0 rounded-full bg-[var(--green)]" aria-hidden />
            ) : null}
            <span className={cn("truncate", watching ? "text-txt" : "text-txt2")}>
              {watching ? t("netWatching") : t("netIdle")}
            </span>
            <span className="ms-auto shrink-0 text-[11px] text-txt2">
              {t("netCount").replace("{n}", String(counts.all))}
            </span>
          </span>

          <div className="flex min-h-0 flex-1 flex-col justify-center gap-8">
            <button
              type="button"
              onClick={() => void toggleWatch()}
              disabled={watchBusy || driverReady === false}
              className={cn(
                "press flex h-12 w-full items-center justify-center gap-2 rounded-[10px] text-[14px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45",
                watching
                  ? "bg-[var(--green-bg)] text-[var(--green)] hover:bg-[rgb(95_176_140/0.18)]"
                  : "bg-[var(--brand)] text-white hover:brightness-110",
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

            <span className="flex flex-col gap-6">
              <span className="flex flex-col items-start gap-0.5">
                <span className="text-[11px] text-txt2">{t("netDownload")}</span>
                <span className="flex items-baseline gap-2">
                  <span
                    key={`d-${watching ? "on" : "off"}`}
                    className={cn(
                      "net-list-in font-display text-[48px] font-semibold leading-[1.16] tracking-[-0.035em] tabular-nums",
                      watching ? "text-[var(--green)]" : "text-txt3",
                    )}
                  >
                    {downNum}
                  </span>
                  {downUnit ? <span className="text-[14px] text-txt2">{downUnit}</span> : null}
                </span>
              </span>
              <span className="flex flex-col items-start gap-0.5">
                <span className="text-[11px] text-txt2">{t("netUpload")}</span>
                <span className="flex items-baseline gap-2">
                  <span
                    key={`u-${watching ? "on" : "off"}`}
                    className={cn(
                      "net-list-in font-display text-[48px] font-semibold leading-[1.16] tracking-[-0.035em] tabular-nums",
                      watching ? "text-brand-strong" : "text-txt3",
                    )}
                  >
                    {upNum}
                  </span>
                  {upUnit ? <span className="text-[14px] text-txt2">{upUnit}</span> : null}
                </span>
              </span>
            </span>
          </div>

          {/* what the tunnel is carrying, not just how much: session scoped,
              so it is a reading of now and never presented as history */}
          <div className="shrink-0 border-t border-line pt-3">
            <span className="text-[11px] text-txt2">{t("netDestinations")}</span>
            {dests.length === 0 ? (
              <p className="mt-1 text-[12px] leading-snug text-txt3">{t("netDestinationsEmpty")}</p>
            ) : (
              <ul className="mt-1.5 flex flex-col gap-1">
                {dests.map((d) => (
                  <li
                    key={`${d.process}|${d.host}`}
                    className="flex items-baseline justify-between gap-3"
                  >
                    <span className="min-w-0 truncate text-[12px] text-txt" dir="auto" title={d.host}>
                      {d.host || "-"}
                    </span>
                    <span className="shrink-0 font-mono text-[11px] tabular-nums text-txt3" dir="ltr">
                      {fmtBytes(d.up + d.down)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="shrink-0 pb-4">
            <button
              type="button"
              onClick={() => setRouterOpen(true)}
              className="press flex w-full items-center gap-3 border-t border-line pt-3.5 text-start transition-colors hover:opacity-90"
            >
              <Router className="size-4 shrink-0 text-txt2" aria-hidden />
              <span className="flex min-w-0 flex-1 flex-col items-start">
                <span className="max-w-full truncate text-[11px] text-txt2" dir="auto">
                  {routerLinked ? t("routerLinked") : t("routerNames")}
                </span>
                <span className="max-w-full truncate font-mono text-[14px] text-txt" dir="ltr">
                  {gateway?.ip ?? "-"}
                </span>
                <span className="max-w-full truncate text-[11px] text-txt3" dir="auto">
                  {gatewayLabel}
                </span>
              </span>
              {routerLinked ? (
                <span className="size-1.5 shrink-0 rounded-full bg-[var(--green)]" aria-hidden />
              ) : null}
              <ChevronRight className="size-4 shrink-0 text-txt3" aria-hidden />
            </button>
          </div>
        </aside>
      </div>

      {/* What the surface has to report when something is missing or wrong:
          one line each, no frame, the product's own accent. */}
      {driverReady === false ? (
        <div className="flex shrink-0 items-center gap-3 border-s-2 border-[var(--brand)] bg-[var(--brand-bg)] py-2 ps-3 pe-2">
          <Info className="size-4 shrink-0 text-brand-strong" aria-hidden />
          <span className="min-w-0 flex-1 text-[12px] leading-snug text-txt2" dir="auto">
            {t("netDriverBody")}
          </span>
          <button
            type="button"
            onClick={() => void api.openPage("https://npcap.com/#download")}
            className="press h-7 shrink-0 rounded-[7px] px-2 text-[12px] font-medium text-brand-strong transition-colors hover:bg-white/[0.06]"
          >
            {t("netDriverGet")}
          </button>
        </div>
      ) : null}

      {guard && guard.attacker_mac ? (
        <div className="flex shrink-0 items-center gap-3 border-s-2 border-[var(--red)] bg-[var(--red-bg)] py-2 ps-3 pe-3">
          <ShieldAlert className="size-4 shrink-0 text-[var(--red)]" aria-hidden />
          <span className="min-w-0 flex-1 text-[12px] leading-snug text-[var(--red)]" dir="auto">
            {t("netGuardAlert").replace("{mac}", guard.attacker_mac)}
          </span>
        </div>
      ) : null}

      {active ? (
        <div className="flex shrink-0 items-center gap-3 border-s-2 border-[var(--brand)] bg-[var(--brand-bg)] py-1.5 ps-3 pe-2">
          <span className="shrink-0 font-mono text-[12px] text-txt" dir="ltr">
            {active.ip}
          </span>
          <span className="min-w-0 flex-1 truncate text-[12px] text-txt2">
            {active.blocked
              ? t("netActiveBlocked")
              : t("netActiveCapped")
                  .replace("{down}", String(Number(active.down_kbps) || 0))
                  .replace("{up}", String(Number(active.up_kbps) || 0))}
          </span>
          <button
            type="button"
            onClick={() => void stopAll()}
            className="press flex h-7 shrink-0 items-center gap-1.5 rounded-[7px] px-2 text-[12px] text-txt2 transition-colors hover:bg-white/[0.06] hover:text-txt"
          >
            <X className="size-3.5" aria-hidden />
            {t("netRemove")}
          </button>
        </div>
      ) : null}

      {err ? (
        <div className="shrink-0 border-s-2 border-[var(--red)] bg-[var(--red-bg)] py-1.5 ps-3 pe-3 text-[12px] text-[var(--red)]" dir="auto">
          {err}
        </div>
      ) : null}

      {/* The band under the surface: one flat line of controls that narrow
          the roster. No outer frame, no frames inside. */}
      <div className="flex shrink-0 items-center gap-1">
        {filters.map((f) => {
          const on = filter === f.id
          return (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              aria-pressed={on}
              className={cn(
                "press flex h-8 items-center gap-1.5 rounded-[8px] px-2.5 text-[12px] transition-colors",
                on
                  ? "bg-[var(--brand-bg)] text-txt"
                  : "text-txt2 hover:bg-white/[0.04] hover:text-txt",
              )}
            >
              {f.label}
              <span className={cn("tabular-nums", on ? "text-brand-strong" : "text-txt3")}>
                {f.n}
              </span>
            </button>
          )
        })}
        <div className="ms-auto flex items-center gap-2">
          <div className="relative">
            <Search
              className="pointer-events-none absolute start-2.5 top-1/2 size-[14px] -translate-y-1/2 text-txt3"
              aria-hidden
            />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("netSearchDevices")}
              aria-label={t("netSearchDevices")}
              className="h-8 w-[232px] rounded-[8px] border border-line bg-[var(--field)] ps-8 pe-2.5 text-[12px] text-txt outline-none placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
          </div>
          <Tip text={sort === "ip" ? t("netSortBusy") : t("netSortIp")} align="end">
            <button
              type="button"
              onClick={() => setSort(sort === "ip" ? "traffic" : "ip")}
              aria-label={sort === "ip" ? t("netSortBusy") : t("netSortIp")}
              aria-pressed={sort === "traffic"}
              className={cn(
                "press grid size-8 place-items-center rounded-[8px] transition-colors",
                sort === "traffic"
                  ? "bg-[var(--brand-bg)] text-brand-strong"
                  : "text-txt2 hover:bg-white/[0.04] hover:text-txt",
              )}
            >
              <SlidersHorizontal className="size-[14px]" aria-hidden />
            </button>
          </Tip>
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
