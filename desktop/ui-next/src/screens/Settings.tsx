import { useEffect, useRef, useState } from "react"
import { Download, Languages, Loader2, MonitorPlay, RefreshCw, ShieldCheck } from "lucide-react"
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
 * Settings, to the mockup: rows run the full width of the pane, each section
 * name carries the rule under it, and every control lands on the same right
 * edge. No page title, because the rail already says where you are.
 */
function Section({ label, gap, children }: { label: string; gap?: boolean; children: React.ReactNode }) {
  return (
    <section className={cn("flex flex-col", gap && "mt-8")}>
      <h2 className="mb-0 border-b border-line pb-2 text-[11.5px] font-semibold uppercase leading-none tracking-[0.14em] text-txt2">
        {label}
      </h2>
      <div>{children}</div>
    </section>
  )
}


/** The WE mark in a ring, the way the quota card on Home wears it. */
function WeIcon() {
  return (
    <span className="grid size-10 place-items-center rounded-full border border-white/[0.11] bg-[rgb(255_255_255/0.03)] text-txt">
      <WeMark className="w-[23px]" />
    </span>
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
    <div className="group flex h-[76px] items-center gap-4 border-b border-line transition-colors duration-150 last:border-b-0 hover:bg-white/[0.018] max-[760px]:h-auto max-[760px]:flex-wrap max-[760px]:gap-y-3 max-[760px]:py-4">
      <span className="grid size-10 shrink-0 place-items-center text-txt2">
        <Icon className="size-6" strokeWidth={1.7} aria-hidden />
      </span>
      <div className="min-w-0 flex-1">
        <h3 className="text-[14px] font-medium text-txt">{title}</h3>
        {body ? <p className="mt-1 break-words text-[13px] leading-relaxed text-txt2">{body}</p> : null}
        {note ? <div className="mt-1 text-[12.5px] leading-snug text-txt3">{note}</div> : null}
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
        icon={WeIcon}
        title={t("weQuota")}
        body={
          q.linked && remaining ? (
            <span className="flex items-baseline gap-1.5">
              <span className="font-display text-[16px] font-semibold tabular-nums text-txt">{fmtQuota(remaining.remain)}</span>
              {t("weLeftShort")}
            </span>
          ) : (
            t("weNotLinked")
          )
        }
        control={
          q.linked ? (
            <>
              <Tip text={t("weRefresh")} align="end">
                <button
                  type="button"
                  onClick={() => void quotaRefresh(true)}
                  disabled={busy}
                  className="flex h-10 items-center gap-2 whitespace-nowrap rounded-[9px] border border-line bg-white/[0.03] px-4 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)]"
                >
                  {busy ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                  {t("weRefresh")}
                </button>
              </Tip>
              <button
                type="button"
                onClick={() => void quotaLogout()}
                className="flex h-10 items-center whitespace-nowrap rounded-[9px] border border-line bg-transparent px-4 text-[13px] text-txt2 transition-colors hover:border-[var(--red-line)] hover:text-txt"
              >
                {t("weUnlink")}
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => setQuotaOpen(true)}
              className="press flex h-10 items-center rounded-[9px] bg-[var(--brand)] px-5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110"
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
  const { update, updateState, updateError, checkUpdates, applyUpdate, updatePct, version, updateChannel, setUpdateChannel, bypass, setBypass } =
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
    <div className="flex w-full flex-col pb-6">
      <Section label={t("setAppGroup")}>
        <Row
          icon={Languages}
          title={t("language")}
          body={lang === "ar" ? "عربي" : "English"}
          control={
            <Segmented
              boxed
              className="h-10 w-[150px]"
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
        <Row
          icon={ShieldCheck}
          title={t("bypassTitle")}
          body={bypass ? t("bypassOn") : t("bypassBody")}
          control={
            <Toggle
              on={bypass}
              busy={false}
              onFlip={() => setBypass(!bypass)}
              label={t("bypassTitle")}
            />
          }
        />
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
                className="flex h-10 items-center gap-2 whitespace-nowrap rounded-[9px] border border-line bg-white/[0.03] px-4 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
              >
                {checking ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                {checking ? t("upChecking") : t("upCheck")}
              </button>
              {offered && (
                <button
                  type="button"
                  onClick={() => void applyUpdate()}
                  disabled={installing}
                  className="flex h-10 items-center whitespace-nowrap rounded-[9px] bg-[var(--brand)] px-4 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
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
              boxed
              className="h-10 w-[250px]"
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
