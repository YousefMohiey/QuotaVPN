import { useEffect, useRef, useState } from "react"
import { Download, Languages, Loader2, MonitorPlay, RefreshCw } from "lucide-react"
import { Toggle } from "@/components/Toggle"
import { Segmented } from "@/components/Segmented"
import { Tip } from "@/components/Tip"
import { useApp } from "@/state/app"
import { fmtQuota, quotaLogout, quotaRefresh, useQuota } from "@/state/quota"
import { WeQuotaDialog } from "@/components/WeQuotaDialog"
import { WeMark } from "@/components/WeMark"
import { useI18n } from "@/lib/i18n"
import { api } from "@/lib/ipc"
import { cn } from "@/lib/utils"

/**
 * Settings is a ledger, not a stack of cards.
 *
 * The section names sit in a left gutter, the rows sit beside them on
 * hairlines that run the full width, and every control lands on the same right
 * edge. No page title: the rail already says where you are, and a title block
 * would cost a row for nothing.
 */
function Section({ label, gap, children }: { label: string; gap?: boolean; children: React.ReactNode }) {
  return (
    <section className={cn("flex flex-col", gap && "mt-8")}>
      <h2 className="mb-2 text-[11.5px] font-semibold uppercase leading-none tracking-[0.13em] text-txt2">{label}</h2>
      <div className="border-t border-line">{children}</div>
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
    <div className="flex min-h-[52px] items-center gap-4 border-b border-line py-3 last:border-b-0 max-[760px]:flex-wrap max-[760px]:gap-y-3">
      <span className="grid size-7 shrink-0 place-items-center text-txt2">
        <Icon className="size-[22px]" strokeWidth={1.75} aria-hidden />
      </span>
      <div className="min-w-0 flex-1">
        <h3 className="text-[13.5px] font-medium text-txt">{title}</h3>
        {body ? <p className="mt-0.5 break-words text-[12.5px] leading-relaxed text-txt2">{body}</p> : null}
        {note ? <div className="mt-1 text-[12px] leading-snug text-txt3">{note}</div> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2 max-[760px]:w-full max-[760px]:justify-start">{control}</div>
    </div>
  )
}

const AUTOSTART_CACHE = "qc-autostart"

function WeQuotaRows() {
  const { t } = useI18n()
  const q = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const busy = q.phase === "busy"
  const remaining = q.units.find((u) => u.total > 0)

  return (
    <>
      <Row
        icon={WeMark}
        title={t("weQuota")}
        body={
          q.linked && remaining ? (
            <span className="flex items-baseline gap-1.5">
              <span className="font-display text-[16px] font-semibold tabular-nums text-txt">{fmtQuota(remaining.remain)}</span>
              {t("weLeftShort")}
            </span>
          ) : (
            t("weQuotaSub")
          )
        }
        note={!q.linked ? t("weLinkHint") : null}
        control={
          q.linked ? (
            <>
              <Tip text={t("weRefresh")} align="end">
                <button
                  type="button"
                  onClick={() => void quotaRefresh(true)}
                  disabled={busy}
                  className="flex h-[34px] items-center gap-1.5 whitespace-nowrap rounded-[9px] border border-line bg-white/[0.03] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--brand-line)]"
                >
                  {busy ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                  {t("weRefresh")}
                </button>
              </Tip>
              <button
                type="button"
                onClick={() => void quotaLogout()}
                className="flex h-[34px] items-center whitespace-nowrap rounded-[9px] border border-line bg-transparent px-3.5 text-[12.5px] text-txt2 transition-colors hover:border-[var(--red-line)] hover:text-txt"
              >
                {t("weUnlink")}
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => setQuotaOpen(true)}
              className="press flex h-[34px] items-center rounded-[9px] bg-[var(--brand)] px-4 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110"
            >
              {t("weLink")}
            </button>
          )
        }
      />
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

  const checking = updateState === "checking"
  const installing = updateState === "installing"
  const offered = updateState === "available" || installing

  return (
    <div className="mx-auto flex w-full max-w-[780px] flex-col pb-6">
      <Section label={t("setAppGroup")}>
        <Row
          icon={Languages}
          title={t("language")}
          body={lang === "ar" ? "عربي" : "English"}
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
          icon={MonitorPlay}
          title={t("startupTitle")}
          body={t("startupBody")}
          note={startupMsg ? <span className="text-[var(--red)]">{startupMsg}</span> : null}
          control={<Toggle on={startup} busy={false} onFlip={() => void flipStartup(!startup)} label={t("startupTitle")} />}
        />
      </Section>

      <Section label={t("setConnGroup")} gap>
        <WeQuotaRows />
      </Section>

      <Section label={t("setUpdGroup")} gap>
        <Row
          icon={Download}
          title={
            <span className="flex flex-wrap items-baseline gap-x-2">
              {t("chCur")}
              <span className="font-display text-[15px] font-semibold tabular-nums text-txt">{cur || "-"}</span>
            </span>
          }
          body={
            <span aria-live="polite" className="flex min-w-0 items-center gap-1.5">
              <span
                aria-hidden
                className={cn(
                  "size-1.5 shrink-0 rounded-full",
                  updateState === "latest"
                    ? "bg-[var(--green)]"
                    : updateState === "error"
                      ? "bg-[var(--red)]"
                      : updateState === "idle"
                        ? "bg-[var(--line-strong)]"
                        : "bg-[var(--brand-vivid)]",
                )}
              />
              <span className="truncate">{updateText}</span>
            </span>
          }
          control={
            <>
              <button
                type="button"
                onClick={() => void checkUpdates()}
                disabled={checking}
                className="flex h-[34px] items-center gap-1.5 whitespace-nowrap rounded-[9px] border border-line bg-white/[0.03] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
              >
                {checking ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                {checking ? t("upChecking") : t("upCheck")}
              </button>
              {offered && (
                <button
                  type="button"
                  onClick={() => void applyUpdate()}
                  disabled={installing}
                  className="flex h-[34px] items-center whitespace-nowrap rounded-[9px] bg-[var(--brand)] px-3.5 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
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
          body={
            <span className={updateChannel === "experimental" ? "text-[var(--amber)]" : undefined}>
              {updateChannel === "experimental" ? t("chExpBody") : t("chStableBody")}
            </span>
          }
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
  )
}
