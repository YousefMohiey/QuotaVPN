import { useState } from "react"
import { Activity, ArrowDown, ArrowLeft, ArrowUp, Gauge } from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { buildVerdicts, loadHistory, type Run } from "./Speed"
import { cn } from "@/lib/utils"

/** One stored run shown the way the Speed page states a finished test: the
    headline number on its own, the four readings under it, verdicts last.
    Nothing is measured here; everything comes from the stored run. */
export function SpeedResult({ runAt, onBack }: { runAt: number | null; onBack: () => void }) {
  const { t } = useI18n()
  const [history] = useState<Run[]>(() => loadHistory())
  const run = history.find((h) => h.at === runAt) ?? history[0] ?? null
  const verdicts = run ? buildVerdicts(run, t) : []

  const marquee = run?.down ?? run?.up ?? null
  const headline = marquee ?? run?.ping ?? null
  const headlineUnit = marquee !== null && marquee !== undefined ? t("mbps") : t("ms")
  const headlineText =
    headline === null || headline === undefined
      ? "-"
      : headlineUnit === t("ms")
        ? String(Math.round(headline))
        : headline >= 100
          ? headline.toFixed(0)
          : headline.toFixed(1)

  const readings = run
    ? [
        { key: "ping", icon: Gauge, title: t("pingTitle"), value: run.ping, unit: t("ms") },
        { key: "jitter", icon: Activity, title: t("jitter"), value: run.jitter, unit: t("ms") },
        { key: "down", icon: ArrowDown, title: t("chDown"), value: run.down, unit: t("mbps") },
        { key: "up", icon: ArrowUp, title: t("chUp"), value: run.up, unit: t("mbps") },
      ]
    : []

  const when = run
    ? `${new Date(run.at).toLocaleDateString([], { month: "short", day: "numeric" })} ${new Date(run.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
    : t("history")

  return (
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-4">
      <div className="flex items-end justify-between gap-6">
        <div className="min-w-0 flex-1">
          <h1 className="text-[30px] font-semibold leading-tight text-txt">{when}</h1>
          <p className="mt-1 truncate text-[15px] text-txt2" dir="auto">
            {run?.target || t("targetServer")}
          </p>
        </div>
        <button
          type="button"
          onClick={onBack}
          className="flex h-9 shrink-0 items-center gap-1.5 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"
        >
          <ArrowLeft className="size-3.5" aria-hidden />
          {t("back")}
        </button>
      </div>

      {run === null ? (
        <section className="flex flex-col items-center justify-center rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] px-5 py-12 text-center">
          <p className="text-[13.5px] text-txt2">{t("histEmpty")}</p>
        </section>
      ) : (
        <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
          <div className="flex min-h-[196px] items-center justify-center px-5 py-4">
            <div className="flex items-baseline gap-2">
              <span
                className="text-[44px] leading-none font-light tabular-nums text-txt"
                style={{ letterSpacing: "-0.02em" }}
              >
                {headlineText}
              </span>
              <span className="text-[13px] text-txt3">{headlineUnit}</span>
            </div>
          </div>

          <div className="grid grid-cols-4 divide-x divide-line border-t border-line">
            {readings.map((r) => (
              <div key={r.key} className="flex min-w-0 items-center gap-3 px-4 py-3">
                <span className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-line bg-[rgb(255_255_255/0.03)] text-brand-strong">
                  <r.icon className="size-4" strokeWidth={1.7} aria-hidden />
                </span>
                <span className="min-w-0">
                  <span className="block truncate text-[11.5px] text-txt3">{r.title}</span>
                  <span
                    className={cn(
                      "mt-0.5 block truncate text-[17px] font-medium tabular-nums",
                      r.value === null ? "text-txt3" : "text-txt",
                    )}
                  >
                    {r.value === null ? "-" : r.unit === "ms" ? Math.round(r.value) : r.value.toFixed(1)}
                    <span className="ms-1 text-[11px] font-normal text-txt3">{r.unit}</span>
                  </span>
                </span>
              </div>
            ))}
          </div>

          {verdicts.length > 0 && (
            <div className="flex flex-wrap items-center gap-x-5 gap-y-2 border-t border-line px-5 py-3">
              {verdicts.map((v) => (
                <span key={v.label} className="flex items-center gap-2 text-[12px]">
                  <v.icon className="size-3.5 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
                  <span
                    aria-hidden
                    className={cn(
                      "size-1.5 rounded-full",
                      v.tone === "ok" && "bg-[var(--green)]",
                      v.tone === "warn" && "bg-[var(--amber)]",
                      v.tone === "bad" && "bg-[var(--red)]",
                    )}
                  />
                  <span className="text-txt2">{v.text}</span>
                </span>
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  )
}
