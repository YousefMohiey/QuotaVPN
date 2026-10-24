import { useEffect, useRef, useState } from "react"
import { Download, Languages, Loader2, Power, RefreshCw } from "lucide-react"
import { Toggle } from "@/components/Toggle"
import { WeMark } from "@/components/WeMark"
import { Segmented } from "@/components/Segmented"
import { Tip } from "@/components/Tip"
import { useApp } from "@/state/app"
import { fmtQuota, quotaLogout, quotaRefresh, useQuota } from "@/state/quota"
import { WeQuotaDialog } from "@/components/WeQuotaDialog"
import { useI18n } from "@/lib/i18n"
import { api } from "@/lib/ipc"
import { cn } from "@/lib/utils"

/**
 * Settings is one surface, not a stack of slabs.
 *
 * The section name lives in a left rail and the rows sit beside it on
 * hairlines, so the page reads as a list of facts about the app rather than
 * three identical cards competing for attention. Nothing is nested: the rail
 * labels, one panel, dividers only.
 */
function Section({
  label,
  badge,
  children,
}: {
  label: string
  badge?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <section className="grid grid-cols-1 gap-x-8 gap-y-3 border-b border-line px-5 py-5 last:border-b-0 sm:grid-cols-[126px_minmax(0,1fr)]">
      <h2 className="flex items-start gap-2 text-[11px] font-medium uppercase tracking-[0.14em] text-txt3 sm:pt-[3px]">
        {label}
        {badge}
      </h2>
      <div className="flex min-w-0 flex-col divide-y divide-line">{children}</div>
    </section>
  )
}

