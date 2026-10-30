import { useCallback, useEffect, useState } from "react"
import { Ban, Loader2, RefreshCw, Wifi } from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { api, type LanDevice } from "@/lib/ipc"
import { cn } from "@/lib/utils"

/** Who is on this network, as the machine can see it without a driver. */
export function Network() {
  const { t } = useI18n()
  const [devices, setDevices] = useState<LanDevice[]>([])
  const [busy, setBusy] = useState(true)
  const [failed, setFailed] = useState(false)
  const [blockedIp, setBlockedIp] = useState<string | null>(null)
  const [err, setErr] = useState("")

  const scan = useCallback(async () => {
    setBusy(true)
    setFailed(false)
    try {
      const list = await api.lanDevices()
      setDevices(list)
    } catch {
      setFailed(true)
    } finally {
      setBusy(false)
    }
  }, [])

  // What the machine already knows shows at once; the sweep fills the rest in
  // behind it, so the screen is never a spinner with nothing to read.
  // One device at a time is redirected; the app keeps that state here so the
  // row can show which one is currently cut off.
  const toggleBlock = async (ip: string) => {
    setErr("")
    try {
      if (blockedIp === ip) {
        await api.lanStop()
        setBlockedIp(null)
        return
      }
      await api.lanStart(ip, true)
      setBlockedIp(ip)
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
        if (alive) void scan()
      })
    return () => {
      alive = false
    }
  }, [scan])

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
            onClick={() => void scan()}
            disabled={busy}
            className="press flex h-9 shrink-0 items-center gap-1.5 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <RefreshCw className="size-3.5" aria-hidden />}
            {t("netRefresh")}
          </button>
        </div>

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
              {devices.map((d) => (
                <li
                  key={d.ip}
                  className="flex h-[60px] items-center gap-4 border-b border-line px-5 last:border-b-0"
                >
                  <span className="grid size-9 shrink-0 place-items-center rounded-full border border-white/[0.11] bg-[rgb(255_255_255/0.03)] text-txt2">
                    <Wifi className="size-4" aria-hidden />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[14px] font-medium text-txt" dir="auto">
                      {d.name}
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
                    onClick={() => void toggleBlock(d.ip)}
                    className={cn(
                      "press flex h-8 shrink-0 items-center gap-1.5 rounded-[9px] border px-2.5 text-[12px] transition-colors",
                      blockedIp === d.ip
                        ? "border-[var(--red)]/50 bg-[var(--red)]/10 text-[var(--red)]"
                        : "border-line bg-white/[0.02] text-txt2 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-txt",
                    )}
                  >
                    <Ban className="size-3.5" aria-hidden />
                    {blockedIp === d.ip ? t("netUnblock") : t("netBlock")}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  )
}
