import { useEffect, useState } from "react"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { useI18n } from "@/lib/i18n"
import { pushCloser } from "@/lib/navStack"
import { api, type LanDevice, type LanStatus } from "@/lib/ipc"

/** Caps for one device, in KB/s. Zero means no cap, which is what the fields
    start at: the honest default is "leave this device alone". */
export function LanLimitDialog({
  device,
  current,
  open,
  onOpenChange,
  onApplied,
  onRemoved,
}: {
  device: LanDevice | null
  current?: LanStatus | null
  open: boolean
  onOpenChange: (o: boolean) => void
  onApplied: () => void
  onRemoved: () => void
}) {
  const { t } = useI18n()
  const [down, setDown] = useState("")
  const [up, setUp] = useState("")
  const [err, setErr] = useState("")

  useEffect(() => {
    if (!open) return
    setErr("")
    // Open on whatever this device is already set to, so the sheet is a
    // readout as much as a form.
    setDown(current && current.down_kbps > 0 ? String(current.down_kbps) : "")
    setUp(current && current.up_kbps > 0 ? String(current.up_kbps) : "")
    return pushCloser(() => onOpenChange(false))
  }, [open, onOpenChange, current])

  const remove = async () => {
    setErr("")
    try {
      await api.lanStop()
      onRemoved()
      onOpenChange(false)
    } catch (e) {
      setErr(String(e))
    }
  }

  const apply = async () => {
    if (!device) return
    const d = Math.max(0, Number(down) || 0)
    const u = Math.max(0, Number(up) || 0)
    setErr("")
    try {
      await api.lanStart(device.ip, device.mac, false)
      await api.lanLimits(d, u)
      onApplied()
      onOpenChange(false)
    } catch (e) {
      setErr(String(e))
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[380px]">
        <DialogTitle className="text-[16px] font-semibold">{t("netLimitTitle")}</DialogTitle>
        <p className="mt-1 text-[12.5px] leading-relaxed text-txt2" dir="auto">
          {t("netLimitBody")}
        </p>
        <div className="mt-4 flex flex-col gap-3">
          <label className="flex items-center justify-between gap-3">
            <span className="text-[13px] text-txt2">{t("netLimitDown")}</span>
            <Input
              value={down}
              onChange={(e) => setDown(e.target.value.replace(/[^0-9]/g, ""))}
              inputMode="numeric"
              placeholder="0"
              className="h-9 w-24 text-center"
              dir="ltr"
            />
          </label>
          <label className="flex items-center justify-between gap-3">
            <span className="text-[13px] text-txt2">{t("netLimitUp")}</span>
            <Input
              value={up}
              onChange={(e) => setUp(e.target.value.replace(/[^0-9]/g, ""))}
              inputMode="numeric"
              placeholder="0"
              className="h-9 w-24 text-center"
              dir="ltr"
            />
          </label>
        </div>
        {err ? <p className="mt-3 text-[12px] text-[var(--red)]">{err}</p> : null}
        <div className="mt-5 flex items-center justify-between gap-2">
          {current && (current.down_kbps > 0 || current.up_kbps > 0) ? (
            <button
              type="button"
              onClick={() => void remove()}
              className="press h-9 rounded-[9px] border border-line px-3 text-[13px] text-txt2 hover:border-[var(--red)]/45 hover:text-[var(--red)]"
            >
              {t("netLimitRemove")}
            </button>
          ) : (
            <span />
          )}
          <span className="flex gap-2">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-[9px] border border-line px-3.5 text-[13px] text-txt2 hover:bg-white/[0.03]"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void apply()}
            className="press h-9 rounded-[9px] bg-[var(--brand)] px-4 text-[13px] font-medium text-[var(--brand-ink)]"
          >
            {t("netLimitApply")}
          </button>
          </span>
        </div>
      </DialogContent>
    </Dialog>
  )
}
