import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Ban,
  Check,
  ChevronRight,
  Copy,
  Gauge,
  Gamepad2,
  Info,
  Laptop,
  Loader2,
  LockKeyhole,
  Monitor,
  Phone,
  Printer,
  Radar,
  RefreshCw,
  Router as RouterIcon,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Tablet,
  Tv,
  Unlock,
  Wifi,
  X,
} from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { api, type LanDevice, type LanPrepare, type LanStatus, type LanWire, type NetworkSnapshot } from "@/lib/ipc"
import { copyText } from "@/lib/clipboard"
import { cn } from "@/lib/utils"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { pushCloser } from "@/lib/navStack"

/**
 * The network control room.
 *
 * Discovery is deliberately passive: the sweep and the router's lease table
 * name the devices, and neither touches a packet. Control is a separate,
 * explicit act: one device is claimed at a time, its traffic passes this PC,
 * and Release hands it straight back. Watching every device at once was the
 * old design and it put the whole house behind one queue; that path is gone.
 */

const keyOf = (d: { mac: string; ip: string }) => (d.mac ? d.mac.toLowerCase() : d.ip)

function fmtRate(bytes: number): [string, string] {
  if (bytes < 1024) return [String(Math.max(0, Math.round(bytes))), "B/s"]
  if (bytes < 1024 * 1024) return [String(Math.round(bytes / 1024)), "KB/s"]
  return [(bytes / (1024 * 1024)).toFixed(1), "MB/s"]
}

const ipRank = (ip: string) => {
  const parts = ip.split(".").map((p) => Number(p) || 0)
  return ((parts[0] << 24) >>> 0) + (parts[1] << 16) + (parts[2] << 8) + parts[3]
}

function iconFor(d: LanDevice) {
  if (d.self_pc) return Monitor
  if (d.protected_always) return RouterIcon
  const n = d.name.toLowerCase()
  if (/iphone|android|pixel|galaxy|phone|redmi|oppo|vivo/.test(n)) return Phone
  if (/ipad|tablet|tab /.test(n)) return Tablet
  if (/tv|roku|fire|chromecast|shield|bravia/.test(n)) return Tv
  if (/ps[45]|playstation|xbox|switch|steam/.test(n)) return Gamepad2
  if (/printer|eprint|officejet|deskjet|laserjet/.test(n)) return Printer
  return Laptop
}

function CopyButton({ value, label, onError }: { value: string; label: string; onError: (s: string) => void }) {
  const [done, setDone] = useState(false)
  if (!value) return null
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={async (e) => {
        e.stopPropagation()
        const ok = await copyText(value)
        if (!ok) {
          onError(label)
          return
        }
        setDone(true)
        window.setTimeout(() => setDone(false), 1200)
      }}
      className="press grid size-6 place-items-center rounded-[6px] text-txt3 opacity-55 transition-opacity hover:bg-white/[0.06] hover:text-txt group-hover:opacity-100 focus-visible:opacity-100"
    >
      {done ? <Check className="size-3.5 text-[var(--green)]" aria-hidden /> : <Copy className="size-3.5" aria-hidden />}
    </button>
  )
}

