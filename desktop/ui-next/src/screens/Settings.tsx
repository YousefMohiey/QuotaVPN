import { useEffect, useRef, useState, type ReactNode } from "react"
import { RefreshCw } from "lucide-react"
import { Toggle } from "@/components/Toggle"
import { Segmented } from "@/components/Segmented"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { api } from "@/lib/ipc"
import { cn } from "@/lib/utils"

/** One control row: quiet label, title, one line of copy, control on the
    right. Rows sit in one card and are split by hairlines, like every other
    surface in the app. */
function Row({
  label,
  title,
  body,
  control,
  extra,
}: {
  label: string
  title: string
  body: string
  control: ReactNode
  extra?: ReactNode
}) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-4">
      <div className="min-w-0">
        <div className="text-[10.5px] font-medium tracking-[0.08em] text-txt3 uppercase">{label}</div>
        <h3 className="mt-1 text-[14px] font-semibold text-txt">{title}</h3>
        <p className="mt-0.5 max-w-[430px] text-[12.5px] leading-relaxed text-txt2">{body}</p>
        {extra}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  )
}

const AUTOSTART_CACHE = "qc-autostart"

export function Settings() {
  const { t, lang, setLang } = useI18n()
  const { update, updateState, checkUpdates, applyUpdate, updatePct } = useApp()
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
              ? t("upFail")
              : t("upCur").replace("{v}", update?.current ?? "0.2.4")

  return (
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-4">
      <div className="min-w-0">
        <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("tabSettings")}</h1>
        <p className="mt-1 text-[15px] text-txt2">{t("settingsSub")}</p>
      </div>

      <section className="divide-y divide-line rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
        <Row
          label={t("secGeneral")}
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
          label={t("secStartup")}
          title={t("startupTitle")}
          body={t("startupBody")}
          control={
            <Toggle
              on={startup}
              busy={false}
              onFlip={() => void flipStartup(!startup)}
              label={t("startupTitle")}
            />
          }
          extra={startupMsg ? <p className="mt-1 text-[12px] text-txt2">{startupMsg}</p> : undefined}
        />

        <Row
          label={t("upTitle")}
          title={t("updRow")}
          body={t("upRowBody")}
          control={
            <div className="flex items-center gap-3">
              <span aria-live="polite" className="text-[12px] text-txt3">
                {updateText}
              </span>
              <button
                type="button"
                onClick={() => void checkUpdates()}
                disabled={updateState === "checking"}
                className="flex h-9 items-center gap-2 rounded-[10px] border border-line bg-white/[0.02] px-3.5 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait"
              >
                <RefreshCw className={cn("size-3.5", updateState === "checking" && "animate-spin")} aria-hidden />
                {updateState === "checking" ? t("upChecking") : t("upCheck")}
              </button>
              {(updateState === "available" || updateState === "installing") && (
                <button
                  type="button"
                  onClick={() => void applyUpdate()}
                  disabled={updateState === "installing"}
                  className="flex h-9 items-center rounded-[10px] bg-[var(--brand-vivid)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
                >
                  {updateState === "installing"
                    ? updatePct !== null
                      ? `${updatePct}%`
                      : t("upInstalling")
                    : t("upGet")}
                </button>
              )}
            </div>
          }
        />
      </section>
    </div>
  )
}