function Row({
  icon: Icon,
  title,
  body,
  note,
  control,
}: {
  icon: React.ComponentType<{ className?: string; strokeWidth?: number; "aria-hidden"?: boolean }>
  title: React.ReactNode
  body?: React.ReactNode
  note?: React.ReactNode
  control: React.ReactNode
}) {
  return (
    <div className="flex items-start gap-4 py-3.5 first:pt-0 last:pb-0">
      <Icon className="mt-0.5 size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
      <div className="min-w-0 flex-1">
        <h3 className="text-[13px] font-medium text-txt">{title}</h3>
        {body ? <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{body}</p> : null}
        {note ? <div className="mt-1 text-[12px] leading-snug text-txt3">{note}</div> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2">{control}</div>
    </div>
  )
}

const AUTOSTART_CACHE = "qc-autostart"

function WeQuotaRow() {
  const { t } = useI18n()
  const q = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const busy = q.phase === "busy"
  const main = q.units.find((u) => u.total > 0)

  return (
    <>
      <div className="flex items-start gap-4 py-3.5 first:pt-0 last:pb-0">
        <span className="mt-0.5 grid size-7 shrink-0 place-items-center rounded-full bg-white/[0.05] text-txt">
          <WeMark className="w-[19px]" />
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="text-[13px] font-medium text-txt">{t("weQuota")}</h3>
          <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("weQuotaSub")}</p>
          {!q.linked && <p className="mt-1 text-[12px] leading-snug text-txt3">{t("weLinkHint")}</p>}
        </div>
        {q.linked ? (
          <div className="flex shrink-0 items-center gap-2.5">
            {main ? (
              <span className="flex items-baseline gap-1 text-[12.5px] tabular-nums text-txt2">
                <span className="font-display text-[15px] font-semibold text-txt">{fmtQuota(main.remain)}</span>
                {t("weLeftShort")}
              </span>
            ) : null}
            <Tip text={t("weRefresh")} align="end">
              <button
                type="button"
                onClick={() => void quotaRefresh(true)}
                disabled={busy}
                className="flex h-9 items-center gap-1.5 whitespace-nowrap rounded-[8px] border border-line bg-white/[0.02] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--brand-line)]"
              >
                {busy ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                {t("weRefresh")}
              </button>
            </Tip>
            <button
              type="button"
              onClick={() => void quotaLogout()}
              className="flex h-9 items-center whitespace-nowrap rounded-[8px] px-2 text-[12.5px] text-txt3 transition-colors hover:bg-white/[0.04] hover:text-txt"
            >
              {t("weUnlink")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setQuotaOpen(true)}
            className="press flex h-9 shrink-0 items-center rounded-[8px] bg-[var(--brand)] px-3.5 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110"
          >
            {t("weLink")}
          </button>
        )}
      </div>
      <WeQuotaDialog open={quotaOpen} onOpenChange={setQuotaOpen} />
    </>
  )
}

export function Settings() {
  const { t, lang, setLang } = useI18n()
  const { update, updateState, updateError, checkUpdates, applyUpdate, updatePct, version, updateChannel, setUpdateChannel } =
    useApp()
  const [startup, setStartup] = useState(() => {
    try {
      return localStorage.getItem(AUTOSTART_CACHE) === "1"
    } catch {
      return false
    }
  })
  const [startupMsg, setStartupMsg] = useState("")
  const startupBusyRef = useRef(false)

  useEffect(() => {
    let alive = true
    api
      .autostartGet()
      .then((v) => {
        if (!alive) return
        setStartup(v === true)
        try {
          localStorage.setItem(AUTOSTART_CACHE, v === true ? "1" : "0")
        } catch {
          /* private mode: the cache just does not stick */
        }
      })
      .catch(() => {
        /* engine without the setting: stays as shown */
      })
    return () => {
      alive = false
    }
  }, [])

  const flipStartup = async (next: boolean) => {
    if (startupBusyRef.current) return
    startupBusyRef.current = true
    setStartup(next)
    setStartupMsg("")
    try {
      await api.autostartSet(next)
      try {
        localStorage.setItem(AUTOSTART_CACHE, next ? "1" : "0")
      } catch {
        /* ignore */
      }
    } catch {
      setStartup(!next)
      setStartupMsg(t("startupFail"))
    } finally {
      startupBusyRef.current = false
    }
  }

  const cur = version || update?.current
  const updateText =
    updateState === "checking"
      ? t("upChecking")
      : updateState === "latest" && update
        ? t("upToDate")
        : updateState === "available" && update
          ? t("upOut").replace("{v}", update.latest)
          : updateState === "installing"
            ? t("upInstalling")
            : updateState === "error"
              ? updateError || t("upFail")
              : cur
                ? t("upCur").replace("{v}", cur)
                : ""

  const upTone =
    updateState === "latest"
      ? "bg-[var(--green)]"
      : updateState === "error"
        ? "bg-[var(--red)]"
        : updateState === "idle"
          ? "bg-[var(--line-strong)]"
          : "bg-[var(--brand-vivid)]"

  const checking = updateState === "checking"
  const installing = updateState === "installing"
  const offered = updateState === "available" || installing

  return (
    <div className="mx-auto flex w-full max-w-[1000px] flex-col">
      <div className="overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)] shadow-[inset_0_1px_0_rgb(255_255_255/0.045)]">
        <Section label={t("setAppGroup")}>
          <Row
            icon={Languages}
            title={t("language")}
            body={t("languageBody")}
            control={
              <Segmented
                id="lang"
                value={lang}
                onChange={(l) => setLang(l)}
                options={[
                  { value: "en", label: "EN" },
                  { value: "ar", label: "عربي" },
                ]}
              />
            }
          />
          <Row
            icon={Power}
            title={t("startupTitle")}
            body={t("startupBody")}
            note={startupMsg ? <span className="text-[var(--red)]">{startupMsg}</span> : null}
            control={
              <>
                <span className="text-[12.5px] text-txt2">{startup ? t("wordOn") : t("wordOff")}</span>
                <Toggle on={startup} busy={false} onFlip={() => void flipStartup(!startup)} label={t("startupTitle")} />
              </>
            }
          />
        </Section>

        <Section label={t("setConnGroup")}>
          <WeQuotaRow />
        </Section>

        <Section
          label={t("setUpdGroup")}
          badge={
            updateChannel === "experimental" ? (
              <span className="rounded-full border border-[var(--brand-line)] bg-[var(--brand-bg)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.07em] text-brand-strong">
                {t("chBadge")}
              </span>
            ) : null
          }
        >
          <Row
            icon={Download}
            title={t("chCur")}
            body={
              <span className="flex min-w-0 items-baseline gap-2">
                <span className="font-display text-[16px] font-semibold tabular-nums text-txt">{cur || "-"}</span>
                <span aria-live="polite" className="flex min-w-0 items-center gap-1.5">
                  <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", upTone)} />
                  <span className="truncate">{updateText}</span>
                </span>
              </span>
            }
            control={
              <>
                <button
                  type="button"
                  onClick={() => void checkUpdates()}
                  disabled={checking}
                  className="flex h-9 items-center gap-1.5 whitespace-nowrap rounded-[8px] border border-line bg-white/[0.02] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
                >
                  {checking ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                  {checking ? t("upChecking") : t("upCheck")}
                </button>
                {offered && (
                  <button
                    type="button"
                    onClick={() => void applyUpdate()}
                    disabled={installing}
                    className="flex h-9 items-center whitespace-nowrap rounded-[8px] bg-[var(--brand)] px-3.5 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
                  >
                    {installing ? (updatePct !== null ? `${updatePct}%` : t("upInstalling")) : t("upGet")}
                  </button>
                )}
              </>
            }
          />
          <Row
            icon={RefreshCw}
            title={t("chTitle")}
            body={updateChannel === "experimental" ? t("chExpBody") : t("chStableBody")}
            control={
              <Segmented
                id="chan"
                value={updateChannel}
                options={[
                  { value: "normal", label: t("chStable") },
                  { value: "experimental", label: t("chExp") },
                ]}
                onChange={(v) => {
                  if (v !== updateChannel) void setUpdateChannel(v)
                }}
              />
            }
          />
        </Section>
      </div>
    </div>
  )
}
