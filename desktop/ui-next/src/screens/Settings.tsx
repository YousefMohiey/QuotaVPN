import { useEffect, useRef, useState } from "react"
import { Languages, Loader2, MonitorPlay, RefreshCw } from "lucide-react"
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
 * Settings is a board: the switches on the left, the readings they produce on
 * the right.
 *
 * The five settings never fill a desktop window on their own, and a column of
 * five rows leaves two thirds of the screen empty however it is spaced. The
 * right rail carries what those switches actually report, so the screen is full
 * of facts instead of padding.
 */
function Group({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-1 flex-col">
      <h2 className="border-b border-line px-5 py-3 text-[11px] font-semibold uppercase tracking-[0.14em] text-txt2">
        {label}
      </h2>
      <div className="flex flex-1 flex-col justify-center">{children}</div>
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
    <div className="flex items-center gap-3.5 border-b border-line px-5 py-4 last:border-b-0">
      <span className="grid size-8 shrink-0 place-items-center text-txt2">
        <Icon className="size-6" strokeWidth={1.7} aria-hidden />
      </span>
      <div className="min-w-0 flex-1">
        <h3 className="text-[14px] font-medium text-txt">{title}</h3>
        {body ? <p className="mt-1 break-words text-[13px] leading-relaxed text-txt2">{body}</p> : null}
        {note ? <div className="mt-1 text-[12.5px] leading-snug text-txt3">{note}</div> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2">{control}</div>
    </div>
  )
}

function Rail({ children, label, className }: { children: React.ReactNode; label: string; className?: string }) {
  return (
    <section className={cn("flex flex-col rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-5 py-4", className)}>
      <h2 className="text-[11px] font-semibold uppercase tracking-[0.14em] text-txt3">{label}</h2>
      <div className="mt-3 flex flex-1 flex-col">{children}</div>
    </section>
  )
}

const AUTOSTART_CACHE = "qc-autostart"

export function Settings() {
  const { t, lang, setLang } = useI18n()
  const { update, updateState, updateError, checkUpdates, applyUpdate, updatePct, version, updateChannel, setUpdateChannel } =
    useApp()
  const q = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
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
  const remaining = q.units.find((u) => u.total > 0)
  const share = remaining && remaining.total > 0 ? Math.max(0, Math.min(1, remaining.remain / remaining.total)) : 0
  const busy = q.phase === "busy"
  const checking = updateState === "checking"
  const installing = updateState === "installing"
  const offered = updateState === "available" || installing

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

  return (
    <div className="grid h-full min-h-0 w-full grid-cols-[minmax(0,1.55fr)_minmax(0,1fr)] gap-5 max-[980px]:grid-cols-1 max-[980px]:overflow-y-auto">
      {/* the switches */}
      <div className="flex min-h-0 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <Group label={t("setAppGroup")}>
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
        </Group>

        <Group label={t("setConnGroup")}>
          <div className="flex items-center gap-3.5 px-5 py-4">
            <span className="grid size-8 shrink-0 place-items-center text-txt2">
              <WeMark className="w-6" />
            </span>
            <div className="min-w-0 flex-1">
              <h3 className="text-[14px] font-medium text-txt">{t("weQuota")}</h3>
              <p className="mt-1 text-[13px] leading-relaxed text-txt2">{t("weQuotaSub")}</p>
              {!q.linked && <p className="mt-1 text-[12.5px] leading-snug text-txt3">{t("weLinkHint")}</p>}
            </div>
            {q.linked ? (
              <div className="flex shrink-0 items-center gap-2">
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
                  className="flex h-10 items-center whitespace-nowrap rounded-[9px] border border-line px-4 text-[13px] text-txt2 transition-colors hover:border-[var(--red-line)] hover:text-txt"
                >
                  {t("weUnlink")}
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setQuotaOpen(true)}
                className="press flex h-10 shrink-0 items-center rounded-[9px] bg-[var(--brand)] px-5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110"
              >
                {t("weLink")}
              </button>
            )}
          </div>
        </Group>

        <Group label={t("setUpdGroup")}>
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

      {/* what the switches report */}
      <div className="flex min-h-0 flex-col gap-5">
        <Rail label={t("setConnGroup")} className="flex-1">
          <div className="flex flex-1 flex-col justify-center">
            <div className="flex items-baseline gap-2">
              <span className="font-display text-[32px] font-semibold leading-none tabular-nums text-txt">
                {q.linked && remaining ? fmtQuota(remaining.remain) : "--"}
              </span>
              <span className="text-[13px] text-txt2">{t("weLeftShort")}</span>
            </div>
            <div className="mt-4 h-1.5 w-full overflow-hidden rounded-full bg-white/[0.07]">
              <div
                className="h-full rounded-full bg-[var(--brand-vivid)] transition-[width] duration-500"
                style={{ width: `${Math.round(share * 100)}%` }}
              />
            </div>
            <div className="mt-2 flex items-center justify-between text-[12px] text-txt3">
              <span>{q.linked ? t("weLinked") : t("weNotLinked")}</span>
              {remaining ? <span className="tabular-nums">{Math.round(share * 100)}%</span> : null}
            </div>
          </div>
          {!q.linked && (
            <button
              type="button"
              onClick={() => setQuotaOpen(true)}
              className="press mt-4 flex h-10 w-full items-center justify-center rounded-[9px] bg-[var(--brand)] text-[13px] font-medium text-white transition-[filter] hover:brightness-110"
            >
              {t("weLink")}
            </button>
          )}
        </Rail>

        <Rail label={t("setUpdGroup")} className="flex-1">
          <div className="flex flex-1 flex-col justify-center gap-3">
            <div className="flex items-baseline justify-between">
              <span className="text-[12.5px] text-txt3">{t("chCur")}</span>
              <span className="font-display text-[16px] font-semibold tabular-nums text-txt">{cur || "-"}</span>
            </div>
            <div className="flex items-center gap-2 border-t border-line pt-3" aria-live="polite">
              <span
                aria-hidden
                className={cn(
                  "size-1.5 shrink-0 rounded-full",
                  updateState === "latest"
                    ? "bg-[var(--green)]"
                    : updateState === "error"
                      ? "bg-[var(--red)]"
                      : "bg-[var(--brand-vivid)]",
                )}
              />
              <span className="text-[12.5px] text-txt2">{updateText}</span>
            </div>
            <div className="mt-3 flex gap-2">
              <button
                type="button"
                onClick={() => void checkUpdates()}
                disabled={checking}
                className="flex h-10 flex-1 items-center justify-center gap-2 whitespace-nowrap rounded-[9px] border border-line bg-white/[0.03] text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
              >
                {checking ? <Loader2 className="size-3 animate-spin" aria-hidden /> : <RefreshCw className="size-3" aria-hidden />}
                {checking ? t("upChecking") : t("upCheck")}
              </button>
              {offered && (
                <button
                  type="button"
                  onClick={() => void applyUpdate()}
                  disabled={installing}
                  className="flex h-10 flex-1 items-center justify-center whitespace-nowrap rounded-[9px] bg-[var(--brand)] text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
                >
                  {installing ? (updatePct !== null ? `${updatePct}%` : t("upInstalling")) : t("upGet")}
                </button>
              )}
            </div>
          </div>
        </Rail>
      </div>

      <WeQuotaDialog open={quotaOpen} onOpenChange={setQuotaOpen} />
    </div>
  )
}
