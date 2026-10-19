import { useEffect, useMemo, useState } from "react"
import { RefreshCw } from "lucide-react"
import { cn } from "@/lib/utils"
import { WeMark } from "./WeMark"
import { useI18n } from "@/lib/i18n"
import { fmtQuota, quotaRefresh, useQuota } from "@/state/quota"
import { WeQuotaDialog } from "./WeQuotaDialog"
import { Tip } from "./Tip"

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

  // every bar on this panel measures what is left, the same thing the big
  // numbers say; the used figure stays a number, never a second bar
  const pct =
    mainUnit && mainUnit.total > 0
      ? Math.max(0, Math.min(100, (mainUnit.remain / mainUnit.total) * 100))
      : 0
  const buckets = useMemo(
    () => (mainUnit ? mainUnit.buckets.filter((b) => b.remain > 0).slice(0, 3) : []),
    [mainUnit],
  )
  const [numPart, unitPart = ""] = useMemo(
    () => fmtQuota(mainUnit?.remain ?? 0).split(" "),
    [mainUnit],
  )

  return (
    <section className="flex min-h-0 flex-col gap-3.5 border-s border-line bg-[var(--side)] px-5 py-5">
      {quota.linked && mainUnit ? (
        <>
          <div className="flex shrink-0 items-center justify-between gap-2">
            <span className="flex min-w-0 items-center gap-2">
              <span className="grid size-7 shrink-0 place-items-center rounded-full bg-white/[0.04] text-txt">
                <WeMark className="w-[15px]" />
              </span>
              <span className="truncate text-[11px] font-medium uppercase tracking-[0.1em] text-txt2">{t("weQuota")}</span>
            </span>
            <span className="flex shrink-0 items-center gap-1.5">
              <span className="max-w-[120px] truncate rounded-full bg-white/[0.04] px-2 py-0.5 text-[11px] text-txt2" title={mainUnit.offer || "WE"} dir="auto">
                {mainUnit.offer || "WE"}
              </span>
              <Tip text={t("weRefresh")}>
                <button
                  type="button"
                  onClick={() => void quotaRefresh(true)}
                  aria-label={t("weRefresh")}
                  disabled={quota.phase === "busy"}
                  className="press grid size-7 shrink-0 place-items-center rounded-full text-txt3 transition-colors hover:text-brand-strong disabled:cursor-wait"
                >
                  <RefreshCw className={cn("size-3.5", quota.phase === "busy" && "animate-spin")} aria-hidden />
                </button>
              </Tip>
            </span>
          </div>

          <div className="shrink-0 rounded-[12px] bg-white/[0.025] px-4 py-4">
            <div className="flex min-w-0 items-baseline gap-2">
              <span className="font-display text-[44px] font-semibold leading-none tracking-[-0.035em] tabular-nums text-txt">{numPart}</span>
              <span className="text-[15px] font-medium text-txt2">{unitPart}</span>
            </div>
            <div className="mt-1 flex items-baseline justify-between gap-3 text-[12px] text-txt2">
              <span className="truncate">
                {t("weLeft")} {fmtQuota(mainUnit.total)}
              </span>
              <span className="shrink-0 text-[11.5px] tabular-nums text-txt3">
                {t("weUsed")} {fmtQuota(mainUnit.used)}
              </span>
            </div>
            <div
              role="progressbar"
              aria-valuenow={Math.round(mainUnit.remain)}
              aria-valuemin={0}
              aria-valuemax={Math.round(mainUnit.total)}
              aria-label={t("weQuota")}
              className="mt-3.5 h-3 w-full overflow-hidden rounded-full bg-white/[0.06]"
            >
              <div
                className="h-full origin-left rounded-full bg-[var(--brand-vivid)] transition-transform duration-[240ms] ease-linear"
                style={{ transform: `scaleX(${pct / 100})` }}
              />
            </div>
            {quota.updatedAt ? (
              <div className="mt-2 flex items-baseline justify-end text-[11.5px] tabular-nums text-txt3">
                {t("weUpdated")} {new Date(quota.updatedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
              </div>
            ) : null}
            {quota.phase === "error" && quota.msgKey ? (
              <p className="mt-2 truncate text-[11px] text-[var(--red)]" dir="auto">
                {t(quota.msgKey as Parameters<typeof t>[0])}
                {quota.msgRaw ? ` (${quota.msgRaw})` : ""}
              </p>
            ) : null}
          </div>

          {buckets.length ? (
            <div className="flex min-h-0 flex-1 flex-col divide-y divide-line border-t border-line">
              <div className="flex shrink-0 items-baseline justify-between gap-2 py-2 text-[11px] uppercase tracking-[0.09em] text-txt2">
                <span>{t("wePackage")}</span>
                <span className="normal-case tracking-normal text-txt3">{t("wePackNote")}</span>
              </div>
              {buckets.map((b) => {
                const k = bucketKey(b.item)
                const label = k ? t(k) : b.label || "WE"
                const upct = b.total > 0 ? Math.max(0, Math.min(100, ((b.total - b.remain) / b.total) * 100)) : 0
                return (
                  <div key={b.item || b.label} className={cn("flex min-h-0 min-w-0 flex-col gap-2.5 py-3.5", buckets.length > 1 ? "flex-1 justify-center" : "justify-start")}>
                    <div className="flex min-w-0 items-baseline justify-between gap-2">
                      <span className="truncate text-[11px] font-medium uppercase tracking-[0.09em] text-txt2">{label}</span>
                      <span className="shrink-0 text-[12px] tabular-nums text-txt2">
                        <span className="font-medium text-txt">{fmtQuota(b.remain)}</span> / {fmtQuota(b.total)}
                      </span>
                    </div>
                    <span
                      role="progressbar"
                      aria-valuenow={Math.round(b.remain)}
                      aria-valuemin={0}
                      aria-valuemax={Math.round(b.total)}
                      aria-label={label}
                      className={cn("w-full shrink-0 overflow-hidden rounded-full bg-white/[0.07]", buckets.length === 1 ? "h-2.5" : "h-2")}
                    >
                      <span
                        className={cn(
                          "block h-full origin-left rounded-full transition-transform duration-[240ms] ease-linear",
                          upct >= 99 ? "bg-[var(--red)]" : "bg-[var(--brand-vivid)]",
                        )}
                        style={{ transform: `scaleX(${(100 - upct) / 100})` }}
                      />
                    </span>
                  </div>
                )
              })}
            </div>
          ) : (
            <div className="flex min-h-0 flex-1 items-center justify-center text-[11.5px] text-txt2">
              {t("wePackageNone")}
            </div>
          )}
        </>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center text-center">
          <span className="grid size-14 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
            <WeMark className="w-[30px]" />
          </span>
          <h2 className="mt-3.5 text-[16.5px] font-semibold text-txt">{t("weQuota")}</h2>
          <p className="mt-1 text-[13.5px] leading-relaxed text-txt2">{t("weQuotaSub")}</p>
          <button
            type="button"
            onClick={() => setQuotaOpen(true)}
            className="mt-4 flex h-10 shrink-0 items-center rounded-[10px] border border-[var(--brand-line)] bg-[var(--brand-bg)] px-5 text-[13.5px] font-medium text-[var(--brand-strong)] transition-colors hover:border-[var(--brand-strong)]"
          >
            {t("weLink")}
          </button>
          <p className="mt-3 text-[11.5px] leading-snug text-txt3">{t("weLinkHint")}</p>
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
