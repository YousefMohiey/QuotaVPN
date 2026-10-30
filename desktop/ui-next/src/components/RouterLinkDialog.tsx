import { useEffect, useState } from "react"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { useI18n } from "@/lib/i18n"
import { pushCloser } from "@/lib/navStack"
import { api } from "@/lib/ipc"

/** The router holds the one list that knows every device by name, including
    the phones that hide behind a random address. This is the way in. */
export function RouterLinkDialog({
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

  const link = async () => {
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
      <DialogContent className="max-w-[400px]">
        <DialogTitle className="text-[16px] font-semibold">{t("routerTitle")}</DialogTitle>
        <p className="mt-1 text-[12.5px] leading-relaxed text-txt2" dir="auto">
          {t("routerBody")}
        </p>
        <div className="mt-4 flex flex-col gap-3">
          <label className="flex flex-col gap-1.5">
            <span className="text-[12px] text-txt3">{t("routerUser")}</span>
            <Input value={user} onChange={(e) => setUser(e.target.value)} className="h-9" dir="ltr" autoComplete="off" />
          </label>
          <label className="flex flex-col gap-1.5">
            <span className="text-[12px] text-txt3">{t("routerPass")}</span>
            <Input
              type="password"
              value={pass}
              onChange={(e) => setPass(e.target.value)}
              className="h-9"
              dir="ltr"
              autoComplete="off"
            />
          </label>
        </div>
        {err ? <p className="mt-3 text-[12px] text-[var(--red)]">{err}</p> : null}
        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-[9px] border border-line px-3.5 text-[13px] text-txt2 hover:bg-white/[0.03]"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void link()}
            disabled={busy || !user || !pass}
            className="press h-9 rounded-[9px] bg-[var(--brand)] px-4 text-[13px] font-medium text-[var(--brand-ink)] disabled:opacity-50"
          >
            {busy ? t("routerConnecting") : t("routerLink")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
