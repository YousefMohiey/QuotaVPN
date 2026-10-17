import { useEffect, useMemo, useState } from "react"
import { RefreshCw } from "lucide-react"
import { cn } from "@/lib/utils"
import { WeMark } from "./WeMark"
import { useI18n } from "@/lib/i18n"
import { fmtQuota, quotaRefresh, useQuota } from "@/state/quota"
import { WeQuotaDialog } from "./WeQuotaDialog"

/**
 * The package panel, and the only place on Home the WE line reads: the big
 * remaining number on top, the bar under it, and the package split holding
 * the foot. Unlinked it is the invite, centered in the same panel.
 */
export function WeQuotaCard() {
  const { t } = useI18n()
  const quota = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const mainUnit = useMemo(() => quota.units.find((u) => u.total > 0), [quota.units])

  // The linked WE line reads itself once per launch; the tap refreshes it.
  useEffect(() => {
    void quotaRefresh()
  }, [])

  const pct =
    mainUnit && mainUnit.total > 0
      ? Math.max(0, Math.min(100, (mainUnit.remain / mainUnit.total) * 100))
      : 0
  const buckets = useMemo(
    () => (mainUnit ? mainUnit.buckets.filter((b) => b.remain > 0).slice(0, 3) : []),
    [mainUnit],
  )

  return (
    <section className="flex min-h-0 flex-1 flex-col rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 pt-4 pb-5">
      {quota.linked && mainUnit ? (
        <>
          <div className="flex items-center gap-3">
            <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
              <WeMark className="w-[22px]" />
            </span>
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-[13.5px] font-semibold text-txt">{t("weQuota")}</h2>
              <p className="mt-0.5 truncate text-[11px] text-txt2" dir="auto" title={mainUnit.offer || "WE"}>
                {mainUnit.offer || "WE"}
              </p>
            </div>
            <button
              type="button"
              onClick={() => void quotaRefresh(true)}
              title={t("weRefresh")}
              aria-label={t("weRefresh")}
              disabled={quota.phase === "busy"}
              className="grid size-8 shrink-0 place-items-center rounded-full border border-line bg-white/[0.02] text-txt2 transition-colors hover:border-[var(--brand-line)] hover:text-brand-strong disabled:cursor-wait"
            >
              <RefreshCw className={cn("size-3.5", quota.phase === "busy" && "animate-spin")} aria-hidden />
            </button>
          </div>

          <div className="my-auto">
            <div className="mt-4 text-[30px] font-semibold leading-none tabular-nums text-txt">
              {fmtQuota(mainUnit.remain)}
            </div>
            <div className="mt-2 text-[12px] text-txt2">
              {t("weLeft")} {fmtQuota(mainUnit.total)}
            </div>
            <div className="mt-4 h-2 overflow-hidden rounded-full bg-white/[0.06]">
              <div
                className="h-full rounded-full bg-[var(--brand-vivid)] transition-[width] duration-700 ease-out"
                style={{ width: `${pct}%` }}
              />
            </div>
            <div className="mt-2 flex items-baseline justify-between gap-2 text-[11.5px] text-txt2">
              <span className="truncate">
                {t("weUsed")} {fmtQuota(mainUnit.used)}
              </span>
              {quota.updatedAt ? (
                <span className="shrink-0 tabular-nums">
                  {t("weUpdated")}{" "}
                  {new Date(quota.updatedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
                </span>
              ) : null}
            </div>
            {quota.phase === "error" && quota.msgKey ? (
              <p className="mt-1.5 truncate text-[10.5px] text-[var(--red)]" dir="auto">
                {t(quota.msgKey as Parameters<typeof t>[0])}
                {quota.msgRaw ? ` (${quota.msgRaw})` : ""}
              </p>
            ) : null}
          </div>

          {buckets.length ? (
            <div className="mt-auto border-t border-line pt-3">
              <p className="mb-2.5 text-[11px] font-semibold uppercase tracking-[0.07em] text-txt2">{t("wePackage")}</p>
              <div className="grid grid-cols-3 divide-x divide-line">
                {buckets.map((b, i) => {
                  const k = bucketKey(b.item)
                  return (
                    <div key={b.item || b.label} className={cn("min-w-0", i === 0 ? "pe-3" : i === buckets.length - 1 ? "ps-3" : "px-3")}>
                      <div className="truncate text-[11px] font-semibold uppercase tracking-[0.07em] text-txt2">
                        {k ? t(k) : b.label || "WE"}
                      </div>
                      <div className="mt-1 truncate text-[14px] font-semibold tabular-nums text-txt">
                        {fmtQuota(b.remain)}
                      </div>
                      <div className="mt-0.5 truncate text-[11px] tabular-nums text-txt3">
                        {t("weLeft")} {fmtQuota(b.total)}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          ) : null}
        </>
      ) : (
        <div className="flex flex-1 flex-col justify-center">
          <div className="flex items-center gap-3">
            <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
              <WeMark className="w-[22px]" />
            </span>
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-[13.5px] font-semibold text-txt">{t("weQuota")}</h2>
              <p className="mt-1 text-[11.5px] leading-snug text-txt2">{t("weQuotaSub")}</p>
              <p className="mt-1.5 text-[10.5px] leading-snug text-txt3">{t("weLinkHint")}</p>
            </div>
            <button
              type="button"
              onClick={() => setQuotaOpen(true)}
              className="flex h-9 shrink-0 items-center rounded-[9px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-4 text-[12.5px] font-medium text-[var(--brand-strong)] transition-colors hover:border-[var(--brand-strong)]"
            >
              {t("weLink")}
            </button>
          </div>
        </div>
      )}
      <WeQuotaDialog open={quotaOpen} onOpenChange={setQuotaOpen} />
    </section>
  )
}

/** The package buckets read by their meaning; unknown items keep their own label. */
function bucketKey(item: string): "weBucketMain" | "weBucketGaming" | "weBucketStreaming" | null {
  const s = item.toLowerCase()
  if (s.includes("gaming")) return "weBucketGaming"
  if (s.includes("racing") || s.includes("stream")) return "weBucketStreaming"
  if (s.includes("_data")) return "weBucketMain"
  return null
}
