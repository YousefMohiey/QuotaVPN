import { useEffect, useRef, useState } from "react"
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

/** A titled group of settings: the label sits outside the card so the page
    reads as three things (the app, the line, the updates) instead of one long
    slab of rows. */
function Group({ label, badge, children }: { label: string; badge?: React.ReactNode; children: React.ReactNode }) {
  return (
    <section className="flex flex-col">
      <h2 className="mb-2 flex items-center gap-2 ps-1 text-[11px] font-medium uppercase tracking-[0.12em] text-txt3">
        {label}
        {badge}
      </h2>
      <div className="divide-y divide-line overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        {children}
      </div>
    </section>
  )
}

/** One setting as a hairline row: a quiet icon, the name and one line, the
    control at the right edge. The panel around the rows owns the frame. */
const AUTOSTART_CACHE = "qc-autostart"

function WeQuotaRow() {
  const { t } = useI18n()
  const q = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const busy = q.phase === "busy"

  const controls = q.linked ? (
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
      className="press flex h-8 shrink-0 items-center rounded-[7px] bg-[var(--brand)] px-3 text-[12px] font-medium text-white transition-[filter] hover:brightness-110"
    >
      {t("weLink")}
    </button>
  )

  return (
    <>
      <div className="flex items-start gap-4 px-5 py-4">
        <span className="mt-0.5 shrink-0 text-txt3">
          <WeMark className="w-[17px]" />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="text-[13px] font-medium text-txt">{t("weQuota")}</h2>
          <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("weQuotaSub")}</p>
          {!q.linked && <p className="mt-1 text-[12px] leading-snug text-txt3">{t("weLinkHint")}</p>}
        </div>
        {controls}
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

  const cur = version || update?.current
  const updateText =
    updateState === "checking"
      ? t("upChecking")
      : updateState === "latest" && update
        ? t("upLatest").replace("{v}", cur || update.latest)
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
    <div className="mx-auto flex w-full max-w-[1000px] flex-col gap-7">
      <Group label={t("setAppGroup")}>
        <div className="flex items-start gap-4 px-5 py-4">
          <Languages className="mt-0.5 size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
          <div className="min-w-0 flex-1">
            <h2 className="text-[13px] font-medium text-txt">{t("language")}</h2>
            <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("languageBody")}</p>
          </div>
          <Segmented
            id="lang"
            value={lang}
            onChange={(l) => setLang(l)}
            options={[
              { value: "en", label: "EN" },
              { value: "ar", label: "عربي" },
            ]}
          />
        </div>

        <div className="flex items-center gap-4 px-5 py-4">
          <Power className="size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
          <div className="min-w-0 flex-1">
            <h2 className="text-[13px] font-medium text-txt">{t("startupTitle")}</h2>
            <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("startupBody")}</p>
            {startupMsg ? <p className="mt-1 text-[12px] text-[var(--red)]">{startupMsg}</p> : null}
          </div>
          <div className="flex shrink-0 items-center gap-2.5">
            <span className="text-[12.5px] text-txt2">{startup ? t("wordOn") : t("wordOff")}</span>
            <Toggle on={startup} busy={false} onFlip={() => void flipStartup(!startup)} label={t("startupTitle")} />
          </div>
        </div>
      </Group>

      <Group label={t("setConnGroup")}>
        <WeQuotaRow />
      </Group>

      <Group
        label={t("setUpdGroup")}
        badge={
          updateChannel === "experimental" ? (
            <span className="rounded-full border border-[var(--brand-line)] bg-[var(--brand-bg)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.07em] text-brand-strong">
              {t("chBadge")}
            </span>
          ) : null
        }
      >
        <div className="px-5 py-4">
          {/* three columns on one rail: the label, a fixed height line that
              holds whatever the column's control is, then one caption. The
              rail is why the number, the button and the switch sit level. */}
          <div className="mt-4 grid grid-cols-1 gap-x-8 gap-y-5 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1fr)_minmax(0,1.25fr)]">
            <div className="flex min-w-0 flex-col">
              <span className="text-[11px] font-medium uppercase tracking-[0.09em] text-txt3">{t("chCur")}</span>
              <div className="mt-1.5 flex h-9 items-center">
                <span className="font-display text-[24px] font-semibold leading-none tabular-nums text-txt">{cur || "-"}</span>
              </div>
              <p aria-live="polite" className="mt-1.5 flex min-w-0 items-center gap-2 text-[12px] leading-snug text-txt3">
                <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", upTone)} />
                <span className="truncate" title={updateText}>
                  {updateText}
                </span>
              </p>
            </div>

            <div className="flex min-w-0 flex-col">
              <span className="text-[11px] font-medium uppercase tracking-[0.09em] text-txt3">{t("upManual")}</span>
              <div className="mt-1.5 flex h-9 items-center gap-2">
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
                    className="flex h-8 items-center rounded-[7px] bg-[var(--brand)] px-3 text-[12px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
                  >
                    {updateState === "installing"
                      ? updatePct !== null
                        ? `${updatePct}%`
                        : t("upInstalling")
                      : t("upGet")}
                  </button>
                )}
              </div>
              <p className="mt-1.5 text-[12px] leading-snug text-txt3">{t("upRowBody")}</p>
            </div>

            <div className="flex min-w-0 flex-col">
              <span className="text-[11px] font-medium uppercase tracking-[0.09em] text-txt3">{t("chTitle")}</span>
              <div className="mt-1.5 flex h-9 items-center">
                <Segmented
                  id="chan"
                  fill
                  value={updateChannel}
                  options={[
                    { value: "normal", label: t("chStable") },
                    { value: "experimental", label: t("chExp") },
                  ]}
                  onChange={(v) => {
                    if (v !== updateChannel) void setUpdateChannel(v)
                  }}
                  className="w-full"
                />
              </div>
              <p className="mt-1.5 text-[12px] leading-snug text-txt3">
                {updateChannel === "experimental" ? t("chExpBody") : t("chStableBody")}
              </p>
            </div>
          </div>
        </div>
      </Group>
    </div>
  )
}
