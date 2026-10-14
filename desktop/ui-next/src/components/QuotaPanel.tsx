import { useEffect, useMemo, useState } from "react"
import { Gauge, RefreshCw } from "lucide-react"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"
import { fmtQuota, quotaRefresh, useQuota } from "@/state/quota"
import { WeQuotaDialog } from "./WeQuotaDialog"

/**
 * The line's own panel: a full quota block when linked, a centered
 * invite when not. Login lives in the popup, never inline.
 */
export function QuotaPanel({ className }: { className?: string }) {
  const { t } = useI18n()
  const quota = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const mainUnit = useMemo(
    () => quota.units.find((u) => u.total > 0),
    [quota.units],
  )

  // The linked WE line reads itself once per launch; the tap refreshes it.
  useEffect(() => {
    void quotaRefresh()
  }, [])

  return (
    <section className={cn("flex min-h-0 flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] p-5", className)}>
      {quota.linked && mainUnit ? (
        <div>
          <div className="flex items-center gap-3">
            <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
              <Gauge className="size-4" strokeWidth={1.7} aria-hidden />
            </span>
            <div className="min-w-0 flex-1">
              <h2 className="text-[14px] font-semibold text-txt">{t("weQuota")}</h2>
              <p className="mt-0.5 truncate text-[11.5px] text-txt2" dir="auto" title={mainUnit.offer || "WE"}>
                {mainUnit.offer || "WE"}
                {quota.updatedAt ? ` · ${new Date(quota.updatedAt).toLocaleTimeString()}` : ""}
              </p>
            </div>
            <button
              type="button"
              onClick={() => void quotaRefresh(true)}
              title={t("weRefresh")}
              aria-label={t("weRefresh")}
              disabled={quota.phase === "busy"}
              className="grid size-8 shrink-0 place-items-center rounded-[10px] border border-line bg-white/[0.02] text-txt2 transition-colors hover:border-[var(--brand-line)] hover:text-brand-strong disabled:cursor-wait"
            >
              <RefreshCw className={cn("size-3.5", quota.phase === "busy" && "animate-spin")} aria-hidden />
            </button>
          </div>
          <div className="mt-3 text-[24px] font-semibold leading-tight tabular-nums text-txt">
            {fmtQuota(mainUnit.remain)}
          </div>
          <div className="mt-0.5 text-[12px] text-txt2">
            {t("weLeft")} {fmtQuota(mainUnit.total)}
          </div>
          <div className="mt-2.5 h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
            <div
              className="h-full rounded-full bg-[var(--brand-vivid)]"
              style={{
                width: `${mainUnit.total > 0 ? Math.max(0, Math.min(100, (mainUnit.remain / mainUnit.total) * 100)) : 0}%`,
              }}
            />
          </div>
          <p className="mt-1.5 text-[11px] text-txt3">
            {t("weUsed")} {fmtQuota(mainUnit.used)}
          </p>
        </div>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center py-4 text-center">
          <span className="grid size-12 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
            <Gauge className="size-5" strokeWidth={1.7} aria-hidden />
          </span>
          <h2 className="mt-3 text-[14px] font-semibold text-txt">{t("weQuota")}</h2>
          <p className="mt-1 text-[12px] leading-relaxed text-txt2">{t("weQuotaSub")}</p>
          <button
            type="button"
            onClick={() => setQuotaOpen(true)}
            className="mt-3 flex h-10 w-full items-center justify-center rounded-[10px] bg-[var(--brand-vivid)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110"
          >
            {t("weLink")}
          </button>
        </div>
      )}
      <WeQuotaDialog open={quotaOpen} onOpenChange={setQuotaOpen} />
    </section>
  )
}
