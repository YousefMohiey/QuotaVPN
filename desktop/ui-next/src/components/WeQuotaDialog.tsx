import { useEffect, useRef, useState } from "react"
import { WeMark } from "./WeMark"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { useI18n } from "@/lib/i18n"
import { quotaLink, savedQuotaPass, useQuota } from "@/state/quota"

/** The WE login lives here and only here: a popup over whatever screen
    calls it. Home and Settings open it, neither embeds the form. */
export function WeQuotaDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
}) {
  const { t } = useI18n()
  const q = useQuota()
  const [number, setNumber] = useState(q.number)
  const [password, setPassword] = useState("")
  const [code, setCode] = useState("")
  const busy = q.phase === "busy"
  const first = useRef(true)

  useEffect(() => {
    if (open && first.current) {
      first.current = false
      setNumber(q.number)
    }
    if (!open) first.current = true
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open ])

  // A good link closes the popup by itself.
  useEffect(() => {
    if (open && q.linked) {
      onOpenChange(false)
      setPassword("")
      setCode("")
    }
  }, [open, q.linked, onOpenChange])

  const link = (withCode: string) =>
    quotaLink(number.trim(), password || savedQuotaPass(), withCode)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[380px] gap-0 overflow-hidden rounded-[var(--r-card)] border-line bg-[var(--popover)] p-0">
        <div className="border-b border-line px-4 pb-3.5 pt-4">
          <div className="flex items-center gap-3">
            <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
              <WeMark className="w-[22px]" />
            </span>
            <div className="min-w-0">
              <DialogTitle className="text-[14px] font-semibold text-txt">
                {t("weQuota")}
              </DialogTitle>
              <p className="mt-0.5 text-[12px] text-txt2">{t("weQuotaSub")}</p>
            </div>
          </div>
        </div>

        <div className="flex flex-col gap-2.5 p-4">
          <Input
            value={number}
            onChange={(e) => setNumber(e.target.value)}
            inputMode="tel"
            autoComplete="off"
            placeholder={t("weNumber")}
            aria-label={t("weNumber")}
            className="h-10 rounded-[10px] border-line bg-white/[0.02] text-[13px] focus-visible:border-[var(--brand-line)] focus-visible:ring-[3px] focus-visible:ring-[rgb(46_123_246/0.14)]"
          />
          <Input
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            type="password"
            autoComplete="off"
            placeholder={t("wePassword")}
            aria-label={t("wePassword")}
            className="h-10 rounded-[10px] border-line bg-white/[0.02] text-[13px] focus-visible:border-[var(--brand-line)] focus-visible:ring-[3px] focus-visible:ring-[rgb(46_123_246/0.14)]"
          />
          {q.phase === "need-code" && (
            <div className="flex items-center gap-2.5">
              {q.challengeImage ? (
                <img
                  src={q.challengeImage}
                  alt={t("weCode")}
                  className="h-10 shrink-0 rounded-[8px] border border-line"
                />
              ) : null}
              <Input
                value={code}
                onChange={(e) => setCode(e.target.value)}
                autoComplete="off"
                placeholder={t("weCode")}
                aria-label={t("weCode")}
                className="h-10 rounded-[10px] border-line bg-white/[0.02] text-[13px] focus-visible:border-[var(--brand-line)] focus-visible:ring-[3px] focus-visible:ring-[rgb(46_123_246/0.14)]"
              />
            </div>
          )}
          {q.phase === "error" && q.msgKey ? (
            <p className="text-[12px] text-[var(--red)]">
              {t(q.msgKey as never)}
              {q.msgRaw ? ` (${q.msgRaw})` : ""}
            </p>
          ) : null}
          <div className="flex items-center gap-3">
            {q.phase === "need-code" ? (
              <button
                type="button"
                onClick={() => void link(code.trim())}
                disabled={busy || !code.trim()}
                className="flex h-10 flex-1 items-center justify-center rounded-[10px] bg-[var(--brand)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
              >
                {t("weConfirm")}
              </button>
            ) : (
              <button
                type="button"
                onClick={() => void link("")}
                disabled={busy || !number.trim()}
                className="flex h-10 flex-1 items-center justify-center rounded-[10px] bg-[var(--brand)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
              >
                {t("weLink")}
              </button>
            )}
          </div>
          <p className="text-[11px] text-txt3">{t("weLinkHint")}</p>
        </div>
      </DialogContent>
    </Dialog>
  )
}
