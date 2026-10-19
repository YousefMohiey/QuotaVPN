import { useEffect, useMemo, useState } from "react"
import { Gamepad2, Layers, Play, RefreshCw } from "lucide-react"
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

  // the headline reads what is left, so the bar underneath measures what has
  // been spent: the two together, never two different stories
  const pct =
    mainUnit && mainUnit.total > 0
      ? Math.max(0, Math.min(100, (mainUnit.used / mainUnit.total) * 100))
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
    <section className="flex min-h-0 flex-1 flex-col px-4 pb-3.5 pt-3.5">
      {quota.linked && mainUnit ? (
        <>
          <div className="flex items-center gap-3">
            <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
              <WeMark className="w-[22px]" />
            </span>
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-[14.5px] font-semibold text-txt">{t("weQuota")}</h2>
              <p className="mt-0.5 truncate text-[11.5px] text-txt2" dir="auto" title={mainUnit.offer || "WE"}>
                {mainUnit.offer || "WE"}
              </p>
            </div>
            <Tip text={t("weRefresh")}>
              <button
                type="button"
                onClick={() => void quotaRefresh(true)}
                aria-label={t("weRefresh")}
                disabled={quota.phase === "busy"}
                className="press grid size-8 shrink-0 place-items-center rounded-full border border-line bg-white/[0.02] text-txt2 transition-colors hover:border-[var(--brand-line)] hover:text-brand-strong disabled:cursor-wait"
              >
                <RefreshCw className={cn("size-3.5", quota.phase === "busy" && "animate-spin")} aria-hidden />
              </button>
            </Tip>
          </div>

          <div className="mt-3">
            <div className="flex items-baseline gap-1.5">
              <span className="font-display text-[33px] font-semibold leading-none tabular-nums tracking-[-0.02em] text-txt">{numPart}</span>
              <span className="text-[17px] font-medium text-txt2">{unitPart}</span>
            </div>
            <div className="mt-1.5 text-[12px] text-txt2">
              {t("weLeft")} {fmtQuota(mainUnit.total)}
            </div>
            <div
              role="progressbar"
              aria-valuenow={Math.round(mainUnit?.used ?? 0)}
              aria-valuemin={0}
              aria-valuemax={Math.round(mainUnit?.total ?? 0)}
              aria-label={t("weQuota")}
              className="mt-3.5 h-3 overflow-hidden rounded-full bg-white/[0.06]"
            >
              <div
                className="h-full origin-left rounded-full bg-[var(--brand-vivid)] transition-transform duration-[240ms] ease-linear"
                style={{ transform: `scaleX(${pct / 100})` }}
              />
            </div>
            <div className="mt-2.5 flex items-baseline justify-between gap-2 text-[11.5px]">
              <span className="truncate text-txt2">
                {t("weUsed")} {fmtQuota(mainUnit.used)}
              </span>
              {quota.updatedAt ? (
                <span className="shrink-0 tabular-nums text-txt3">
                  {t("weUpdated")}{" "}
                  {new Date(quota.updatedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
                </span>
              ) : null}
            </div>
            {quota.phase === "error" && quota.msgKey ? (
              <p className="mt-1.5 truncate text-[11px] text-[var(--red)]" dir="auto">
                {t(quota.msgKey as Parameters<typeof t>[0])}
                {quota.msgRaw ? ` (${quota.msgRaw})` : ""}
              </p>
            ) : null}
          </div>

          {buckets.length ? (
            <div className="mt-4 flex min-h-0 flex-1 flex-col gap-2.5 border-t border-line pt-4">
              <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{t("wePackage")}</p>
              {buckets.map((b, i) => {
                const k = bucketKey(b.item)
                const label = k ? t(k) : b.label || "WE"
                const used = Math.max(0, b.total - b.remain)
                const upct = b.total > 0 ? Math.max(0, Math.min(100, (used / b.total) * 100)) : 0
                const Icon = i === 1 ? Gamepad2 : i === 2 ? Play : Layers
                return (
                  <article
                    key={b.item || b.label}
                    className="flex min-h-0 flex-1 flex-col justify-between gap-1.5 rounded-[10px] bg-white/[0.03] px-3.5 py-2.5"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-white/[0.04] text-brand-strong">
                        <Icon className="size-3.5" aria-hidden />
                      </span>
                      <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{label}</span>
                      <span className="ml-auto shrink-0 text-[11px] tabular-nums text-txt2">{fmtQuota(b.total)}</span>
                    </div>
                    <div className="flex min-w-0 items-baseline gap-1.5">
                      <span className="font-display text-[21px] font-semibold leading-none tabular-nums text-txt">{fmtQuota(used)}</span>
                      <span className="shrink-0 text-[11px] text-txt2">{t("weUsed")}</span>
                      <span className="ml-auto shrink-0 text-[11px] tabular-nums text-txt2">
                        {fmtQuota(b.remain)} {t("weLeftShort")}
                      </span>
                    </div>
                    <span
                      role="progressbar"
                      aria-valuenow={Math.round(used)}
                      aria-valuemin={0}
                      aria-valuemax={Math.round(b.total)}
                      aria-label={label}
                      className="h-2 w-full shrink-0 overflow-hidden rounded-full bg-white/[0.07]"
                    >
                      <span
                        className={cn(
                          "block h-full origin-left rounded-full transition-transform duration-[240ms] ease-linear",
                          upct >= 99 ? "bg-[var(--red)]" : "bg-[var(--brand-vivid)]",
                        )}
                        style={{ transform: `scaleX(${upct / 100})` }}
                      />
                    </span>
                  </article>
                )
              })}
            </div>
          ) : null}
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
