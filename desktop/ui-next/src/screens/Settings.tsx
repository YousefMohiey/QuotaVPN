import { useEffect, useRef, useState, type ReactNode } from "react"
import { Languages, Power, RefreshCw } from "lucide-react"
import { Toggle } from "@/components/Toggle"
import { WeMark } from "@/components/WeMark"
import { Segmented } from "@/components/Segmented"
import { useApp } from "@/state/app"
import {
  quotaLogout,
  quotaRefresh,
  useQuota,
} from "@/state/quota"
import { WeQuotaDialog } from "@/components/WeQuotaDialog"
import { useI18n } from "@/lib/i18n"
import { api } from "@/lib/ipc"
import { cn } from "@/lib/utils"

/** One setting as a hairline row: a quiet icon, the name and one line, the
    control at the right edge. The panel around the rows owns the frame. */
function SettingRow({
  icon,
  title,
  sub,
  control,
}: {
  icon: ReactNode
  title: string
  sub: ReactNode
  control: ReactNode
}) {
  return (
    <div className="flex items-center gap-4 px-5 py-3.5">
      <span className="shrink-0 text-txt3">{icon}</span>
      <div className="min-w-0 flex-1">
        <h2 className="text-[12.5px] font-medium text-txt">{title}</h2>
        <div className="mt-0.5 text-[12px] leading-relaxed text-txt2">{sub}</div>
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  )
}

const AUTOSTART_CACHE = "qc-autostart"

/** The WE line, as a manage row: link it once, then refresh or unlink. */
function WeQuotaRow() {
  const { t } = useI18n()
  const q = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const busy = q.phase === "busy"

  return (
    <>
      <div className="flex items-center gap-4 px-5 py-3.5">
        <span className="shrink-0 text-txt3">
          <WeMark className="w-[18px]" />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="text-[12.5px] font-medium text-txt">{t("weQuota")}</h2>
          <p className="mt-0.5 text-[12px] leading-relaxed text-txt2">{t("weQuotaSub")}</p>
          {!q.linked && <p className="mt-1 text-[11px] leading-snug text-txt3">{t("weLinkHint")}</p>}
        </div>
        {q.linked ? (
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              onClick={() => void quotaRefresh(true)}
              disabled={busy}
              className="flex h-8 items-center gap-1.5 rounded-[7px] border border-line bg-white/[0.02] px-2.5 text-[12px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
            >
              <RefreshCw className={cn("size-3", busy && "animate-spin")} aria-hidden />
              {t("weRefresh")}
            </button>
            <button
              type="button"
              onClick={() => {
                void quotaLogout()
              }}
              className="flex h-8 items-center rounded-[7px] border border-line bg-white/[0.02] px-2.5 text-[12px] text-txt2 transition-colors hover:text-txt"
            >
              {t("weUnlink")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setQuotaOpen(true)}
            className="flex h-8 shrink-0 items-center rounded-[7px] bg-[var(--brand-vivid)] px-3 text-[12px] font-medium text-white transition-[filter] hover:brightness-110"
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
  // The cached value paints on the first frame; the engine read (a registry
  // lookup now) reconciles a moment later.
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
    // Flip first: the switch must answer the click on the same frame. A
    // failure reverts the flip and reports it.
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

  const cur = update?.current ?? version
  const updateText =
    updateState === "checking"
      ? t("upChecking")
      : updateState === "latest" && update
        ? t("upLatest").replace("{v}", update.latest)
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

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1120px] flex-col gap-3">
      <div className="min-w-0">
        <h1 className="font-display text-[22px] font-semibold leading-tight tracking-[-0.02em] text-txt">{t("tabSettings")}</h1>
        <p className="mt-1 text-[12.5px] text-txt2">{t("settingsSub")}</p>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
        {/* left: the preferences, hairline rows in one slab */}
        <section className="flex flex-col self-start divide-y divide-line overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
          <SettingRow
            icon={<Languages className="size-4" strokeWidth={1.7} aria-hidden />}
            title={t("language")}
            sub={t("languageBody")}
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

          <SettingRow
            icon={<Power className="size-4" strokeWidth={1.7} aria-hidden />}
            title={t("startupTitle")}
            sub={
              <>
                {t("startupBody")}
                {startupMsg ? <p className="mt-1 text-[11.5px] text-[var(--red)]">{startupMsg}</p> : null}
              </>
            }
            control={<Toggle on={startup} busy={false} onFlip={() => void flipStartup(!startup)} label={t("startupTitle")} />}
          />

          <WeQuotaRow />
        </section>

        {/* right: the release panel, the version at scale */}
        <section className="flex min-h-0 flex-col self-start overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
          <div className="flex items-center gap-2.5 border-b border-line px-5 py-3">
            <RefreshCw className="size-3.5 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
            <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-txt">{t("updRow")}</span>
            {updateChannel === "experimental" ? (
              <span className="shrink-0 rounded-full border border-[var(--brand-line)] bg-[var(--brand-bg)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.07em] text-brand-strong">
                {t("chBadge")}
              </span>
            ) : null}
            {updateText ? (
              <span aria-live="polite" className="flex min-w-0 max-w-[180px] items-center gap-2 text-[11px] text-txt2">
                <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", upTone)} />
                <span className="truncate" title={updateText}>
                  {updateText}
                </span>
              </span>
            ) : null}
          </div>

          <div className="flex flex-col items-center gap-4 px-5 py-7 text-center">
            <div className="flex flex-col items-center gap-1.5">
              <span className="font-display text-[30px] font-semibold leading-none tabular-nums text-txt">{cur || "-"}</span>
              <span className="text-[10.5px] font-medium uppercase tracking-[0.09em] text-txt3">{t("chCur")}</span>
            </div>
            <div className="flex flex-wrap items-center justify-center gap-2">
              <button
                type="button"
                onClick={() => void checkUpdates()}
                disabled={updateState === "checking"}
                className="flex h-8 items-center gap-1.5 rounded-[7px] border border-line bg-white/[0.02] px-3 text-[12px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:cursor-wait"
              >
                <RefreshCw className={cn("size-3", updateState === "checking" && "animate-spin")} aria-hidden />
                {updateState === "checking" ? t("upChecking") : t("upCheck")}
              </button>
              {(updateState === "available" || updateState === "installing") && (
                <button
                  type="button"
                  onClick={() => void applyUpdate()}
                  disabled={updateState === "installing"}
                  className="flex h-8 items-center rounded-[7px] bg-[var(--brand-vivid)] px-3 text-[12px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
                >
                  {updateState === "installing"
                    ? updatePct !== null
                      ? `${updatePct}%`
                      : t("upInstalling")
                    : t("upGet")}
                </button>
              )}
            </div>
            <p className="max-w-[300px] text-[11.5px] leading-relaxed text-txt3">{t("upRowBody")}</p>
          </div>

          <div className="mt-auto border-t border-line px-5 py-3.5">
            <p className="text-[10.5px] font-medium uppercase tracking-[0.09em] text-txt3">{t("chTitle")}</p>
            <p className="mt-1 text-[12px] leading-snug text-txt2">
              {updateChannel === "experimental" ? t("chExpBody") : t("chStableBody")}
            </p>
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
              className="mt-3"
            />
          </div>
        </section>
      </div>
    </div>
  )
}