function RouterDialog({
  open,
  onOpenChange,
  onLinked,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onLinked: () => void
}) {
  const { t } = useI18n()
  const [user, setUser] = useState("")
  const [pass, setPass] = useState("")
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState("")

  useEffect(() => {
    if (!open) return
    setErr("")
    setPass("")
    return pushCloser(() => onOpenChange(false))
  }, [open, onOpenChange])

  const submit = async () => {
    setErr("")
    setBusy(true)
    try {
      await api.routerLink(user.trim(), pass)
      onLinked()
      onOpenChange(false)
    } catch (e) {
      setErr(String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[420px] overflow-hidden p-0">
        <div className="relative border-b border-line bg-[linear-gradient(135deg,rgba(101,145,255,0.12),transparent_62%)] px-5 py-5">
          <div className="flex items-center gap-3">
            <span className="grid size-11 place-items-center rounded-[12px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
              <RouterIcon className="size-5 text-brand-strong" aria-hidden />
            </span>
            <div className="min-w-0">
              <DialogTitle className="text-[17px] font-semibold tracking-[-0.01em]">{t("routerTitle")}</DialogTitle>
              <p className="mt-0.5 text-[11.5px] uppercase tracking-[0.16em] text-txt3">{t("netNamesTitle")}</p>
            </div>
          </div>
        </div>
        <div className="px-5 py-4">
          <p className="text-[12.5px] leading-relaxed text-txt2" dir="auto">
            {t("routerBody")}
          </p>
          <div className="mt-4 grid gap-3">
            <label className="grid gap-1.5">
              <span className="text-[11px] uppercase tracking-[0.12em] text-txt3">{t("routerUser")}</span>
              <Input value={user} onChange={(e) => setUser(e.target.value)} className="h-9" dir="ltr" autoComplete="off" />
            </label>
            <label className="grid gap-1.5">
              <span className="text-[11px] uppercase tracking-[0.12em] text-txt3">{t("routerPass")}</span>
              <Input
                type="password"
                value={pass}
                onChange={(e) => setPass(e.target.value)}
                className="h-9"
                dir="ltr"
                autoComplete="off"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && user && pass && !busy) void submit()
                }}
              />
            </label>
          </div>
          <p className="mt-3 flex items-start gap-2 text-[11.5px] leading-relaxed text-txt3">
            <LockKeyhole className="mt-0.5 size-3.5 shrink-0" aria-hidden />
            {t("netRouterHint")}
          </p>
          {err ? <p className="mt-3 text-[12px] leading-snug text-[var(--red)]">{err}</p> : null}
        </div>
        <div className="flex items-center justify-end gap-2 border-t border-line px-5 py-3">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-[9px] border border-line px-3.5 text-[12.5px] text-txt2 hover:bg-white/[0.03]"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void submit()}
            disabled={busy || !user || !pass}
            className="press flex h-9 items-center gap-2 rounded-[9px] bg-[var(--brand)] px-4 text-[12.5px] font-medium text-[var(--brand-ink)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {busy ? t("netLoggingIn") : t("routerLink")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export function Network() {
  const { t } = useI18n()
  const [devices, setDevices] = useState<LanDevice[]>([])
  const [snap, setSnap] = useState<NetworkSnapshot | null>(null)
  const [active, setActive] = useState<LanStatus | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [query, setQuery] = useState("")
  const [filter, setFilter] = useState<"all" | "online" | "offline">("all")
  const [tab, setTab] = useState<"control" | "traffic" | "info">("control")
  const [scanning, setScanning] = useState(true)
  const [routerOpen, setRouterOpen] = useState(false)
  const [routerNotice, setRouterNotice] = useState("")
  const [error, setError] = useState("")
  const [down, setDown] = useState("")
  const [up, setUp] = useState("")
  const [blocked, setBlocked] = useState(false)
  const [busy, setBusy] = useState<string | null>(null)
  const [setup, setSetup] = useState<LanPrepare | null>(null)
  const [wire, setWire] = useState<LanWire | null>(null)
  const [driverMsg, setDriverMsg] = useState("")
  const [npcapBusy, setNpcapBusy] = useState(false)
  const [rates, setRates] = useState({ up: 0, down: 0 })
  const lastRate = useRef<{ t: number; up: number; down: number } | null>(null)

  const refresh = useCallback(async () => {
    setScanning(true)
    setError("")
    try {
      const s = await api.networkSnapshot()
      setSnap(s)
      setDevices(s.devices)
      setActive(s.active)
      setSelected((cur) => {
        if (cur && s.active && (s.active.ip === cur || s.active.mac === cur)) return cur
        if (s.active) return keyOf(s.active)
        return cur
      })
    } catch (e) {
      setError(String(e))
    } finally {
      setScanning(false)
    }
  }, [])

  /** Re-read what the neighbour table already knows. No sweep, no router wait. */
  const quickRefresh = useCallback(async () => {
    setScanning(true)
    try {
      const [known, st] = await Promise.all([api.lanKnown(), api.lanStatus()])
      setDevices(known)
      setActive(st)
      if (st) setSelected(keyOf(st))
    } catch (e) {
      setError(String(e))
    } finally {
      setScanning(false)
    }
  }, [])
  useEffect(() => {
    let alive = true
    void (async () => {
      try {
        const known = await api.lanKnown()
        if (alive) setDevices(known)
      } catch {
        /* the full snapshot reports its own failure */
      }
      void api.lanWire().then((w) => {
        if (alive) setWire(w)
      }).catch(() => undefined)
      if (alive) await refresh()
    })()
    return () => {
      alive = false
    }
  }, [refresh])

  // The driver copy is idempotent and cheap; doing it when the page opens means
  // the control panel knows whether it can act instead of guessing.
  useEffect(() => {
    let alive = true
    void (async () => {
      try {
        const p = await api.lanPrepare()
        if (alive) setSetup(p)
      } catch (e) {
        if (alive) setDriverMsg(String(e))
      }
    })()
    return () => {
      alive = false
    }
  }, [])

  // Presence and names do not need to be swept constantly. A slow refresh is
  // enough, and the press of Refresh is always available.
  useEffect(() => {
    const id = window.setInterval(() => void refresh(), 20000)
    return () => window.clearInterval(id)
  }, [refresh])

  // The active session's counters are cheap to read and are what the live
  // numbers on the right are made from.
  useEffect(() => {
    const id = window.setInterval(() => {
      void api
        .lanStatus()
        .then((st) => setActive(st))
        .catch(() => undefined)
    }, 2000)
    return () => window.clearInterval(id)
  }, [])

  // Turn two byte counters into a rate over the measured gap, so a slow tick
  // does not read as a spike.
  useEffect(() => {
    if (!active) {
      lastRate.current = null
      setRates({ up: 0, down: 0 })
      return
    }
    const now = Date.now()
    const prev = lastRate.current
    if (prev) {
      const dt = Math.max(0.2, (now - prev.t) / 1000)
      setRates({
        up: Math.max(0, (active.up_bytes - prev.up) / dt),
        down: Math.max(0, (active.down_bytes - prev.down) / dt),
      })
    }
    lastRate.current = { t: now, up: active.up_bytes, down: active.down_bytes }
  }, [active])

  const selectedDevice = useMemo(
    () => devices.find((d) => keyOf(d) === selected) ?? null,
    [devices, selected],
  )
  const activeHere = !!(
    active &&
    selectedDevice &&
    (active.ip === selectedDevice.ip || active.mac === selectedDevice.mac)
  )
  const activeOther = !!active && !activeHere
  const ready = !!(
    (snap?.driver_ready || setup?.windivert) &&
    (snap?.claim_ready || setup?.npcap)
  )
  // The guard sees our own claim as the gateway MAC moving, and after the claim
  // it can name the real router as the intruder. An alert needs a real,
  // different hardware address; an empty string is a parser miss, not a proof.
  const guardAlert = !!(
    snap?.guard.watching &&
    wire &&
    /^([0-9a-f]{2}:){5}[0-9a-f]{2}$/i.test(snap.guard.attacker_mac) &&
    snap.guard.attacker_mac !== wire.gateway_mac &&
    snap.guard.attacker_mac !== wire.mac
  )

  // Opening a row fills the caps from the session that is already controlling
  // it, so the panel is a readout before it is a form. The session identity is
  // part of the key: the 2 s status poll must not overwrite a field while the
  // user is typing.
  const capLoadedFor = useRef<string | null>(null)
  useEffect(() => {
    if (!selectedDevice) return
    if (!activeHere || !active) {
      capLoadedFor.current = null
      setBlocked(false)
      setDown("")
      setUp("")
      return
    }
    const id = `${active.ip}|${active.down_kbps}|${active.up_kbps}|${active.blocked}`
    if (capLoadedFor.current === id) return
    capLoadedFor.current = id
    setBlocked(active.blocked)
    setDown(active.down_kbps > 0 ? String(Math.round(active.down_kbps)) : "")
    setUp(active.up_kbps > 0 ? String(Math.round(active.up_kbps)) : "")
  }, [selectedDevice, activeHere, active])

  const choose = (d: LanDevice) => {
    setSelected(keyOf(d))
    setTab("control")
    setError("")
  }

  const startControl = async (block: boolean) => {
    if (!selectedDevice) return
    if (!ready) {
      setError(t("netNoPipeline"))
      return
    }
    setBusy(block ? "block" : "control")
    setError("")
    try {
      await api.lanStart(selectedDevice.ip, selectedDevice.mac, block)
      const d = Math.max(0, Number(down) || 0)
      const u = Math.max(0, Number(up) || 0)
      if (!block && (d > 0 || u > 0)) await api.lanLimits(d, u)
      setBlocked(block)
      setActive(await api.lanStatus())
      await refresh()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  const applyCaps = async () => {
    if (!selectedDevice) return
    if (!activeHere) {
      await startControl(false)
      return
    }
    setBusy("caps")
    setError("")
    try {
      const d = Math.max(0, Number(down) || 0)
      const u = Math.max(0, Number(up) || 0)
      await api.lanLimits(d, u)
      setActive(await api.lanStatus())
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  const toggleBlock = async () => {
    if (!selectedDevice) return
    if (!activeHere) {
      await startControl(true)
      return
    }
    setBusy("block")
    setError("")
    try {
      await api.lanBlock(!blocked)
      setBlocked((b) => !b)
      setActive(await api.lanStatus())
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  const release = async () => {
    setBusy("release")
    setError("")
    try {
      await api.lanStop()
      setActive(null)
      setBlocked(false)
      await refresh()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  const installNpcap = async () => {
    setNpcapBusy(true)
    setDriverMsg("")
    try {
      const msg = await api.lanInstallNpcap()
      setDriverMsg(msg)
      const p = await api.lanPrepare()
      setSetup(p)
      await refresh()
    } catch (e) {
      setDriverMsg(String(e))
    } finally {
      setNpcapBusy(false)
    }
  }

  const prepareDriver = async () => {
    setBusy("driver")
    setDriverMsg("")
    try {
      const p = await api.lanPrepare()
      setSetup(p)
      await refresh()
    } catch (e) {
      setDriverMsg(String(e))
    } finally {
      setBusy(null)
    }
  }

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    const list = devices.filter((d) => {
      if (filter === "online" && !d.live) return false
      if (filter === "offline" && d.live) return false
      if (!q) return true
      return `${d.name} ${d.ip} ${d.mac}`.toLowerCase().includes(q)
    })
    return list.sort((a, b) => {
      const aa = active && (active.ip === a.ip || active.mac === a.mac) ? 0 : 1
      const bb = active && (active.ip === b.ip || active.mac === b.mac) ? 0 : 1
      if (aa !== bb) return aa - bb
      if (!!a.live !== !!b.live) return a.live ? -1 : 1
      return ipRank(a.ip) - ipRank(b.ip)
    })
  }, [devices, query, active, filter])

  const onlineCount = devices.filter((d) => d.live).length

  const nameOf = (d: LanDevice) => {
    if (d.self_pc) return t("netThisPc")
    if (d.protected_always) return t("netRouter")
    if (d.name && (d.source === "admin" || d.source === "self")) return d.name
    // Router names are the ones the user signed in for. The backend keeps them
    // for the next login, but the screen hides them until the router session is
    // live again, which is what "names stay hidden until you sign in" means.
    if (d.name && d.source === "router" && snap?.router_linked) return d.name
    return ""
  }

  const subtitleOf = (d: LanDevice) => {
    if (d.self_pc) return t("netWindowsPc")
    if (d.protected_always) return t("netGateway")
    const n = d.name.toLowerCase()
    if (!n) return t("netNetworkDevice")
    if (/iphone|ipad/.test(n)) return t("netIos")
    if (/s23|honor|a53|phone|android|pixel|galaxy|redmi|oppo|vivo/.test(n)) return t("netAndroid")
    if (/tv|roku|fire|chromecast|shield|bravia|haier/.test(n)) return t("netTv")
    if (/ps[45]|playstation|xbox|switch|steam/.test(n)) return t("netConsole")
    if (/printer|eprint|officejet|deskjet|laserjet/.test(n)) return t("netPrinter")
    if (/win|desktop|pc|laptop/.test(n)) return t("netWindowsPc")
    return t("netOther")
  }

  const statusOf = (d: LanDevice) => {
    if (d.protected) return { text: t("netProtected"), cls: "border-line text-txt3" }
    if (active && (active.ip === d.ip || active.mac === d.mac)) {
      return active.blocked
        ? { text: t("netBlocked"), cls: "border-[var(--red)]/35 bg-[var(--red)]/10 text-[var(--red)]" }
        : { text: t("netLive"), cls: "border-[var(--green-line)] bg-[var(--green)]/10 text-[var(--green)]" }
    }
    if (d.live) return { text: t("netStatusOnline"), cls: "border-[var(--green-line)]/70 bg-[var(--green)]/[0.07] text-[var(--green)]" }
    return { text: t("netStatusIdle"), cls: "border-line text-txt3" }
  }

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-3">
      <header className="flex shrink-0 items-end justify-between gap-5">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-[10.5px] uppercase tracking-[0.26em] text-txt3">
            <Wifi className="size-3.5 text-brand-strong" aria-hidden />
            {t("netSub")}
          </div>
          <div className="mt-1 flex items-baseline gap-3">
            <h1 className="font-display text-[36px] font-semibold leading-none tracking-[-0.02em] text-txt">
              {t("netTitle")}
            </h1>
            <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[11px] text-txt3">
              {t("netCount").replace("{n}", String(devices.length))}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void quickRefresh()}
            disabled={scanning}
            className="press flex h-9 items-center gap-2 rounded-[9px] border border-line px-3.5 text-[12px] text-txt2 hover:border-[var(--brand-line)] hover:text-txt disabled:opacity-50"
          >
            <RefreshCw className={cn("size-3.5", scanning && "animate-spin")} aria-hidden />
            {t("netRefresh")}
          </button>
          <button
            type="button"
            onClick={() => void refresh()}
            disabled={scanning}
            className="press flex h-9 items-center gap-2 rounded-[9px] border border-[var(--brand-line)] bg-[var(--brand)]/15 px-3.5 text-[12px] font-medium text-brand-strong hover:bg-[var(--brand)]/25 disabled:opacity-50"
          >
            <Radar className="size-3.5" aria-hidden />
            {t("netDiscover")}
          </button>
        </div>
      </header>

      {guardAlert ? (
        <div className="flex shrink-0 items-center gap-2 rounded-[10px] border border-[var(--red)]/35 bg-[var(--red)]/10 px-3 py-2 text-[12px] text-[var(--red)]">
          <ShieldCheck className="size-4 shrink-0" aria-hidden />
          {t("netGuardAlert").replace("{mac}", snap?.guard.attacker_mac ?? "")}
        </div>
      ) : null}

      <div className="flex shrink-0 items-center justify-between gap-3 rounded-[12px] border border-line bg-[var(--panel)] px-4 py-2.5">
        <div className="flex min-w-0 items-center gap-3">
          <span className={cn("grid size-8 place-items-center rounded-[9px] border", snap?.router_linked ? "border-[var(--green-line)] bg-[var(--green)]/10" : "border-line bg-white/[0.03]")}>
            {snap?.router_linked ? <RouterIcon className="size-4 text-[var(--green)]" aria-hidden /> : <LockKeyhole className="size-4 text-txt3" aria-hidden />}
          </span>
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-[12.5px] font-medium text-txt">
              {snap?.router_linked ? t("netRouterSaved") : t("netNamesHidden")}
              {snap?.router_linked ? <span className="size-1.5 rounded-full bg-[var(--green)]" aria-hidden /> : null}
            </div>
            <div className="mt-0.5 truncate text-[11px] text-txt3">
              {routerNotice || (snap?.router_linked ? t("netRouterHint") : t("netNamesBody"))}
            </div>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {snap?.router_linked ? (
            <button
              type="button"
              onClick={() => {
                void api.routerUnlink().then(() => refresh())
              }}
              className="press h-8 rounded-[8px] border border-line px-2.5 text-[11.5px] text-txt3 hover:text-txt"
            >
              {t("netNamesUnlink")}
            </button>
          ) : (
            <button
              type="button"
              onClick={() => {
                setRouterNotice("")
                setRouterOpen(true)
              }}
              className="press flex h-8 items-center gap-1.5 rounded-[8px] bg-[var(--brand)]/15 px-2.5 text-[11.5px] font-medium text-brand-strong hover:bg-[var(--brand)]/25"
            >
              <LockKeyhole className="size-3" aria-hidden />
              {t("netNamesLink")}
              <ChevronRight className="size-3" aria-hidden />
            </button>
          )}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.55fr)_minmax(320px,0.9fr)] gap-3">
        <section className="flex min-h-0 flex-col gap-3">
          <div className="flex min-h-0 flex-col overflow-hidden rounded-[14px] border border-line bg-[var(--panel)]">
            <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-line px-3 py-2.5">
              <span className="relative flex min-w-[190px] flex-1 items-center">
                <Search className="pointer-events-none absolute left-2.5 size-3.5 text-txt3" aria-hidden />
                <Input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder={t("netSearchDevices")}
                  className="h-8 border-0 bg-white/[0.04] pl-8 text-[12px] focus-visible:ring-1"
                />
              </span>
              <div className="flex items-center gap-0.5 rounded-[8px] border border-line bg-white/[0.03] p-0.5">
                {(
                  [
                    { id: "all" as const, label: t("netAll"), count: devices.length },
                    { id: "online" as const, label: t("netStatusOnline"), count: onlineCount },
                    { id: "offline" as const, label: t("netOffline"), count: devices.length - onlineCount },
                  ] as const
                ).map((f) => (
                  <button
                    key={f.id}
                    type="button"
                    onClick={() => setFilter(f.id)}
                    className={cn(
                      "press h-6 rounded-[6px] px-2 text-[10.5px] font-medium",
                      filter === f.id ? "bg-[var(--brand)]/18 text-brand-strong" : "text-txt3 hover:text-txt",
                    )}
                  >
                    {f.label} ({f.count})
                  </button>
                ))}
              </div>
            </div>

            <div className="grid shrink-0 grid-cols-[minmax(0,1fr)_118px_146px_98px_28px] items-center gap-2 border-b border-line px-3 py-1.5 text-[9.5px] uppercase tracking-[0.14em] text-txt3">
              <span>{t("netColDevice")}</span>
              <span>{t("netColIp")}</span>
              <span>{t("netColMac")}</span>
              <span>{t("netColStatus")}</span>
              <span />
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto">
              {visible.length === 0 ? (
                <div className="flex h-full min-h-[220px] flex-col items-center justify-center gap-3 px-6 text-center">
                  <Radar className="size-9 text-txt3" aria-hidden />
                  <p className="text-[13px] text-txt2">{query ? t("netNoMatch") : scanning ? t("netScanning") : t("netEmpty")}</p>
                  {!scanning ? (
                    <button type="button" onClick={() => void refresh()} className="press text-[12px] text-brand-strong underline-offset-4 hover:underline">
                      {t("netDiscover")}
                    </button>
                  ) : null}
                </div>
              ) : (
                visible.map((d) => {
                  const Icon = iconFor(d)
                  const name = nameOf(d)
                  const k = keyOf(d)
                  const isSel = selected === k
                  const status = statusOf(d)
                  return (
                    <div
                      key={k}
                      role="button"
                      tabIndex={0}
                      onClick={() => choose(d)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") choose(d)
                      }}
                      className={cn(
                        "group grid cursor-pointer grid-cols-[minmax(0,1fr)_118px_146px_98px_28px] items-center gap-2 border-b border-line/60 px-3 py-2 transition-colors last:border-b-0",
                        isSel ? "bg-[var(--brand)]/[0.075]" : "hover:bg-white/[0.025]",
                      )}
                    >
                      <div className="flex min-w-0 items-center gap-2.5">
                        <span className={cn("relative grid size-8 shrink-0 place-items-center rounded-[9px] border", d.live ? "border-[var(--green-line)]/70 bg-[var(--green)]/[0.07]" : "border-line bg-white/[0.025]")}>
                          <Icon className={cn("size-4", d.live ? "text-txt" : "text-txt3")} aria-hidden />
                          <span className={cn("absolute -bottom-0.5 -right-0.5 size-2 rounded-full ring-2 ring-[var(--panel)]", d.live ? "bg-[var(--green)]" : "bg-txt3/50")} aria-hidden />
                        </span>
                        <span className="min-w-0">
                          <span className="flex min-w-0 items-center gap-1.5">
                            {name ? (
                              <span className="truncate text-[12.5px] font-medium text-txt">{name}</span>
                            ) : (
                              <span className="truncate text-[12.5px] text-txt3">{t("netUnknownName")}</span>
                            )}
                            {name ? <CopyButton value={name} label={t("netCopyName")} onError={() => setError(t("netCopyFailed"))} /> : null}
                          </span>
                          <span className="mt-0.5 block truncate text-[10.5px] text-txt3">{subtitleOf(d)}</span>
                        </span>
                      </div>
                      <div className="flex min-w-0 items-center gap-1">
                        <span className="truncate font-mono text-[11px] text-txt2">{d.ip}</span>
                        <CopyButton value={d.ip} label={t("netCopyIp")} onError={() => setError(t("netCopyFailed"))} />
                      </div>
                      <div className="flex min-w-0 items-center gap-1">
                        <span className="truncate font-mono text-[10.5px] text-txt3">{d.mac || "-"}</span>
                        <CopyButton value={d.mac} label={t("netCopyMac")} onError={() => setError(t("netCopyFailed"))} />
                      </div>
                      <div>
                        <span className={cn("inline-flex rounded-full border px-2 py-0.5 text-[9.5px] font-medium", status.cls)}>
                          {status.text}
                        </span>
                      </div>
                      <button
                        type="button"
                        aria-label={t("netControl")}
                        onClick={(e) => {
                          e.stopPropagation()
                          choose(d)
                        }}
                        className="press grid size-7 place-items-center rounded-[6px] text-txt3 hover:bg-white/[0.06] hover:text-txt"
                      >
                        <ChevronRight className="size-3.5" aria-hidden />
                      </button>
                    </div>
                  )
                })
              )}
            </div>
          </div>

          <div className="flex shrink-0 items-center justify-between rounded-[12px] border border-line bg-[var(--panel)] px-4 py-3">
            <div className="flex min-w-0 items-center gap-3">
              <span className={cn("grid size-9 place-items-center rounded-[10px] border", snap?.router_linked ? "border-[var(--green-line)] bg-[var(--green)]/10" : "border-line bg-white/[0.03]")}>
                <RouterIcon className="size-4 text-txt3" aria-hidden />
              </span>
              <div className="min-w-0">
                <div className="text-[12.5px] font-medium text-txt">{t("netRouter")}</div>
                <div className="mt-0.5 flex items-center gap-1.5 text-[10.5px] text-txt3">
                  <span className={cn("size-1.5 rounded-full", snap?.router_linked ? "bg-[var(--green)]" : "bg-txt3/50")} aria-hidden />
                  {snap?.router_linked ? t("netRouterSaved") : t("netStatusOnline")}
                </div>
              </div>
            </div>
            <div className="text-right">
              <div className="font-mono text-[13px] text-txt">{devices.length}</div>
              <div className="text-[9.5px] uppercase tracking-[0.14em] text-txt3">{t("netDevices")}</div>
            </div>
          </div>
        </section>

        <aside className="flex min-h-0 flex-col overflow-hidden rounded-[14px] border border-line bg-[var(--panel)]">
          {!selectedDevice ? (
            <div className="flex h-full flex-col justify-between p-4">
              <div>
                <div className="flex items-center gap-2 text-[10px] uppercase tracking-[0.24em] text-txt3">
                  <Radar className="size-3.5 text-brand-strong" aria-hidden />
                  {t("netControlTitle")}
                </div>
                <h2 className="mt-2 font-display text-[22px] font-semibold leading-tight text-txt">{t("netPickDevice")}</h2>
                <p className="mt-2 text-[11.5px] leading-relaxed text-txt2">{t("netControlBody")}</p>
              </div>
              <div className="grid gap-2">
                {[
                  { icon: Search, title: t("netAllDevices"), body: t("netHintSee") },
                  { icon: Gauge, title: t("netLimitTitle"), body: t("netHintCap") },
                  { icon: Ban, title: t("netHintBlockTitle"), body: t("netHintBlock") },
                ].map(({ icon: Icon, title, body }, i) => (
                  <div key={i} className="flex items-start gap-2.5 rounded-[9px] border border-line/70 bg-white/[0.02] px-3 py-2.5">
                    <Icon className="mt-0.5 size-3.5 shrink-0 text-brand-strong" aria-hidden />
                    <span className="min-w-0">
                      <span className="block text-[11.5px] font-medium text-txt">{title}</span>
                      <span className="mt-0.5 block text-[10.5px] leading-snug text-txt2">{body}</span>
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="flex h-full min-h-0 flex-col">
              <div className="shrink-0 border-b border-line px-3.5 py-3">
                <div className="flex items-start justify-between gap-2">
                  <div className="flex min-w-0 items-center gap-2.5">
                    <span className="grid size-11 place-items-center rounded-[11px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
                      {(() => {
                        const Icon = iconFor(selectedDevice)
                        return <Icon className="size-5 text-brand-strong" aria-hidden />
                      })()}
                    </span>
                    <div className="min-w-0">
                      <div className="flex min-w-0 items-center gap-1.5">
                        <span className="truncate text-[15px] font-semibold text-txt">
                          {nameOf(selectedDevice) || t("netUnknownName")}
                        </span>
                        {nameOf(selectedDevice) ? (
                          <CopyButton value={nameOf(selectedDevice)} label={t("netCopyName")} onError={() => setError(t("netCopyFailed"))} />
                        ) : null}
                      </div>
                      <div className="mt-1 flex items-center gap-1.5">
                        <span className={cn("inline-flex rounded-full border px-2 py-0.5 text-[9.5px] font-medium", statusOf(selectedDevice).cls)}>
                          {statusOf(selectedDevice).text}
                        </span>
                        <span className="truncate text-[10.5px] text-txt3">{subtitleOf(selectedDevice)}</span>
                      </div>
                    </div>
                  </div>
                  <button
                    type="button"
                    aria-label={t("cancel")}
                    onClick={() => setSelected(null)}
                    className="press grid size-7 shrink-0 place-items-center rounded-[7px] text-txt3 hover:bg-white/[0.06] hover:text-txt"
                  >
                    <X className="size-3.5" aria-hidden />
                  </button>
                </div>
                {!nameOf(selectedDevice) ? (
                  <button
                    type="button"
                    onClick={() => setRouterOpen(true)}
                    className="press mt-2 flex items-center gap-1 text-[10.5px] text-brand-strong hover:underline"
                  >
                    <LockKeyhole className="size-3" aria-hidden />
                    {t("netSignInName")}
                  </button>
                ) : null}
              </div>

              <div className="grid shrink-0 grid-cols-2 gap-2 border-b border-line px-3.5 py-2.5">
                <div className="group rounded-[8px] border border-line px-2.5 py-1.5">
                  <div className="text-[9px] uppercase tracking-[0.12em] text-txt3">{t("netColIp")}</div>
                  <div className="mt-0.5 flex items-center gap-1 font-mono text-[11.5px] text-txt">
                    {selectedDevice.ip}
                    <CopyButton value={selectedDevice.ip} label={t("netCopyIp")} onError={() => setError(t("netCopyFailed"))} />
                  </div>
                </div>
                <div className="group rounded-[8px] border border-line px-2.5 py-1.5">
                  <div className="text-[9px] uppercase tracking-[0.12em] text-txt3">{t("netColMac")}</div>
                  <div className="mt-0.5 flex items-center gap-1 font-mono text-[11px] text-txt">
                    {selectedDevice.mac || "-"}
                    <CopyButton value={selectedDevice.mac} label={t("netCopyMac")} onError={() => setError(t("netCopyFailed"))} />
                  </div>
                </div>
              </div>

              <div className="flex shrink-0 gap-1 border-b border-line px-3 py-2">
                {(
                  [
                    { id: "control" as const, icon: SlidersHorizontal, label: t("netTabControl") },
                    { id: "traffic" as const, icon: Gauge, label: t("netTabTraffic") },
                    { id: "info" as const, icon: Info, label: t("netTabInfo") },
                  ] as const
                ).map(({ id, icon: Icon, label }) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => setTab(id)}
                    className={cn(
                      "press flex h-7 flex-1 items-center justify-center gap-1.5 rounded-[6px] text-[10.5px] font-medium",
                      tab === id ? "bg-[var(--brand)]/15 text-brand-strong" : "text-txt3 hover:bg-white/[0.04] hover:text-txt",
                    )}
                  >
                    <Icon className="size-3" aria-hidden />
                    {label}
                  </button>
                ))}
              </div>

              <div className="min-h-0 flex-1 overflow-y-auto p-3.5">
                {tab === "control" ? (
                  <div className="grid gap-3">
                    {!ready ? (
                      <div className="rounded-[9px] border border-[var(--brand-line)] bg-[var(--brand)]/[0.04] px-3 py-2.5">
                        <div className="flex items-center gap-2 text-[11.5px] font-medium text-txt">
                          <LockKeyhole className="size-3.5 text-brand-strong" aria-hidden />
                          {t("netSetupTitle")}
                        </div>
                        <p className="mt-1.5 text-[10.5px] leading-relaxed text-txt2">{t("netSetupBody")}</p>
                        <div className="mt-2 grid gap-2">
                          <div className="flex items-center justify-between gap-2 rounded-[7px] border border-line/70 px-2.5 py-1.5">
                            <span className="flex items-center gap-2 text-[11px] text-txt2">
                              <span className={cn("size-1.5 rounded-full", setup?.npcap || snap?.claim_ready ? "bg-[var(--green)]" : "bg-[var(--amber,orange)]")} aria-hidden />
                              Npcap
                            </span>
                            {setup?.npcap || snap?.claim_ready ? (
                              <span className="text-[10px] uppercase tracking-[0.1em] text-[var(--green)]">{t("netReady")}</span>
                            ) : (
                              <button
                                type="button"
                                onClick={() => void installNpcap()}
                                disabled={npcapBusy}
                                className="press flex h-7 items-center gap-1.5 rounded-[6px] bg-[var(--brand)]/15 px-2 text-[10.5px] font-medium text-brand-strong disabled:opacity-50"
                              >
                                {npcapBusy ? <Loader2 className="size-3 animate-spin" aria-hidden /> : null}
                                {npcapBusy ? t("netNpcapBusy") : t("netNpcapInstall")}
                              </button>
                            )}
                          </div>
                          <div className="flex items-center justify-between gap-2 rounded-[7px] border border-line/70 px-2.5 py-1.5">
                            <span className="flex items-center gap-2 text-[11px] text-txt2">
                              <span className={cn("size-1.5 rounded-full", setup?.windivert || snap?.driver_ready ? "bg-[var(--green)]" : "bg-[var(--amber,orange)]")} aria-hidden />
                              WinDivert
                            </span>
                            {setup?.windivert || snap?.driver_ready ? (
                              <span className="text-[10px] uppercase tracking-[0.1em] text-[var(--green)]">{t("netReady")}</span>
                            ) : (
                              <button
                                type="button"
                                onClick={() => void prepareDriver()}
                                disabled={busy === "driver"}
                                className="press flex h-7 items-center gap-1.5 rounded-[6px] bg-[var(--brand)]/15 px-2 text-[10.5px] font-medium text-brand-strong disabled:opacity-50"
                              >
                                {busy === "driver" ? <Loader2 className="size-3 animate-spin" aria-hidden /> : null}
                                {t("netSetupDriver")}
                              </button>
                            )}
                          </div>
                        </div>
                        {!(setup?.npcap || snap?.claim_ready) ? (
                          <p className="mt-2 text-[10px] leading-snug text-txt3">{t("netNpcapHint")}</p>
                        ) : null}
                        {driverMsg ? <p className="mt-2 text-[10.5px] leading-snug text-txt3">{driverMsg}</p> : null}
                      </div>
                    ) : null}

                    {activeOther ? (
                      <p className="rounded-[8px] border border-line/70 px-2.5 py-2 text-[10.5px] leading-snug text-[var(--amber,orange)]">
                        {t("netControlOther")}
                      </p>
                    ) : null}

                    {activeHere ? (
                      <div className="rounded-[9px] border border-[var(--green-line)] bg-[var(--green)]/[0.06] px-3 py-2.5">
                        <span className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--green)]">
                          <span className="size-1.5 animate-pulse rounded-full bg-[var(--green)]" aria-hidden />
                          {blocked ? t("netStateBlocked") : t("netLive")}
                        </span>
                        <div className="mt-2 grid grid-cols-2 gap-2">
                          <div className="rounded-[7px] bg-black/15 px-2.5 py-1.5">
                            <div className="text-[9px] uppercase tracking-[0.12em] text-txt3">{t("netDownload")}</div>
                            <div className="mt-0.5 flex items-baseline gap-1">
                              <span className="font-mono text-[16px] text-txt">{fmtRate(rates.down)[0]}</span>
                              <span className="text-[9px] text-txt3">{fmtRate(rates.down)[1]}</span>
                            </div>
                          </div>
                          <div className="rounded-[7px] bg-black/15 px-2.5 py-1.5">
                            <div className="text-[9px] uppercase tracking-[0.12em] text-txt3">{t("netUpload")}</div>
                            <div className="mt-0.5 flex items-baseline gap-1">
                              <span className="font-mono text-[16px] text-txt">{fmtRate(rates.up)[0]}</span>
                              <span className="text-[9px] text-txt3">{fmtRate(rates.up)[1]}</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    ) : ready ? (
                      <p className="px-0.5 text-[10.5px] leading-snug text-txt2">{t("netControlBody")}</p>
                    ) : null}

                    <div className="grid gap-2">
                      <div className="flex items-center justify-between">
                        <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-txt3">{t("netLimitTitle")}</span>
                        <span className="text-[10px] text-txt3">{t("netCapsHint")}</span>
                      </div>
                      <label className="flex items-center justify-between gap-2 rounded-[8px] border border-line px-2.5 py-1.5">
                        <span className="text-[11.5px] text-txt2">{t("netDownload")}</span>
                        <span className="flex items-center gap-1.5">
                          <Input
                            value={down}
                            onChange={(e) => setDown(e.target.value.replace(/[^0-9]/g, ""))}
                            inputMode="numeric"
                            placeholder="0"
                            className="h-7 w-20 border-0 bg-transparent text-right font-mono text-[12px]"
                            dir="ltr"
                          />
                          <span className="text-[9.5px] text-txt3">KB/s</span>
                        </span>
                      </label>
                      <label className="flex items-center justify-between gap-2 rounded-[8px] border border-line px-2.5 py-1.5">
                        <span className="text-[11.5px] text-txt2">{t("netUpload")}</span>
                        <span className="flex items-center gap-1.5">
                          <Input
                            value={up}
                            onChange={(e) => setUp(e.target.value.replace(/[^0-9]/g, ""))}
                            inputMode="numeric"
                            placeholder="0"
                            className="h-7 w-20 border-0 bg-transparent text-right font-mono text-[12px]"
                            dir="ltr"
                          />
                          <span className="text-[9.5px] text-txt3">KB/s</span>
                        </span>
                      </label>
                      <div className="flex gap-2">
                        {activeHere ? (
                          <>
                            <button
                              type="button"
                              onClick={() => void applyCaps()}
                              disabled={!ready || busy !== null}
                              className="press flex h-8 flex-1 items-center justify-center gap-1.5 rounded-[7px] bg-[var(--brand)] text-[11.5px] font-medium text-[var(--brand-ink)] disabled:opacity-50"
                            >
                              {busy === "caps" ? <Loader2 className="size-3 animate-spin" aria-hidden /> : null}
                              {t("netLimitApply")}
                            </button>
                            <button
                              type="button"
                              onClick={() => void release()}
                              disabled={busy !== null}
                              className="press h-8 rounded-[7px] border border-line px-3 text-[11.5px] text-txt2 hover:border-[var(--red)]/40 hover:text-[var(--red)] disabled:opacity-50"
                            >
                              {t("netRelease")}
                            </button>
                          </>
                        ) : (
                          <button
                            type="button"
                            onClick={() => void startControl(false)}
                            disabled={!ready || busy !== null}
                            className="press flex h-8 flex-1 items-center justify-center gap-1.5 rounded-[7px] bg-[var(--brand)] text-[11.5px] font-medium text-[var(--brand-ink)] disabled:opacity-50"
                          >
                            {busy === "control" ? <Loader2 className="size-3 animate-spin" aria-hidden /> : null}
                            {t("netTakeControl")}
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                ) : tab === "traffic" ? (
                  <div className="grid gap-3">
                    {activeHere ? (
                      <>
                        <div className="grid grid-cols-2 gap-2">
                          <div className="rounded-[8px] border border-line px-2.5 py-2">
                            <div className="text-[9px] uppercase tracking-[0.12em] text-txt3">{t("netDownload")}</div>
                            <div className="mt-0.5 flex items-baseline gap-1">
                              <span className="font-mono text-[18px] text-txt">{fmtRate(rates.down)[0]}</span>
                              <span className="text-[9.5px] text-txt3">{fmtRate(rates.down)[1]}</span>
                            </div>
                            <div className="mt-1 font-mono text-[9.5px] text-txt3">{active?.down_bytes ?? 0} B total</div>
                          </div>
                          <div className="rounded-[8px] border border-line px-2.5 py-2">
                            <div className="text-[9px] uppercase tracking-[0.12em] text-txt3">{t("netUpload")}</div>
                            <div className="mt-0.5 flex items-baseline gap-1">
                              <span className="font-mono text-[18px] text-txt">{fmtRate(rates.up)[0]}</span>
                              <span className="text-[9.5px] text-txt3">{fmtRate(rates.up)[1]}</span>
                            </div>
                            <div className="mt-1 font-mono text-[9.5px] text-txt3">{active?.up_bytes ?? 0} B total</div>
                          </div>
                        </div>
                        <p className="text-[10px] leading-snug text-txt3">{t("netTrafficOnly")}</p>
                      </>
                    ) : (
                      <div className="rounded-[9px] border border-line/70 px-3 py-2.5">
                        <p className="text-[11px] leading-snug text-txt2">{t("netTrafficNone")}</p>
                        <button
                          type="button"
                          onClick={() => void startControl(false)}
                          disabled={!ready || busy !== null}
                          className="press mt-2 flex h-8 w-full items-center justify-center rounded-[7px] bg-[var(--brand)] text-[11.5px] font-medium text-[var(--brand-ink)] disabled:opacity-50"
                        >
                          {t("netTakeControl")}
                        </button>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="grid gap-0">
                    {[
                      [t("netInfoName"), nameOf(selectedDevice) || t("netUnknownName")],
                      [t("netInfoSource"), selectedDevice.self_pc ? t("netThisPc") : selectedDevice.source === "router" && snap?.router_linked ? t("netInfoRouter") : t("netInfoNotSigned")],
                      [t("netInfoPresence"), selectedDevice.live ? t("netStatusOnline") : t("netStatusIdle")],
                      [t("netInfoProtection"), selectedDevice.protected ? t("netProtected") : t("netInfoNone")],
                      [t("netInfoSession"), activeHere ? (blocked ? t("netBlocked") : t("netLive")) : t("netInfoNone")],
                      [
                        t("netLimitTitle"),
                        activeHere
                          ? down || up
                            ? `${down || "0"} / ${up || "0"} KB/s`
                            : t("netInfoUnlimited")
                          : t("netInfoNone"),
                      ],
                    ].map(([label, value], i) => (
                      <div key={i} className="flex items-start justify-between gap-3 border-b border-line/60 py-2 last:border-b-0">
                        <span className="text-[10.5px] uppercase tracking-[0.08em] text-txt3">{label}</span>
                        <span className="max-w-[60%] text-right text-[11px] text-txt">{value}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="shrink-0 border-t border-line p-3">
                {selectedDevice.protected ? (
                  <div className="flex items-center justify-center gap-1.5 py-1 text-[10.5px] text-txt3">
                    <ShieldCheck className="size-3.5" aria-hidden />
                    {t("netProtected")}
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => void toggleBlock()}
                    disabled={!ready || busy !== null}
                    className={cn(
                      "press flex h-8 w-full items-center justify-center gap-1.5 rounded-[7px] border text-[11.5px] font-medium disabled:opacity-50",
                      activeHere && blocked
                        ? "border-[var(--green-line)] bg-[var(--green)]/10 text-[var(--green)] hover:bg-[var(--green)]/15"
                        : "border-[var(--red)]/35 bg-[var(--red)]/[0.06] text-[var(--red)] hover:bg-[var(--red)]/12",
                    )}
                  >
                    {activeHere && blocked ? <Unlock className="size-3.5" aria-hidden /> : <Ban className="size-3.5" aria-hidden />}
                    {activeHere && blocked ? t("netUnblockDevice") : t("netBlockDevice")}
                  </button>
                )}
                {error ? <p className="mt-2 text-[10.5px] leading-snug text-[var(--red)]">{error}</p> : null}
              </div>
            </div>
          )}
        </aside>
      </div>

      <RouterDialog
        open={routerOpen}
        onOpenChange={setRouterOpen}
        onLinked={() => {
          setRouterNotice(t("netRouterSaved"))
          setError("")
          void refresh()
        }}
      />
    </div>
  )
}
