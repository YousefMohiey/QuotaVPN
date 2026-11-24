import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Ban,
  Check,
  ChevronRight,
  Copy,
  Gamepad2,
  Laptop,
  Loader2,
  LockKeyhole,
  Monitor,
  Phone,
  Printer,
  Radar,
  RefreshCw,
  Router as RouterIcon,
  ShieldCheck,
  Tablet,
  Tv,
  Unlock,
  X,
} from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
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
 * One roster, one detail rail, one act at a time. Discovery reads what the
 * machine and the router already know; the 254-address sweep is a headless
 * diagnostic, not a page that makes people wait. Control is explicit: the
 * selected device is claimed, its traffic passes this PC, and Release hands it
 * straight back.
 */

const keyOf = (d: { mac: string; ip: string }) => (d.mac ? d.mac.toLowerCase() : d.ip)

function fmtRate(bytes: number): [string, string] {
  if (bytes < 1024) return [String(Math.max(0, Math.round(bytes))), "B/s"]
  if (bytes < 1024 * 1024) return [String(Math.round(bytes / 1024)), "KB/s"]
  return [(bytes / (1024 * 1024)).toFixed(1), "MB/s"]
}

function fmtBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B"
  if (n < 1024) return `${Math.round(n)} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

const ipRank = (ip: string) => {
  const parts = ip.split(".").map((p) => Number(p) || 0)
  return ((parts[0] << 24) >>> 0) + (parts[1] << 16) + (parts[2] << 8) + parts[3]
}

function iconFor(d: LanDevice) {
  if (d.self_pc) return Monitor
  if (d.protected_always) return RouterIcon
  const n = `${d.name} ${d.source}`.toLowerCase()
  if (/ipad|tablet|tab\b/.test(n)) return Tablet
  if (/tv|haier|bravia|roku|fire|chromecast|shield/.test(n)) return Tv
  if (/ps[45]|playstation|xbox|switch|steam/.test(n)) return Gamepad2
  if (/printer|eprint|officejet|deskjet|laserjet/.test(n)) return Printer
  if (/iphone|android|pixel|galaxy|phone|redmi|oppo|vivo|honor|sm-|s2[0-9]|a5[0-9]|a3[0-9]|v\d{4}|2514/.test(n)) return Phone
  if (/laptop|dell|thinkpad|notebook|macbook/.test(n)) return Laptop
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
      {done ? <Check className="net-check-in size-3.5 text-[var(--green)]" aria-hidden /> : <Copy className="size-3.5" aria-hidden />}
    </button>
  )
}

/** One live reading that answers a new value with a short fade instead of a jump. */
function Rate({ value, unit }: { value: string; unit: string }) {
  const reduce = useReducedMotion()
  return (
    <span className="flex items-baseline gap-1">
      <motion.span
        key={value}
        initial={reduce ? false : { opacity: 0.35, y: 3 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.18, ease: "easeOut" }}
        className="inline-block font-mono text-[20px] text-txt"
      >
        {value}
      </motion.span>
      <span className="text-[12px] text-txt3">{unit}</span>
    </span>
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
              <p className="mt-0.5 text-[12px] uppercase tracking-[0.16em] text-txt3">{t("netNamesTitle")}</p>
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
          <p className="mt-3 flex items-start gap-2 text-[12px] leading-relaxed text-txt3">
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
  const reduce = useReducedMotion()
  const [devices, setDevices] = useState<LanDevice[]>([])
  const [snap, setSnap] = useState<NetworkSnapshot | null>(null)
  const [active, setActive] = useState<LanStatus | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [scanning, setScanning] = useState(true)
  const [routerOpen, setRouterOpen] = useState(false)
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
  const [scanBusy, setScanBusy] = useState(false)
  const lastRate = useRef<{ t: number; up: number; down: number } | null>(null)
  const activeRef = useRef<LanStatus | null>(null)
  activeRef.current = active

  const load = useCallback(async (silent = false) => {
    if (!silent) {
      setScanning(true)
      setError("")
      // Paint the neighbour table before the router round trip. The snapshot
      // then replaces it with the full roster. A session keeps its own row, so
      // skip this shortcut while one is active.
      if (!activeRef.current) {
        try {
          const known = await api.lanKnown()
          if (known.length) setDevices(known)
        } catch {
          /* the snapshot reports its own failure */
        }
      }
    }
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
      if (!silent) setError(String(e))
    } finally {
      if (!silent) setScanning(false)
    }
  }, [])

  const scan = useCallback(async () => {
    if (scanBusy) return
    setScanBusy(true)
    try {
      await api.lanScan()
      await load(true)
    } catch (e) {
      setError(String(e))
    } finally {
      setScanBusy(false)
    }
  }, [load, scanBusy])

  const refresh = useCallback(() => {
    void load(false)
    void scan()
  }, [load, scan])

  useEffect(() => {
    let alive = true
    void load(false)
    void api
      .lanWire()
      .then((w) => {
        if (alive) setWire(w)
      })
      .catch(() => undefined)
    void api
      .lanPrepare()
      .then((p) => {
        if (alive) setSetup(p)
      })
      .catch((e) => {
        if (alive) setDriverMsg(String(e))
      })
    return () => {
      alive = false
    }
  }, [load])

  // Presence is cheap now, so the roster stays current without a sweep. The
  // silent tick never clears a visible error and never spins the button.
  useEffect(() => {
    const id = window.setInterval(() => void load(true), 20000)
    return () => window.clearInterval(id)
  }, [load])

  useEffect(() => {
    const id = window.setInterval(() => {
      void api
        .lanStatus()
        .then((st) => setActive(st))
        .catch(() => undefined)
    }, 2000)
    return () => window.clearInterval(id)
  }, [])

  useEffect(() => {
    if (!active || active.blocked) {
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

  const visible = useMemo(
    () =>
      [...devices].sort((a, b) => {
        if (!!a.live !== !!b.live) return a.live ? -1 : 1
        return ipRank(a.ip) - ipRank(b.ip)
      }),
    [devices],
  )

  const selectedDevice = useMemo(() => devices.find((d) => keyOf(d) === selected) ?? null, [devices, selected])
  const activeHere = !!(active && selectedDevice && (active.ip === selectedDevice.ip || active.mac === selectedDevice.mac))
  const activeOther = !!active && !activeHere
  const ready = !!(snap?.claim_ready || setup?.npcap)

  // A download carries its ACKs back. If the PC has carried a meaningful
  // download and not one upload byte, only the download half is ours.
  const uploadsBypass = !!(activeHere && active && active.up_bytes === 0 && active.down_bytes > 256 * 1024)

  const guardAlert = !!(
    snap?.guard.watching &&
    wire &&
    /^([0-9a-f]{2}:){5}[0-9a-f]{2}$/i.test(snap.guard.attacker_mac) &&
    snap.guard.attacker_mac !== wire.gateway_mac &&
    snap.guard.attacker_mac !== wire.mac
  )

  const nameOf = (d: LanDevice) => {
    if (d.self_pc) return t("netThisPc")
    if (d.protected_always) return t("netRouter")
    if (d.name && (d.source === "admin" || d.source === "self")) return d.name
    if (d.name && d.source === "router" && snap?.router_linked) return d.name
    return ""
  }

  const subtitleOf = (d: LanDevice) => {
    if (d.self_pc) return t("netWindowsPc")
    if (d.protected_always) return t("netGateway")
    const n = d.name.toLowerCase()
    if (!n) return t("netNetworkDevice")
    if (/iphone|ipad/.test(n)) return t("netIos")
    if (/s23|honor|a53|phone|android|pixel|galaxy|redmi|oppo|vivo|v\d{4}|2514/.test(n)) return t("netAndroid")
    if (/tv|roku|fire|chromecast|shield|bravia|haier/.test(n)) return t("netTv")
    if (/ps[45]|playstation|xbox|switch|steam/.test(n)) return t("netConsole")
    if (/printer|eprint|officejet|deskjet|laserjet/.test(n)) return t("netPrinter")
    if (/win|desktop|pc|laptop/.test(n)) return t("netWindowsPc")
    return t("netOther")
  }

  const statusOf = (d: LanDevice) => {
    if (d.protected) return { text: t("netProtected"), tone: "muted" as const, live: false }
    if (active && (active.ip === d.ip || active.mac === d.mac)) {
      return active.blocked
        ? { text: t("netBlocked"), tone: "bad" as const, live: false }
        : { text: t("netLive"), tone: "ok" as const, live: true }
    }
    if (d.live) return { text: t("netStatusOnline"), tone: "ok" as const, live: false }
    return { text: t("netStatusIdle"), tone: "muted" as const, live: false }
  }

  const selectedName = selectedDevice ? nameOf(selectedDevice) : ""
  const selectedSubtitle = selectedDevice ? subtitleOf(selectedDevice) : ""
  const selectedStatus = selectedDevice ? statusOf(selectedDevice) : null

  const choose = (d: LanDevice) => {
    setSelected(keyOf(d))
    setError("")
  }

  // At the narrow layout the detail sits below the roster, so selecting a row
  // has to bring the panel into view; otherwise the tap seems to do nothing.
  useEffect(() => {
    if (!selected) return
    if (typeof window === "undefined" || !window.matchMedia("(max-width: 1080px)").matches) return
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    document
      .querySelector("[data-network-detail]")
      ?.scrollIntoView({ behavior: reduceMotion ? "auto" : "smooth", block: "start" })
  }, [selected])

  // Opening a row fills the caps from the session that is already controlling
  // it. The session identity is part of the key, so the 2 s poll cannot overwrite
  // a field while the user is typing.
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
      const u = uploadsBypass ? 0 : Math.max(0, Number(up) || 0)
      if (!block && (d > 0 || u > 0)) await api.lanLimits(d, u)
      setBlocked(block)
      await load(true)
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
      const u = uploadsBypass ? 0 : Math.max(0, Number(up) || 0)
      await api.lanLimits(d, u)
      await load(true)
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
      await load(true)
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
      await load(true)
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
      await load(true)
    } catch (e) {
      setDriverMsg(String(e))
    } finally {
      setNpcapBusy(false)
    }
  }

  return (
    <div className="net-page flex h-full min-h-0 w-full flex-col gap-3">
      <header className="flex shrink-0 items-center justify-between gap-4">
        <div className="flex min-w-0 items-baseline gap-3">
          <h1 className="font-display text-[30px] font-semibold leading-none tracking-[-0.02em] text-txt">
            {t("netTitle")}
          </h1>
          <span className="text-[13px] text-txt3">{t("netCount").replace("{n}", String(devices.length))}</span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {snap?.router_linked ? (
            <>
              <span className="flex h-10 items-center gap-2 rounded-[10px] border border-[var(--green-line)] bg-[var(--green)]/[0.06] px-3 text-[13px] text-[var(--green)]">
                <RouterIcon className="size-4" aria-hidden />
                {t("netRouterSaved")}
              </span>
              <button
                type="button"
                aria-label={t("netNamesUnlink")}
                title={t("netNamesUnlink")}
                onClick={() => {
                  void api.routerUnlink().then(() => load(false))
                }}
                className="press grid size-10 place-items-center rounded-[10px] border border-line text-txt3 hover:border-[var(--red)]/40 hover:text-[var(--red)]"
              >
                <X className="size-4" aria-hidden />
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => setRouterOpen(true)}
              className="press flex h-10 items-center gap-2 rounded-[10px] border border-[var(--brand-line)]/50 bg-[var(--brand)]/[0.06] px-3 text-[13px] font-medium text-brand-strong hover:bg-[var(--brand)]/[0.12]"
            >
              <LockKeyhole className="size-4" aria-hidden />
              {t("netNamesLink")}
            </button>
          )}
          <button
            type="button"
            onClick={refresh}
            disabled={scanning}
            className="press flex h-10 items-center gap-2 rounded-[10px] border border-line px-4 text-[13px] text-txt2 hover:border-[var(--brand-line)] hover:text-txt disabled:opacity-50"
          >
            <RefreshCw className={cn("size-4", scanning && "animate-spin")} aria-hidden />
            {t("netRefresh")}
          </button>
        </div>
      </header>

      {guardAlert ? (
        <div className="flex shrink-0 items-center gap-2 rounded-[10px] border border-[var(--red)]/35 bg-[var(--red)]/10 px-3 py-2 text-[12.5px] text-[var(--red)]">
          <ShieldCheck className="size-4 shrink-0" aria-hidden />
          {t("netGuardAlert").replace("{mac}", snap?.guard.attacker_mac ?? "")}
        </div>
      ) : null}

      {error && !selectedDevice ? (
        <div className="shrink-0 rounded-[9px] border border-[var(--red)]/30 bg-[var(--red)]/[0.06] px-3 py-2 text-[12.5px] leading-snug text-[var(--red)]">
          {error}
        </div>
      ) : null}

      {scanBusy ? (
        <div className="flex shrink-0 items-center gap-2 rounded-[9px] border border-[var(--brand-line)]/50 bg-[var(--brand)]/[0.05] px-3 py-2 text-[12px] text-brand-strong">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {t("netScanningSleepers")}
        </div>
      ) : null}

      <div className="net-layout min-h-0 flex-1">
        <section className="flex min-h-0 flex-col">
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[16px] border border-line bg-[var(--panel)]">
            <div className="net-grid shrink-0 items-center gap-2 border-b border-line px-4 py-2.5 text-[12px] uppercase tracking-[0.12em] text-txt3">
              <span>{t("netColDevice")}</span>
              <span>{t("netColIp")}</span>
              <span className="net-mac">{t("netColMac")}</span>
              <span>{t("netColStatus")}</span>
              <span />
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto">
              {visible.length === 0 ? (
                <div className="flex h-full min-h-[240px] flex-col items-center justify-center gap-3 px-6 text-center">
                  <Radar className={cn("size-10 text-txt3", scanning && "net-wait")} aria-hidden />
                  <p className="text-[13.5px] text-txt2">{scanning ? t("netScanning") : t("netEmpty")}</p>
                  {!scanning ? (
                    <button
                      type="button"
                      onClick={refresh}
                      className="press text-[13px] text-brand-strong underline-offset-4 hover:underline"
                    >
                      {t("netRefresh")}
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
                      key={`${d.ip}|${d.mac}|${d.self_pc ? "self" : ""}`}
                      role="button"
                      tabIndex={0}
                      data-no-drag
                      aria-label={`${name || t("netUnknownName")}, ${d.ip}`}
                      aria-current={isSel ? "true" : undefined}
                      onClick={() => choose(d)}
                      onKeyDown={(e) => {
                        if (e.target !== e.currentTarget) return
                        if (e.key === "Enter" || e.key === " ") choose(d)
                      }}
                      className={cn(
                        "group net-grid relative cursor-pointer items-center gap-2 border-b border-line/60 px-4 py-3 transition-colors last:border-b-0",
                        isSel ? "text-txt" : "hover:bg-white/[0.025]",
                      )}
                    >
                      {isSel ? (
                        <motion.span
                          layoutId="net-selection"
                          transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 430, damping: 38 }}
                          className="pointer-events-none absolute inset-x-1.5 inset-y-1.5 rounded-[11px] bg-[var(--brand)]/[0.10] ring-1 ring-inset ring-[var(--brand-line)]/40"
                          aria-hidden
                        />
                      ) : null}
                      <div className="relative z-10 flex min-w-0 items-center gap-3">
                        <span
                          className={cn(
                            "grid size-9 shrink-0 place-items-center rounded-[10px] border",
                            isSel ? "border-[var(--brand-line)] bg-[var(--brand)]/10" : "border-line bg-white/[0.025]",
                          )}
                        >
                          <Icon className={cn("size-[18px]", isSel ? "text-brand-strong" : "text-txt2")} aria-hidden />
                        </span>
                        <span className="min-w-0">
                          <span className={cn("block truncate text-[15px] font-medium", name ? "text-txt" : "text-txt3")}>
                            {name || t("netUnknownName")}
                          </span>
                          <span className="mt-0.5 block truncate text-[12.5px] text-txt3">{subtitleOf(d)}</span>
                        </span>
                      </div>
                      <div className="relative z-10 flex min-w-0 items-center gap-1">
                        <span className="truncate font-mono text-[13px] text-txt2">{d.ip}</span>
                        <CopyButton value={d.ip} label={t("netCopyIp")} onError={() => setError(t("netCopyFailed"))} />
                      </div>
                      <div className="net-mac relative z-10 flex min-w-0 items-center gap-1">
                        <span className="truncate font-mono text-[12px] text-txt3">{d.mac || "-"}</span>
                        <CopyButton value={d.mac} label={t("netCopyMac")} onError={() => setError(t("netCopyFailed"))} />
                      </div>
                      <div className="relative z-10">
                        <span
                          className={cn(
                            "inline-flex items-center gap-1.5 text-[12px]",
                            status.tone === "bad" ? "text-[var(--red)]" : "text-txt2",
                          )}
                        >
                          <span
                            className={cn(
                              "size-1.5 rounded-full",
                              status.tone === "ok" && "bg-[var(--green)]",
                              status.tone === "bad" && "bg-[var(--red)]",
                              status.tone === "muted" && "bg-txt3/50",
                              status.live && "net-live-dot",
                            )}
                            aria-hidden
                          />
                          {status.text}
                        </span>
                      </div>
                      <span
                        className={cn(
                          "relative z-10 grid size-6 place-items-center rounded-[6px] transition-transform group-hover:translate-x-0.5",
                          isSel ? "text-brand-strong" : "text-txt3",
                        )}
                        aria-hidden
                      >
                        {isSel ? <Check className="size-4" /> : <ChevronRight className="size-4" />}
                      </span>
                    </div>
                  )
                })
              )}
            </div>
          </div>
        </section>

        <aside
          data-network-detail
          className="net-detail flex min-h-0 max-h-full flex-col self-start overflow-hidden rounded-[16px] border border-line bg-[var(--panel)]"
        >
          {!selectedDevice ? (
            <div className="flex items-start gap-4 p-5">
              <span className="relative grid size-14 shrink-0 place-items-center rounded-[16px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
                <span className="net-ping absolute inset-0 rounded-[16px] border border-[var(--brand-line)]/40" aria-hidden />
                <Radar className="size-7 text-brand-strong" aria-hidden />
              </span>
              <div className="min-w-0">
                <h2 className="font-display text-[22px] font-semibold leading-tight text-txt">{t("netPickDevice")}</h2>
                <p className="mt-1.5 text-[13px] leading-relaxed text-txt2">{t("netPickBody")}</p>
              </div>
            </div>
          ) : (
            <div className="flex min-h-0 flex-col">
              <div className="shrink-0 border-b border-line px-4 py-4">
                <div className="flex items-start justify-between gap-2">
                  <div className="flex min-w-0 items-center gap-3">
                    <span className="grid size-12 place-items-center rounded-[12px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
                      {(() => {
                        const Icon = iconFor(selectedDevice)
                        return <Icon className="size-5 text-brand-strong" aria-hidden />
                      })()}
                    </span>
                    <div className="min-w-0">
                      <div className="flex min-w-0 items-center gap-1.5">
                        <span className="truncate text-[17px] font-semibold text-txt">
                          {selectedName || t("netUnknownName")}
                        </span>
                        {selectedName ? (
                          <CopyButton
                            value={selectedName}
                            label={t("netCopyName")}
                            onError={() => setError(t("netCopyFailed"))}
                          />
                        ) : null}
                      </div>
                      <div className="mt-1 flex min-w-0 items-center gap-2">
                        <span
                          className={cn(
                            "size-1.5 shrink-0 rounded-full",
                            selectedStatus?.tone === "ok" && "bg-[var(--green)]",
                            selectedStatus?.tone === "bad" && "bg-[var(--red)]",
                            selectedStatus?.tone === "muted" && "bg-txt3/50",
                            selectedStatus?.live && "net-live-dot",
                          )}
                          aria-hidden
                        />
                        <span
                          className={cn(
                            "shrink-0 text-[12px]",
                            selectedStatus?.tone === "bad" ? "text-[var(--red)]" : "text-txt2",
                          )}
                        >
                          {selectedStatus?.text}
                        </span>
                        <span className="truncate text-[12.5px] text-txt3">{selectedSubtitle}</span>
                      </div>
                    </div>
                  </div>
                  <button
                    type="button"
                    aria-label={t("cancel")}
                    onClick={() => setSelected(null)}
                    className="press grid size-8 shrink-0 place-items-center rounded-[7px] text-txt3 hover:bg-white/[0.06] hover:text-txt"
                  >
                    <X className="size-4" aria-hidden />
                  </button>
                </div>
                {!selectedName ? (
                  <button
                    type="button"
                    onClick={() => setRouterOpen(true)}
                    className="press mt-2 flex items-center gap-1 text-[12px] text-brand-strong hover:underline"
                  >
                    <LockKeyhole className="size-3.5" aria-hidden />
                    {t("netSignInName")}
                  </button>
                ) : null}
              </div>

              <div className="net-readings shrink-0 divide-x divide-line/60 border-b border-line/60">
                <div className="group px-4 py-3">
                  <div className="text-[12px] uppercase tracking-[0.1em] text-txt3">{t("netColIp")}</div>
                  <div className="mt-1 flex items-center gap-1 font-mono text-[13px] text-txt">
                    {selectedDevice.ip}
                    <CopyButton value={selectedDevice.ip} label={t("netCopyIp")} onError={() => setError(t("netCopyFailed"))} />
                  </div>
                </div>
                <div className="group px-4 py-3">
                  <div className="text-[12px] uppercase tracking-[0.1em] text-txt3">{t("netColMac")}</div>
                  <div className="mt-1 flex items-center gap-1 font-mono text-[12.5px] text-txt">
                    {selectedDevice.mac || "-"}
                    <CopyButton value={selectedDevice.mac} label={t("netCopyMac")} onError={() => setError(t("netCopyFailed"))} />
                  </div>
                </div>
              </div>

              <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto p-4">
                {!ready ? (
                  <div className="rounded-[12px] border border-[var(--brand-line)] bg-[var(--brand)]/[0.04] px-4 py-3.5">
                    <div className="flex items-center gap-2 text-[13px] font-medium text-txt">
                      <LockKeyhole className="size-4 text-brand-strong" aria-hidden />
                      {t("netSetupTitle")}
                    </div>
                    <p className="mt-1.5 text-[12.5px] leading-relaxed text-txt2">{t("netSetupBody")}</p>
                    <div className="mt-3 flex items-center justify-between gap-2 border-b border-line/60 pb-2">
                      <span className="flex items-center gap-2 text-[13px] text-txt2">
                        <span
                          className={cn(
                            "size-1.5 rounded-full",
                            setup?.npcap || snap?.claim_ready ? "bg-[var(--green)]" : "bg-[var(--amber)]",
                          )}
                          aria-hidden
                        />
                        Npcap
                      </span>
                      {setup?.npcap || snap?.claim_ready ? (
                        <span className="text-[12px] uppercase tracking-[0.1em] text-[var(--green)]">{t("netReady")}</span>
                      ) : (
                        <button
                          type="button"
                          onClick={() => void installNpcap()}
                          disabled={npcapBusy}
                          className="press flex h-9 items-center gap-1.5 rounded-[8px] bg-[var(--brand)]/15 px-3 text-[12.5px] font-medium text-brand-strong disabled:opacity-50"
                        >
                          {npcapBusy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
                          {npcapBusy ? t("netNpcapBusy") : t("netNpcapInstall")}
                        </button>
                      )}
                    </div>
                    {!(setup?.npcap || snap?.claim_ready) ? (
                      <p className="mt-2 text-[12px] leading-snug text-txt3">{t("netNpcapHint")}</p>
                    ) : null}
                    {driverMsg ? <p className="mt-2 text-[12px] leading-snug text-txt3">{driverMsg}</p> : null}
                  </div>
                ) : null}

                {activeOther ? (
                  <p className="rounded-[10px] border border-line/70 px-3.5 py-2.5 text-[12.5px] leading-snug text-[var(--amber)]">
                    {t("netControlOther")}
                  </p>
                ) : null}

                {activeHere ? (
                  <div className="rounded-[12px] border border-line/70 bg-white/[0.02] px-4 py-3.5">
                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <div className="text-[12px] uppercase tracking-[0.1em] text-txt3">{t("netDownload")}</div>
                        <Rate value={fmtRate(rates.down)[0]} unit={fmtRate(rates.down)[1]} />
                        <div className="mt-1 font-mono text-[12px] text-txt3">
                          {fmtBytes(active?.down_bytes ?? 0)}
                        </div>
                      </div>
                      <div>
                        <div className="text-[12px] uppercase tracking-[0.1em] text-txt3">{t("netUpload")}</div>
                        <Rate value={fmtRate(rates.up)[0]} unit={fmtRate(rates.up)[1]} />
                        <div className="mt-1 font-mono text-[12px] text-txt3">
                          {fmtBytes(active?.up_bytes ?? 0)}
                        </div>
                      </div>
                    </div>
                  </div>
                ) : null}

                {!selectedDevice.protected ? (
                  <div className="grid gap-2.5 rounded-[12px] border border-line/70 bg-white/[0.02] p-3.5">
                    <div className="flex items-baseline justify-between gap-2">
                      <span className="text-[12px] font-semibold uppercase tracking-[0.12em] text-txt3">
                        {t("netLimitTitle")}
                      </span>
                      <span className="text-[11.5px] text-txt3">{t("netCapsHint")}</span>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <label className="rounded-[10px] border border-line bg-[var(--panel)] px-3 py-2 transition-colors focus-within:border-[var(--brand-line)] focus-within:bg-[var(--brand)]/[0.04]">
                        <span className="block text-[11.5px] text-txt3">{t("netDownload")}</span>
                        <span className="mt-0.5 flex items-baseline gap-1">
                          <Input
                            value={down}
                            onChange={(e) => setDown(e.target.value.replace(/[^0-9]/g, ""))}
                            inputMode="numeric"
                            placeholder="0"
                            aria-label={t("netLimitDown")}
                            className="h-6 min-w-0 flex-1 border-0 bg-transparent p-0 text-right font-mono text-[15px] text-txt focus-visible:ring-0"
                            dir="ltr"
                          />
                          <span className="shrink-0 text-[11px] text-txt3">KB/s</span>
                        </span>
                      </label>
                      <label className="rounded-[10px] border border-line bg-[var(--panel)] px-3 py-2 transition-colors focus-within:border-[var(--brand-line)] focus-within:bg-[var(--brand)]/[0.04]">
                        <span className="block text-[11.5px] text-txt3">{t("netUpload")}</span>
                        <span className="mt-0.5 flex items-baseline gap-1">
                          <Input
                            value={up}
                            onChange={(e) => setUp(e.target.value.replace(/[^0-9]/g, ""))}
                            inputMode="numeric"
                            placeholder="0"
                            aria-label={t("netLimitUp")}
                            disabled={uploadsBypass}
                            className="h-6 min-w-0 flex-1 border-0 bg-transparent p-0 text-right font-mono text-[15px] text-txt focus-visible:ring-0 disabled:opacity-40"
                            dir="ltr"
                          />
                          <span className="shrink-0 text-[11px] text-txt3">KB/s</span>
                        </span>
                      </label>
                    </div>
                    {uploadsBypass ? (
                      <p className="text-[12px] leading-snug text-[var(--amber)]">{t("netUploadBypass")}</p>
                    ) : null}
                    <button
                      type="button"
                      onClick={() => void applyCaps()}
                      disabled={!ready || busy !== null}
                      className="press flex h-9 items-center justify-center gap-1.5 rounded-[9px] bg-[var(--brand)]/15 text-[13px] font-medium text-brand-strong hover:bg-[var(--brand)]/25 disabled:opacity-50"
                    >
                      {busy === "caps" || busy === "control" ? (
                        <Loader2 className="size-3.5 animate-spin" aria-hidden />
                      ) : null}
                      {activeHere ? t("netLimitApply") : t("netLimitApplyStart")}
                    </button>
                  </div>
                ) : null}
              </div>

              <div className="shrink-0 border-t border-line p-4">
                {selectedDevice.protected ? (
                  <div className="flex items-center justify-center gap-1.5 py-1 text-[12px] text-txt3">
                    <ShieldCheck className="size-4" aria-hidden />
                    {t("netProtected")}
                  </div>
                ) : (
                  <AnimatePresence mode="wait" initial={false}>
                    <motion.div
                      key={activeHere ? "active" : "idle"}
                      initial={reduce ? false : { opacity: 0, y: 4 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={reduce ? { opacity: 0 } : { opacity: 0, y: -4 }}
                      transition={{ duration: reduce ? 0 : 0.16, ease: "easeOut" }}
                      className="flex gap-2"
                    >
                      {activeHere ? (
                        <>
                          <button
                            type="button"
                            onClick={() => void release()}
                            disabled={busy !== null}
                            className="press flex h-10 flex-1 items-center justify-center gap-1.5 rounded-[9px] border border-[var(--brand-line)] bg-[var(--brand)]/15 text-[13px] font-medium text-brand-strong hover:bg-[var(--brand)]/25 disabled:opacity-50"
                          >
                            {busy === "release" ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
                            {t("netRelease")}
                          </button>
                          <button
                            type="button"
                            onClick={() => void toggleBlock()}
                            disabled={busy !== null}
                            className={cn(
                              "press flex h-10 flex-1 items-center justify-center gap-1.5 rounded-[9px] border text-[13px] font-medium disabled:opacity-50",
                              blocked
                                ? "border-[var(--green-line)] bg-[var(--green)]/10 text-[var(--green)] hover:bg-[var(--green)]/15"
                                : "border-[var(--red)]/35 bg-[var(--red)]/[0.06] text-[var(--red)] hover:bg-[var(--red)]/12",
                            )}
                          >
                            {busy === "block" ? (
                              <Loader2 className="size-3.5 animate-spin" aria-hidden />
                            ) : blocked ? (
                              <Unlock className="size-3.5" aria-hidden />
                            ) : (
                              <Ban className="size-3.5" aria-hidden />
                            )}
                            {blocked ? t("netUnblockDevice") : t("netBlockDevice")}
                          </button>
                        </>
                      ) : (
                        <>
                          <button
                            type="button"
                            onClick={() => void startControl(false)}
                            disabled={!ready || busy !== null}
                            className="press flex h-10 flex-1 items-center justify-center gap-1.5 rounded-[9px] bg-[var(--brand)] text-[13px] font-medium text-[var(--brand-ink)] disabled:opacity-50"
                          >
                            {busy === "control" ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
                            {t("netTakeControl")}
                          </button>
                          <button
                            type="button"
                            onClick={() => void startControl(true)}
                            disabled={!ready || busy !== null}
                            className="press flex h-10 flex-1 items-center justify-center gap-1.5 rounded-[9px] border border-[var(--red)]/35 bg-[var(--red)]/[0.06] text-[13px] font-medium text-[var(--red)] hover:bg-[var(--red)]/12 disabled:opacity-50"
                          >
                            {busy === "block" ? (
                              <Loader2 className="size-3.5 animate-spin" aria-hidden />
                            ) : (
                              <Ban className="size-3.5" aria-hidden />
                            )}
                            {t("netBlockDevice")}
                          </button>
                        </>
                      )}
                    </motion.div>
                  </AnimatePresence>
                )}
                {error ? <p className="mt-2 text-[12px] leading-snug text-[var(--red)]">{error}</p> : null}
              </div>
            </div>
          )}
        </aside>
      </div>

      <RouterDialog
        open={routerOpen}
        onOpenChange={setRouterOpen}
        onLinked={() => {
          setError("")
          void load(false)
        }}
      />
    </div>
  )
}
