import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Ban,
  Check,
  Gauge,
  Info,
  Loader2,
  RefreshCw,
  Route,
  Router,
  Search,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  X,
} from "lucide-react"
import { useI18n, type StrKey } from "@/lib/i18n"
import {
  api,
  type Destination,
  type LanDevice,
  type LanGuard,
  type LanStatus,
  type LanTraffic,
  type NetworkSnapshot,
} from "@/lib/ipc"
import { cn } from "@/lib/utils"
import { copyText } from "@/lib/clipboard"
import { Tip } from "@/components/Tip"
import { LanLimitDialog } from "@/components/LanLimitDialog"
import { RouterLinkDialog } from "@/components/RouterLinkDialog"

/**
 * Who is on this network, and which one of them rides the tunnel.
 *
 * The screen is a roster first: one row per device, one line under a sticky
 * column header, everything a row can do in the row itself. The single surface
 * above it is the route stage, because routing one device through the tunnel
 * with nothing installed on it is the thing this screen exists for, and the
 * tunnel's own picture (what it is carrying, where the names come from) belongs
 * beside it. Nothing is drawn that is not a reading: no charts, no sparklines,
 * no bars.
 */

/** Where a row's name came from. The word stays on the row; the title says it
    in a sentence. The winner is decided in Rust, so this only names it. */
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

function fmtRate(bytes: number): string {
  if (bytes < 1024) return `${bytes} B/s`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB/s`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB/s`
}

function fmtBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/** The figure and its unit, so the number carries the size and the unit stays
    small beside it. */
function splitRate(bytes: number): [string, string] {
  const [num, unit = ""] = fmtRate(bytes).split(" ")
  return [num, unit]
}

type Live = { up: number; down: number; stale?: boolean }

// The rate divides by the MEASURED gap between two counter samples, clamped at
// both ends. Below the floor, two samples a few milliseconds apart read as a
// spike. Past the stale threshold the row reports nothing rather than an old
// average dressed up as the current rate.
const MIN_WINDOW_S = 0.2
const MAX_WINDOW_S = 30
const STALE_WINDOW_S = 8

type Filter = "all" | "protected" | "router" | "announced"
type Sort = "ip" | "traffic"

/** One grid string for the column titles and every row: two hand-written lists
    drift the moment a column is retuned. */
const COLS = "grid grid-cols-[minmax(0,1fr)_56px_84px_84px_140px] items-center gap-2.5"

/** A counter bucket belongs to a hardware address, not to an address the lease
    may hand to somebody else tomorrow. A device the neighbour table never
    resolved has no MAC, and its bucket is keyed by its address instead. */
const rateKey = (d: { mac: string; ip: string }): string =>
  d.mac ? d.mac.toLowerCase().replace(/-/g, ":") : d.ip

const ALLOWED_MIN = 44

