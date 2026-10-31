import { useCallback, useEffect, useRef, useState } from "react"
import { Ban, Gauge, Info, Loader2, Monitor, RefreshCw, Router, ShieldAlert, Wifi, X } from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { api, type LanDevice, type LanGuard, type LanStatus } from "@/lib/ipc"
import { cn } from "@/lib/utils"
import { LanLimitDialog } from "@/components/LanLimitDialog"
import { RouterLinkDialog } from "@/components/RouterLinkDialog"

/** Who is on this network, and what is being done to them. */
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
        }
      })
    // The guard is what protects this machine from the same trick.
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
        // A saved sign in means the names come back on their own.
        const ok = await api.routerAutoLink().catch(() => false)
        if (ok) {
          setRouterLinked(true)
          void scan(true)
        }
      })
      .catch(() => undefined)
    }, 5000)
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
  }, [scan, refreshStatus])

  const stateOf = (d: LanDevice): "blocked" | "capped" | "none" => {
    if (!active || active.ip !== d.ip) return "none"
    if (active.blocked) return "blocked"
    if (active.down_kbps > 0 || active.up_kbps > 0) return "capped"
    return "none"
  }

  // A name means different things depending on where it came from, and the
  // difference matters: a router name is exact, a guess is a guess.
  const sourceChip = (d: LanDevice) => {
    if (d.self_pc) return null
    const key =
      d.source === "router" ? "netSrcRouter" : d.source === "device" ? "netSrcDevice" : d.source === "guess" ? "netSrcGuess" : ""
    if (!key) return null
    return (
      <span className="shrink-0 rounded-[5px] border border-line px-1.5 py-[1px] text-[10px] text-txt3">
        {t(key)}
      </span>
    )
  }

  const capLine = active
    ? active.blocked
      ? t("netActiveBlocked")
      : t("netActiveCapped")
          .replace("{down}", String(active.down_kbps))
          .replace("{up}", String(active.up_kbps))
    : ""

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-3">
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <div className="flex shrink-0 items-center gap-3 border-b border-line px-5 py-3.5">
          <Wifi className="size-4 shrink-0 text-txt2" aria-hidden />
          <span className="min-w-0 flex-1 truncate text-[11px] font-medium uppercase tracking-[0.1em] text-txt2">
            {t("netTitle")}
          </span>
          <span className="shrink-0 font-mono text-[11.5px] tabular-nums text-txt3">
            {busy ? t("netScanning") : t("netCount").replace("{n}", String(devices.length))}
          </span>
          <button
            type="button"
            onClick={() => setRouterOpen(true)}
            className={cn(
              "press flex h-9 shrink-0 items-center gap-1.5 rounded-[10px] border px-3 text-[13px] transition-colors",
              routerLinked
                ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-[var(--brand)]"
                : "border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
            )}
          >
            <Router className="size-3.5" aria-hidden />
            {routerLinked ? t("routerLinked") : t("routerNames")}
          </button>
          <button
            type="button"
            onClick={() => void scan()}
            disabled={busy}
            className="press flex h-9 shrink-0 items-center gap-1.5 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <RefreshCw className="size-3.5" aria-hidden />}
            {t("netRefresh")}
          </button>
        </div>

        {driverReady === false ? (
          <div className="flex shrink-0 items-center gap-3 border-b border-line bg-[var(--brand-bg)] px-5 py-2.5">
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
          <div className="flex shrink-0 items-center gap-3 border-b border-line bg-[var(--red)]/10 px-5 py-2.5">
            <ShieldAlert className="size-4 shrink-0 text-[var(--red)]" aria-hidden />
            <span className="min-w-0 flex-1 text-[12.5px] text-[var(--red)]" dir="auto">
              {t("netGuardAlert").replace("{mac}", guard.attacker_mac)}
            </span>
          </div>
        ) : null}

        {active ? (
          <div className="flex shrink-0 items-center gap-3 border-b border-line bg-[var(--brand-bg)] px-5 py-2.5">
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
          <div className="shrink-0 border-b border-line bg-[var(--red)]/10 px-5 py-2 text-[12px] text-[var(--red)]" dir="auto">
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
                return (
                  <li key={d.ip} className="flex h-[60px] items-center gap-4 border-b border-line px-5 last:border-b-0">
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
                    <span className="min-w-0 flex-1">
                      <span className="flex min-w-0 items-center gap-2">
                        <span className="truncate text-[14px] font-medium text-txt" dir="auto">
                          {d.name === "private" ? t("netPrivateName") : d.name}
                        </span>
                        {d.self_pc ? (
                          <span className="shrink-0 rounded-[5px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--brand)]">
                            {t("netThisPc")}
                          </span>
                        ) : null}
                        {sourceChip(d)}
                        {state === "blocked" ? (
                          <span className="shrink-0 rounded-[5px] border border-[var(--red)]/45 bg-[var(--red)]/10 px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--red)]">
                            {t("netStateBlocked")}
                          </span>
                        ) : null}
                        {state === "capped" ? (
                          <span className="shrink-0 rounded-[5px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--brand)]">
                            {t("netStateCapped")}
                          </span>
                        ) : null}
                      </span>
                      <span className="block truncate font-mono text-[12px] text-txt3" dir="ltr">
                        {d.ip}
                      </span>
                    </span>
                    <span className="hidden shrink-0 font-mono text-[12px] uppercase text-txt3 sm:block" dir="ltr">
                      {d.mac}
                    </span>
                    <button
                      type="button"
                      onClick={() => setLimitFor(d)}
                      disabled={driverReady === false}
                      className={cn(
                        "press flex h-8 shrink-0 items-center gap-1.5 rounded-[9px] border px-2.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
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
                      disabled={driverReady === false}
                      className={cn(
                        "press flex h-8 shrink-0 items-center gap-1.5 rounded-[9px] border px-2.5 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                        state === "blocked"
                          ? "border-[var(--red)]/50 bg-[var(--red)]/10 text-[var(--red)]"
                          : "border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
                      )}
                    >
                      <Ban className="size-3.5" aria-hidden />
                      {state === "blocked" ? t("netUnblock") : t("netBlock")}
                    </button>
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
