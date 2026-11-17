import { useCallback, useEffect, useMemo, useState } from "react"
import { Copy, Loader2, RefreshCw, Shield, ShieldCheck } from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { copyText } from "@/lib/clipboard"
import * as api from "@/lib/ipc"
import type { LanDevice } from "@/lib/ipc"
import { cn } from "@/lib/utils"

type Routed = { ip: string; mac: string } | null

// The Network page, rebuilt around the one thing this machine can actually do
// with a device: take its traffic and send it through the tunnel. Everything
// else that used to live here (capping, blocking, watching) needed a second
// packet driver that is no longer shipped, so it is gone rather than shown
// dead.
//
// Shape: a page header, one banner while a device is routed, then a single
// card holding the roster. The roster is the only scrolling region. A row is
// name + address, and its actions are the route button and the protect
// shield. Nothing is a dropdown.
export function Network() {
  const { t } = useI18n()
  const [list, setList] = useState<LanDevice[]>([])
  const [scanning, setScanning] = useState(true)
  const [scanError, setScanError] = useState("")
  const [routed, setRouted] = useState<Routed>(null)
  const [ready, setReady] = useState(true)
  const [query, setQuery] = useState("")
  const [busy, setBusy] = useState("")
  const [diag, setDiag] = useState<string[]>([])
  const [routeError, setRouteError] = useState("")
  const [copied, setCopied] = useState("")

  // Paint what the last scan remembered, immediately, so the page is never an
  // empty box while a sweep runs.
  useEffect(() => {
    let alive = true
    void api
      .lanKnown()
      .then((d) => alive && setList(d))
      .catch(() => undefined)
    void api
      .lanRoutedDevice()
      .then((r) => alive && setRouted(r))
      .catch(() => undefined)
    void api
      .lanClaimReady()
      .then((r) => alive && setReady(r))
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [])

  const scan = useCallback(async () => {
    setScanning(true)
    setScanError("")
    try {
      const d = await api.lanDevices()
      setList(d)
    } catch (e) {
      setScanError(typeof e === "string" ? e : String(e))
    } finally {
      setScanning(false)
    }
  }, [])

  useEffect(() => {
    void scan()
  }, [scan])

  const route = useCallback(
    async (d: LanDevice) => {
      setBusy(d.mac)
      setRouteError("")
      setDiag([])
      try {
        await api.lanRouteDevice(d.ip, d.mac)
        setRouted({ ip: d.ip, mac: d.mac })
        // The claim changes what the tunnel carries, so re-read the roster.
        void scan()
      } catch (e) {
        setRouteError(t("netRouteFailed"))
        const why = typeof e === "string" ? e : String(e)
        const lines = await api.lanRouteDiagnose(d.ip, d.mac).catch(() => [])
        setDiag(lines.length ? lines : [why])
      } finally {
        setBusy("")
      }
    },
    [scan, t],
  )

  const unroute = useCallback(async () => {
    setBusy("release")
    try {
      await api.lanUnrouteDevice()
      setRouted(null)
      void scan()
    } finally {
      setBusy("")
    }
  }, [scan])

  const protect = useCallback(async (d: LanDevice) => {
    setBusy(d.mac)
    try {
      await api.lanProtect(d.mac, !d.protected)
      setList((rows) =>
        rows.map((r) => (r.mac === d.mac ? { ...r, protected: !r.protected } : r)),
      )
    } finally {
      setBusy("")
    }
  }, [])

  const copy = useCallback(async (text: string) => {
    const ok = await copyText(text).catch(() => false)
    if (!ok) return
    setCopied(text)
    window.setTimeout(() => setCopied(""), 1200)
  }, [])

  // This PC first, then the routed device, then by address, so the list does
  // not reshuffle between scans.
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
        const ar = routed?.mac === a.mac ? 0 : 1
        const br = routed?.mac === b.mac ? 0 : 1
        if (ar !== br) return ar - br
        return ipNum(a.ip) - ipNum(b.ip)
      })
  }, [list, query, routed])

  const routedName = useMemo(() => {
    if (!routed) return ""
    return list.find((d) => d.mac === routed.mac)?.name || routed.ip
  }, [list, routed])

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-3">
      <div className="flex shrink-0 items-end justify-between gap-4">
        <div className="min-w-0">
          <h1 className="text-[19px] font-semibold tracking-[-0.01em] text-txt">
            {t("netTitle")}
          </h1>
          <p className="mt-0.5 truncate text-[12.5px] text-txt2">{t("netSub")}</p>
        </div>
        <button
          onClick={() => void scan()}
          disabled={scanning}
          className="press flex h-9 shrink-0 items-center gap-2 rounded-[7px] border border-line px-3 text-[12.5px] text-txt2 transition-colors hover:bg-white/[0.04] hover:text-txt disabled:opacity-50"
        >
          {scanning ? (
            <Loader2 className="size-3.5 animate-spin" strokeWidth={2} aria-hidden />
          ) : (
            <RefreshCw className="size-3.5" strokeWidth={2} aria-hidden />
          )}
          {scanning ? t("netScanning") : t("netRefresh")}
        </button>
      </div>

      {routed && (
        <div className="flex shrink-0 items-center justify-between gap-4 rounded-[var(--r-card)] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-5 py-3.5">
          <p className="min-w-0 truncate text-[13px] text-txt">
            <span className="font-semibold">{routedName}</span>{" "}
            <span className="text-txt2">{t("netRoutedTail")}</span>
          </p>
          <button
            onClick={() => void unroute()}
            disabled={busy === "release"}
            className="press h-8 shrink-0 rounded-[7px] border border-line px-3 text-[12px] text-txt2 transition-colors hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)] disabled:opacity-50"
          >
            {t("netUnroute")}
          </button>
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
                const isRouted = routed?.mac === d.mac
                const locked = d.self_pc || d.protected_always
                return (
                  <li
                    key={`${d.mac}-${d.ip}`}
                    className={cn(
                      "flex items-center gap-4 border-b border-line/60 px-5 py-3 last:border-b-0",
                      isRouted && "bg-[var(--brand-bg)]",
                    )}
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-[13px] font-medium text-txt">
                          {d.name && d.name !== "-" ? d.name : t("netPrivateName")}
                        </span>
                        {d.self_pc && <Tag>{t("netThisPc")}</Tag>}
                        {d.protected && !d.self_pc && <Tag>{t("netProtected")}</Tag>}
                        {isRouted && <Tag brand>{t("netViaTunnel")}</Tag>}
                      </div>
                      <div className="mt-0.5 flex items-center gap-3 text-[11.5px] text-txt3">
                        <button
                          onClick={() => void copy(d.ip)}
                          className="press inline-flex items-center gap-1 font-mono tabular-nums transition-colors hover:text-txt2"
                          aria-label={t("netCopyIp")}
                        >
                          {d.ip}
                          <Copy className="size-3 opacity-0 transition-opacity group-hover:opacity-100" aria-hidden />
                        </button>
                        <button
                          onClick={() => void copy(d.mac.toUpperCase())}
                          className="press truncate font-mono transition-colors hover:text-txt2"
                          aria-label={t("netCopyMac")}
                        >
                          {d.mac.toUpperCase()}
                        </button>
                        {copied === d.ip || copied === d.mac.toUpperCase() ? (
                          <span className="text-brand-strong">{t("copied")}</span>
                        ) : null}
                      </div>
                    </div>

                    <button
                      onClick={() => void protect(d)}
                      disabled={!!busy || locked}
                      title={d.protected ? t("netUnprotect") : t("netProtect")}
                      aria-label={d.protected ? t("netUnprotect") : t("netProtect")}
                      aria-pressed={d.protected}
                      className={cn(
                        "press grid size-8 shrink-0 place-items-center rounded-[7px] border transition-colors",
                        d.protected
                          ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                          : "border-line text-txt3 hover:text-txt2",
                        locked && "invisible",
                      )}
                    >
                      {d.protected ? (
                        <ShieldCheck className="size-4" strokeWidth={1.9} aria-hidden />
                      ) : (
                        <Shield className="size-4" strokeWidth={1.9} aria-hidden />
                      )}
                    </button>

                    {isRouted ? (
                      <button
                        onClick={() => void unroute()}
                        disabled={!!busy}
                        className="press h-8 w-[118px] shrink-0 rounded-[7px] border border-line text-[12px] text-txt2 transition-colors hover:bg-white/[0.05] disabled:opacity-50"
                      >
                        {t("netUnroute")}
                      </button>
                    ) : (
                      <button
                        onClick={() => void route(d)}
                        disabled={!!busy || locked || !ready}
                        title={
                          locked
                            ? t("netCannotSelf")
                            : !ready
                              ? t("netCannotNpcap")
                              : t("netViaTunnelHint")
                        }
                        className="press h-8 w-[118px] shrink-0 rounded-[7px] border border-[var(--brand-line)] text-[12px] text-brand-strong transition-colors hover:bg-[var(--brand-bg)] disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
                      >
                        {busy === d.mac ? t("netScanning") : t("netRoute")}
                      </button>
                    )}
                  </li>
                )
              })}
            </ul>
          )}
        </div>
      </div>

      {routeError ? (
        <div
          className="shrink-0 rounded-[var(--r-card)] border border-[var(--red-line)] bg-[var(--red-bg)] px-5 py-3"
          role="alert"
        >
          <p className="text-[12.5px] text-[var(--red)]">{routeError}</p>
          {diag.length > 0 && (
            <ul className="mt-2 space-y-1">
              {diag.map((line) => (
                <li key={line} className="font-mono text-[11.5px] text-txt2">
                  {line}
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : (
        <p className="shrink-0 px-1 text-[11.5px] text-txt3">{t("netScanNote")}</p>
      )}
    </div>
  )
}

function Tag({ children, brand }: { children: React.ReactNode; brand?: boolean }) {
  return (
    <span
      className={cn(
        "shrink-0 rounded-full border px-2 py-px text-[10.5px] leading-4",
        brand
          ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
          : "border-line text-txt3",
      )}
    >
      {children}
    </span>
  )
}