export function Network() {
  const { t } = useI18n()
  const [devices, setDevices] = useState<LanDevice[]>([])
  const [busy, setBusy] = useState(true)
  const [failed, setFailed] = useState(false)
  const [active, setActive] = useState<LanStatus | null>(null)
  const [limitFor, setLimitFor] = useState<LanDevice | null>(null)
  const [guard, setGuard] = useState<LanGuard | null>(null)
  const [routerOpen, setRouterOpen] = useState(false)
  const [routerLinked, setRouterLinked] = useState(false)
  const [driverReady, setDriverReady] = useState<boolean | null>(null)
  const [watching, setWatching] = useState(false)
  // Which device rides the tunnel. Read back from the backend, never assumed:
  // a claim can be refused and the badge must not lie about it.
  const [routed, setRouted] = useState<{ ip: string; mac: string } | null>(null)
  const [routingIp, setRoutingIp] = useState<string | null>(null)
  const [routeErr, setRouteErr] = useState("")
  const [err, setErr] = useState("")
  const [dests, setDests] = useState<Destination[]>([])
  const [live, setLive] = useState<Record<string, Live>>({})
  const [filter, setFilter] = useState<Filter>("all")
  const [query, setQuery] = useState("")
  const [sort, setSort] = useState<Sort>("ip")
  const [copied, setCopied] = useState<string | null>(null)
  const prevTraffic = useRef<Record<string, { up: number; down: number; at: number }>>({})
  const polled = useRef(0)

  // The claim survives a screen change, so the first paint asks who holds it
  // rather than drawing every row as unrouted until something is tapped.
  useEffect(() => {
    void api
      .lanRoutedDevice()
      .then((r) => setRouted(r && r.ip ? r : null))
      .catch(() => undefined)
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

  /** Turn one snapshot's counters into rates, honouring the measured window. */
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
      state and every counter. One round trip, so nothing on screen disagrees
      with anything else on screen. */
  const pull = useCallback(async (): Promise<NetworkSnapshot | null> => {
    let snap: NetworkSnapshot
    try {
      snap = await api.networkSnapshot()
    } catch {
      // Leave the last reading up: a failed poll is not evidence that the
      // driver vanished or the watch stopped.
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

  /** Route one device, or hand it back. The backend answers only once
      forwarding is confirmed, so a resolved call is a claim in force; when it
      refuses, the refusal is the message the user sees, word for word, and the
      badge is re-read so it cannot claim a state that is not real. */
  const toggleRoute = async (d: LanDevice) => {
    if (routingIp) return
    setRouteErr("")
    setErr("")
    setRoutingIp(d.ip)
    try {
      if (routed?.ip === d.ip) {
        await api.lanUnrouteDevice()
        setRouted(null)
        return
      }
      if (routed) await api.lanUnrouteDevice()
      await api.lanRouteDevice(d.ip, d.mac)
      setRouted({ ip: d.ip, mac: d.mac })
    } catch (e) {
      setRouteErr(String(e))
      const now = await api.lanRoutedDevice().catch(() => null)
      setRouted(now && now.ip ? now : null)
    } finally {
      setRoutingIp(null)
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
    }
  }

  const copy = async (text: string, key: string) => {
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
    setCopied(key)
    window.setTimeout(() => setCopied((c) => (c === key ? null : c)), 1400)
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

    const tick = window.setInterval(() => {
      void pull()
    }, 1500)

    // Slower than the tick on purpose: the backend refreshes its own picture of
    // the tunnel every two seconds, so asking faster only re-reads one value.
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

    // The same reading straight away, so the guard, the driver and the router
    // state are on screen at first paint, and one more a beat later so the
    // rates are a second old instead of dashes.
    void pull().then((snap) => {
      if (!alive || !snap || !snap.watching) return
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
        d.name.toLowerCase().includes(q) || d.ip.includes(q) || d.mac.toLowerCase().includes(q)
      )
    })
    if (sort === "traffic") {
      const score = (d: LanDevice) => (live[rateKey(d)]?.down ?? 0) + (live[rateKey(d)]?.up ?? 0)
      list = [...list].sort((a, b) => score(b) - score(a))
    }
    return list
  }, [devices, filter, query, sort, live])

  // The routed row, resolved against the roster the screen is showing.
  const routedDev = routed ? devices.find((d) => d.ip === routed.ip) ?? null : null
  const routedName = routedDev
    ? routedDev.name === "private"
      ? t("netPrivateName")
      : routedDev.name
    : routed?.ip ?? ""
  const routedLive = routed ? live[rateKey(routed)] : undefined

  const message = routeErr || err
  const filters: { id: Filter; label: StrKey; n: number }[] = [
    { id: "all", label: "netAllDevices", n: counts.all },
    { id: "protected", label: "netProtected", n: counts.protected },
    { id: "router", label: "netRouter", n: counts.router },
    { id: "announced", label: "netAnnounced", n: counts.announced },
  ]

  const actionCls = cn(
    "press grid place-items-center rounded-[9px] text-txt2 transition-colors",
    "enabled:hover:bg-white/[0.06] enabled:hover:text-txt disabled:cursor-not-allowed disabled:opacity-35",
  )
  const actionStyle = { width: ALLOWED_MIN, height: ALLOWED_MIN }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {guard?.attacker_mac ? (
        <div
          role="alert"
          className="flex shrink-0 items-center gap-2.5 rounded-[10px] bg-[var(--red-bg)] px-3 py-2 text-[12px] leading-snug text-[var(--red)]"
        >
          <ShieldAlert className="size-3.5 shrink-0" aria-hidden />
          <span className="min-w-0 flex-1 break-words" dir="auto">
            {t("netGuardAlert").replace("{mac}", guard.attacker_mac)}
          </span>
        </div>
      ) : null}

      {/* The route stage: the one device riding the tunnel, or the invitation
          to pick one, and beside it what the tunnel is carrying. */}
      <section className="shrink-0 overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <div className="grid grid-cols-[minmax(0,1fr)_292px] items-stretch">
          <div className="flex min-w-0 flex-col justify-center gap-2 px-5 py-3.5">
            {routed ? (
              <div className="flex min-w-0 items-center gap-3">
                <span className="grid size-10 shrink-0 place-items-center rounded-full bg-[var(--green-bg)] text-[var(--green)]">
                  <Route className="size-[18px]" aria-hidden />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-baseline gap-2">
                    <span className="truncate font-display text-[17px] font-semibold text-txt" dir="auto">
                      {routedName}
                    </span>
                    <span className="shrink-0 text-[11px] font-medium text-[var(--green)]">
                      {t("netViaTunnel")}
                    </span>
                  </div>
                  <div className="mt-0.5 flex min-w-0 items-center gap-2 text-[11.5px] text-txt3">
                    <span className="truncate font-mono" dir="ltr">
                      {routed.ip}
                    </span>
                    {routed.mac ? (
                      <>
                        <span className="shrink-0" aria-hidden>
                          ·
                        </span>
                        <span className="truncate font-mono" dir="ltr">
                          {routed.mac}
                        </span>
                      </>
                    ) : null}
                    <span className="shrink-0" aria-hidden>
                      ·
                    </span>
                    <span className="shrink-0 font-mono tabular-nums" dir="ltr">
                      <span className="text-[var(--green)]">
                        {watching && routedLive && !routedLive.stale
                          ? splitRate(routedLive.down)[0]
                          : "-"}
                      </span>{" "}
                      {watching && routedLive && !routedLive.stale
                        ? splitRate(routedLive.down)[1]
                        : ""}{" "}
                      ↓
                    </span>
                    <span className="shrink-0 font-mono tabular-nums" dir="ltr">
                      <span className="text-brand-strong">
                        {watching && routedLive && !routedLive.stale
                          ? splitRate(routedLive.up)[0]
                          : "-"}
                      </span>{" "}
                      {watching && routedLive && !routedLive.stale
                        ? splitRate(routedLive.up)[1]
                        : ""}{" "}
                      ↑
                    </span>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => routedDev && void toggleRoute(routedDev)}
                  disabled={routingIp !== null}
                  style={{ height: ALLOWED_MIN }}
                  className="press shrink-0 rounded-[9px] bg-[var(--green-bg)] px-3.5 text-[12.5px] font-medium text-[var(--green)] transition-colors enabled:hover:bg-[rgb(95_176_140/0.18)] disabled:opacity-50"
                >
                  {routingIp ? (
                    <Loader2 className="size-4 animate-spin" aria-hidden />
                  ) : (
                    t("netUnroute")
                  )}
                </button>
              </div>
            ) : (
              <div className="flex min-w-0 items-center gap-3">
                <span className="grid size-10 shrink-0 place-items-center rounded-full bg-[var(--brand-bg)] text-brand-strong">
                  <Route className="size-[18px]" aria-hidden />
                </span>
                <div className="min-w-0">
                  <div className="text-[15px] font-semibold text-txt">{t("netRoute")}</div>
                  <p className="mt-0.5 max-w-[54ch] text-[12px] leading-snug text-txt2">
                    {t("netViaTunnelHint")}
                  </p>
                </div>
              </div>
            )}

            {/* A refusal is a message from the backend, printed as it arrived. */}
            {message ? (
              <div
                role="alert"
                className="flex min-w-0 items-start gap-2 rounded-[8px] bg-[var(--red-bg)] px-2.5 py-2 text-[12px] leading-snug text-[var(--red)]"
              >
                <ShieldAlert className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                <span className="min-w-0 flex-1 break-words" dir="auto">
                  {message}
                </span>
                <button
                  type="button"
                  onClick={() => {
                    setRouteErr("")
                    setErr("")
                  }}
                  aria-label={t("netRefresh")}
                  style={actionStyle}
                  className={cn(actionCls, "-my-1 shrink-0")}
                >
                  <X className="size-3.5" aria-hidden />
                </button>
              </div>
            ) : null}
          </div>

          {/* Beside the stage: what the tunnel is carrying, and where the names
              on the roster come from. */}
          <div className="flex min-w-0 flex-col justify-center gap-1.5 border-l border-line px-4 py-3">
            <div className="min-w-0">
              <div className="text-[11px] text-txt3">{t("netDestinations")}</div>
              {dests.length ? (
                <ul className="mt-1 flex min-w-0 flex-col gap-0.5">
                  {dests.slice(0, 3).map((d, i) => (
                    <li key={`${d.host || d.process}-${i}`} className="flex min-w-0 items-center gap-2">
                      <span className="truncate text-[12px] text-txt2" dir="auto" title={d.process || d.host}>
                        {d.host || d.process || "-"}
                      </span>
                      <span className="ms-auto shrink-0 font-mono text-[11.5px] tabular-nums text-txt3" dir="ltr">
                        {fmtBytes((d.up || 0) + (d.down || 0))}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="mt-1 truncate text-[12px] text-txt3">{t("netDestinationsEmpty")}</p>
              )}
            </div>
            <button
              type="button"
              onClick={() => setRouterOpen(true)}
              style={{ height: ALLOWED_MIN }}
              className="press flex min-w-0 items-center gap-2 rounded-[9px] px-1 text-[11.5px] text-txt2 transition-colors hover:text-txt"
            >
              <Router className="size-3.5 shrink-0" aria-hidden />
              <span className="min-w-0 flex-1 truncate text-start">
                {routerLinked ? t("routerLinked") : t("routerNames")}
              </span>
              {routerLinked ? (
                <span className="size-1.5 shrink-0 rounded-full bg-[var(--green)]" aria-hidden />
              ) : null}
              <Check className={cn("size-3.5 shrink-0", routerLinked ? "text-[var(--green)]" : "invisible")} aria-hidden />
            </button>
          </div>
        </div>
      </section>

      {driverReady === false ? (
        <div className="flex shrink-0 items-center gap-2.5 rounded-[10px] bg-[var(--brand-bg)] px-3 py-1.5">
          <Info className="size-4 shrink-0 text-brand-strong" aria-hidden />
          <span className="min-w-0 flex-1 text-[12px] leading-snug text-txt2" dir="auto">
            {t("netDriverBody")}
          </span>
          <button
            type="button"
            onClick={() => void api.openPage("https://npcap.com/#download")}
            style={{ minHeight: ALLOWED_MIN }}
            className="press shrink-0 rounded-[8px] px-2.5 text-[12px] font-medium text-brand-strong transition-colors hover:bg-white/[0.06]"
          >
            {t("netDriverGet")}
          </button>
        </div>
      ) : null}

      {active ? (
        <div className="flex shrink-0 items-center gap-2.5 rounded-[10px] bg-[var(--brand-bg)] px-3 py-1.5">
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
            style={{ minHeight: ALLOWED_MIN }}
            className="press flex shrink-0 items-center gap-1.5 rounded-[8px] px-2.5 text-[12px] text-txt2 transition-colors hover:bg-white/[0.06] hover:text-txt"
          >
            <X className="size-3.5" aria-hidden />
            {t("netRemove")}
          </button>
        </div>
      ) : null}

      {/* Narrowing the roster: one line, no frame, no dropdowns. */}
      <div className="flex shrink-0 flex-wrap items-center gap-1.5">
        {filters.map((f) => {
          const on = filter === f.id
          return (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              aria-pressed={on}
              style={{ height: ALLOWED_MIN }}
              className={cn(
                "press flex items-center gap-1.5 rounded-[9px] px-3 text-[12.5px] transition-colors",
                on ? "bg-[var(--brand-bg)] text-txt" : "text-txt2 hover:bg-white/[0.04] hover:text-txt",
              )}
            >
              {t(f.label)}
              <span className={cn("tabular-nums", on ? "text-brand-strong" : "text-txt3")}>{f.n}</span>
            </button>
          )
        })}
        <div className="ms-auto flex items-center gap-2">
          <div className="relative">
            <Search
              className="pointer-events-none absolute start-3 top-1/2 size-[14px] -translate-y-1/2 text-txt3"
              aria-hidden
            />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("netSearchDevices")}
              aria-label={t("netSearchDevices")}
              style={{ height: ALLOWED_MIN }}
              className="w-[236px] rounded-[9px] border border-line bg-[var(--field)] ps-9 pe-3 text-[12.5px] text-txt outline-none placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
          </div>
          <Tip text={sort === "ip" ? t("netSortBusy") : t("netSortIp")} align="end">
            <button
              type="button"
              onClick={() => setSort(sort === "ip" ? "traffic" : "ip")}
              aria-label={sort === "ip" ? t("netSortBusy") : t("netSortIp")}
              aria-pressed={sort === "traffic"}
              style={actionStyle}
              className={cn(actionCls, sort === "traffic" && "bg-[var(--brand-bg)] text-brand-strong")}
            >
              <SlidersHorizontal className="size-4" aria-hidden />
            </button>
          </Tip>
          <Tip text={busy ? t("netScanning") : t("netRefresh")} align="end">
            <button
              type="button"
              onClick={() => void scan()}
              disabled={busy}
              aria-label={t("netRefresh")}
              style={actionStyle}
              className={cn(actionCls, "disabled:cursor-wait")}
            >
              {busy ? (
                <Loader2 className="size-4 animate-spin" aria-hidden />
              ) : (
                <RefreshCw className="size-4" aria-hidden />
              )}
            </button>
          </Tip>
          <button
            type="button"
            onClick={() => void toggleWatch()}
            style={{ height: ALLOWED_MIN }}
            className={cn(
              "press shrink-0 rounded-[9px] px-3.5 text-[12.5px] font-medium transition-colors",
              watching
                ? "bg-[var(--green-bg)] text-[var(--green)] enabled:hover:bg-[rgb(95_176_140/0.18)]"
                : "bg-[var(--brand-bg)] text-brand-strong enabled:hover:bg-[rgb(44_95_216/0.2)]",
            )}
          >
            {watching ? t("netStopWatch") : t("netStartWatch")}
          </button>
        </div>
      </div>

      {/* The roster: one line per device under a column header that never moves. */}
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className={cn(COLS, "shrink-0 border-b border-line px-4 pb-2 text-[11px] text-txt3")}>
          <span className="truncate">{t("netColDevice")}</span>
          <span className="truncate text-center">{t("netViaTunnel")}</span>
          <span className="truncate text-end">{t("netDownload")}</span>
          <span className="truncate text-end">{t("netUpload")}</span>
          <span className="truncate text-end">{t("netColActions")}</span>
        </div>

        <div className="app-scroll flex min-h-0 flex-1 flex-col overflow-y-auto">
          {busy && devices.length === 0 ? (
            <div aria-hidden>
              {[0, 1, 2, 3, 4, 5].map((i) => (
                <div key={i} className={cn(COLS, "min-h-[58px] flex-1 border-b border-line px-4 py-3 last:border-b-0")}>
                  <span className="flex flex-col gap-2">
                    <span className="net-wait h-3 w-32 rounded bg-white/[0.05]" />
                    <span className="net-wait h-2.5 w-44 rounded bg-white/[0.035]" />
                  </span>
                  <span className="net-wait mx-auto h-6 w-6 rounded bg-white/[0.05]" />
                  <span className="net-wait ms-auto h-3 w-12 rounded bg-white/[0.05]" />
                  <span className="net-wait ms-auto h-3 w-12 rounded bg-white/[0.05]" />
                  <span className="net-wait ms-auto h-6 w-24 rounded bg-white/[0.05]" />
                </div>
              ))}
            </div>
          ) : failed ? (
            <div className="flex h-full flex-col items-center justify-center gap-3">
              <ShieldAlert className="size-5 text-txt3" aria-hidden />
              <span className="text-[12.5px] text-txt2">{t("netFail")}</span>
              <button
                type="button"
                onClick={() => void scan()}
                style={{ height: ALLOWED_MIN }}
                className="press rounded-[9px] px-3 text-[12.5px] text-brand-strong transition-colors hover:bg-[var(--brand-bg)]"
              >
                {t("netRefresh")}
              </button>
            </div>
          ) : shown.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2">
              <Router className="size-5 text-txt3" aria-hidden />
              <span className="text-[12.5px] text-txt2">
                {devices.length === 0 ? t("netEmpty") : t("netNoMatch")}
              </span>
            </div>
          ) : (
            <div key={`${filter}|${query}|${sort}`} className="net-list-in flex min-h-0 flex-1 flex-col">
              {shown.map((d) => {
                const state = stateOf(d)
                const rate = live[rateKey(d)]
                const fresh = watching && rate && !rate.stale ? rate : null
                const quiet = d.live === false
                const protectedRow = d.protected || d.protected_always
                const isRouted = !!routed && routed.ip === d.ip
                const src = NAME_SOURCE[d.source]
                const [dn, dnu] = fresh ? splitRate(fresh.down) : ["-", ""]
                const [un, unu] = fresh ? splitRate(fresh.up) : ["-", ""]
                return (
                  <div
                    key={d.ip}
                    className={cn(
                      COLS,
                      "net-row relative min-h-[56px] max-h-[74px] flex-1 border-b border-line px-4 py-1.5 last:border-b-0 hover:bg-white/[0.02]",
                    )}
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <span
                        role="img"
                        aria-label={quiet ? t("netQuiet") : d.name}
                        title={quiet ? t("netQuiet") : undefined}
                        className={cn(
                          "size-1.5 shrink-0 rounded-full",
                          quiet ? "bg-white/[0.18]" : "bg-[var(--green)]",
                        )}
                      />
                      <span className="truncate text-[14px] font-semibold text-txt" dir="auto">
                        {d.name === "private" ? t("netPrivateName") : d.name}
                      </span>
                      {d.self_pc ? (
                        <span className="shrink-0 rounded-[5px] bg-[var(--brand-bg)] px-1.5 py-px text-[11px] font-medium text-brand-strong">
                          {t("netThisPc")}
                        </span>
                      ) : null}
                      {state === "blocked" ? (
                        <span className="shrink-0 text-[11px] font-medium text-[var(--red)]">
                          {t("netStateBlocked")}
                        </span>
                      ) : state === "capped" ? (
                        <span className="shrink-0 text-[11px] font-medium text-[var(--amber)]">
                          {t("netStateCapped")}
                        </span>
                      ) : null}
                      <span className="flex min-w-0 items-center gap-1">
                        <button
                          type="button"
                          onClick={() => void copy(d.ip, `ip-${d.ip}`)}
                          title={t("netCopyIp")}
                          aria-label={`${t("netCopyIp")} ${d.ip}`}
                          style={{ height: ALLOWED_MIN }}
                          className="press inline-flex shrink-0 items-center gap-1 rounded-[6px] px-1 font-mono text-[11.5px] text-txt2 transition-colors hover:bg-white/[0.04] hover:text-txt"
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
                          style={{ height: ALLOWED_MIN }}
                          className="press inline-flex shrink-0 items-center gap-1 rounded-[6px] px-1 font-mono text-[11px] uppercase text-txt3 transition-colors enabled:hover:bg-white/[0.04] enabled:hover:text-txt2 disabled:cursor-default"
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
                            <span className="truncate text-[11px] text-txt3" dir="auto" title={t(src.tip)}>
                              {t(src.label)}
                            </span>
                          </>
                        ) : null}
                      </span>
                    </div>

                    {/* The one control this screen exists for, in its own column
                        under a title that says what it does. */}
                    <span className="flex justify-center">
                      <Tip text={isRouted ? t("netUnroute") : t("netRoute")}>
                        <button
                          type="button"
                          onClick={() => void toggleRoute(d)}
                          disabled={routingIp !== null}
                          aria-pressed={isRouted}
                          aria-label={isRouted ? t("netUnroute") : t("netRoute")}
                          style={actionStyle}
                          className={cn(
                            "press grid place-items-center rounded-[9px] transition-colors",
                            isRouted
                              ? "bg-[var(--green-bg)] text-[var(--green)] enabled:hover:bg-[rgb(95_176_140/0.2)]"
                              : "text-txt2 enabled:hover:bg-white/[0.06] enabled:hover:text-txt",
                            "disabled:cursor-wait disabled:opacity-60",
                          )}
                        >
                          {routingIp === d.ip ? (
                            <Loader2 className="size-4 animate-spin" aria-hidden />
                          ) : (
                            <Route className="size-4" aria-hidden />
                          )}
                        </button>
                      </Tip>
                    </span>

                    <span className="flex items-baseline justify-end gap-1">
                      <span
                        className={cn(
                          "font-display text-[15px] font-semibold tabular-nums",
                          fresh ? "text-[var(--green)]" : "text-txt3",
                        )}
                      >
                        {dn}
                      </span>
                      <span className="text-[11px] text-txt3">{dnu}</span>
                    </span>
                    <span className="flex items-baseline justify-end gap-1">
                      <span
                        className={cn(
                          "font-display text-[15px] font-semibold tabular-nums",
                          fresh ? "text-brand-strong" : "text-txt3",
                        )}
                      >
                        {un}
                      </span>
                      <span className="text-[11px] text-txt3">{unu}</span>
                    </span>

                    <div className="flex items-center justify-end gap-0.5">
                      <Tip text={t("netLimit")} align="end">
                        <button
                          type="button"
                          onClick={() => setLimitFor(d)}
                          disabled={driverReady === false || protectedRow}
                          aria-label={t("netLimit")}
                          style={actionStyle}
                          className={cn(
                            actionCls,
                            (state === "capped" || protectedRow) && "text-txt3",
                            state === "capped" && "text-[var(--amber)]",
                          )}
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
                          style={actionStyle}
                          className={cn(actionCls, state === "blocked" && "text-[var(--red)]")}
                        >
                          <Ban className="size-4" aria-hidden />
                        </button>
                      </Tip>
                      {d.protected_always ? (
                        <Tip text={t("netProtected")} align="end">
                          <span
                            role="img"
                            aria-label={t("netProtected")}
                            style={actionStyle}
                            className="grid place-items-center text-[var(--green)]"
                          >
                            <ShieldCheck className="size-4" aria-hidden />
                          </span>
                        </Tip>
                      ) : (
                        <Tip text={d.protected ? t("netUnprotect") : t("netProtect")} align="end">
                          <button
                            type="button"
                            onClick={() => void toggleProtect(d)}
                            aria-pressed={d.protected}
                            aria-label={d.protected ? t("netUnprotect") : t("netProtect")}
                            style={actionStyle}
                            className={cn(actionCls, d.protected && "text-[var(--green)]")}
                          >
                            <ShieldCheck className="size-4" aria-hidden />
                          </button>
                        </Tip>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
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
