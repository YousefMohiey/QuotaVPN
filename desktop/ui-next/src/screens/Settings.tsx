import { useEffect, useRef, useState } from "react"
import { Download, Languages, Loader2, MonitorPlay, RefreshCw } from "lucide-react"
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
 * Settings is a page you can read without touching anything.
 *
 * Every group carries its own state on the right of its heading, so the answer
 * to "what is this set to" is on screen before a single row is parsed. The rows
 * below are the controls that change it: one column, one card per group, no
 * nesting.
 */
function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col">
      <h2 className="flex h-9 items-center border-b border-line bg-white/[0.02] px-12 text-[11px] font-semibold uppercase tracking-[0.13em] text-txt2">
        {title}
      </h2>
      {children}
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
    <div className="flex items-center gap-4 border-b border-line px-4 py-3.5 last:border-b-0">
      <Icon className="size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
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
  const remaining = q.units.find((u) => u.total > 0)

  return (
    <>
      <div className="flex items-center gap-4 border-b border-line px-4 py-3.5 last:border-b-0">
        <span className="grid size-4 shrink-0 place-items-center text-txt3">
          <WeMark className="w-[16px]" />
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="text-[13px] font-medium text-txt">{t("weQuota")}</h3>
          <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">
            {q.linked && remaining ? (
              <span className="flex items-baseline gap-1.5">
                <span className="font-display text-[14px] font-semibold tabular-nums text-txt">{fmtQuota(remaining.remain)}</span>
                {t("weLeftShort")}
              </span>
            ) : (
              t("weQuotaSub")
            )}
          </p>
          {!q.linked && <p className="mt-1 text-[12px] leading-snug text-txt3">{t("weLinkHint")}</p>}
        </div>
        {q.linked ? (
          <div className="flex shrink-0 items-center gap-2">
            <Tip text={t("weRefresh")} align="end">
              <button
                type="button"
                onClick={() => void quotaRefresh(true)}
                disabled={busy}
                className="flex h-8 items-center gap-1.5 whitespace-nowrap rounded-[8px] border border-line bg-white/[0.03] px-3 text-[12px] text-txt transition-colors hover:border-[var(--brand-line)]"
              >
                {busy ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                {t("weRefresh")}
              </button>
            </Tip>
            <button
              type="button"
              onClick={() => void quotaLogout()}
              className="flex h-8 items-center whitespace-nowrap rounded-[8px] border border-line bg-white/[0.03] px-3 text-[12px] text-txt2 transition-colors hover:border-[var(--red-line)] hover:text-txt"
            >
              {t("weUnlink")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setQuotaOpen(true)}
            className="press flex h-8 shrink-0 items-center rounded-[8px] bg-[var(--brand)] px-3 text-[12px] font-medium text-white transition-[filter] hover:brightness-110"
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
    <div className="mx-auto flex w-full max-w-[760px] flex-col pb-8">
      <header className="mb-5">
        <h1 className="text-[20px] font-semibold tracking-[-0.015em] text-txt">{t("tabSettings")}</h1>
        <p className="mt-1 text-[12.5px] leading-snug text-txt3">{t("setLead")}</p>
      </header>

      <div className="overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)] shadow-[inset_0_1px_0_rgb(255_255_255/0.045)]">
      <Group title={t("setAppGroup")}>
        <Row
          icon={Languages}
          title={t("language")}
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
      </Group>

      <Group title={t("setConnGroup")}>
        <WeQuotaRow />
      </Group>

      <Group title={t("setUpdGroup")}>
        <Row
          icon={Download}
          title={
            <span className="flex items-baseline gap-2">
              {t("chCur")}
              <span className="font-display text-[15px] font-semibold tabular-nums text-txt">{cur || "-"}</span>
            </span>
          }
          body={
            <span aria-live="polite" className="flex min-w-0 items-center gap-1.5">
              <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", upTone)} />
              <span className="truncate">{updateText}</span>
            </span>
          }
          control={
            <>
              <button
                type="button"
                onClick={() => void checkUpdates()}
                disabled={checking}
                className="flex h-8 items-center gap-1.5 whitespace-nowrap rounded-[8px] border border-line bg-white/[0.03] px-3 text-[12px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
              >
                {checking ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                {checking ? t("upChecking") : t("upCheck")}
              </button>
              {offered && (
                <button
                  type="button"
                  onClick={() => void applyUpdate()}
                  disabled={installing}
                  className="flex h-8 items-center whitespace-nowrap rounded-[8px] bg-[var(--brand)] px-3 text-[12px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
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
      </Group>
      </div>
    </div>
  )
}
