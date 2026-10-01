import { useEffect, useState, type ReactNode } from "react"
import { Power, RefreshCw, Settings2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Toggle } from "@/components/Toggle"
import { Segmented } from "@/components/Segmented"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { api } from "@/lib/ipc"

/** One settings card: icon chip plus title above an inset control row. */
function Card({
  icon,
  title,
  sub,
  children,
}: {
  icon: ReactNode
  title: string
  sub: string
  children: ReactNode
}) {
  return (
    <section className="glass rounded-[20px] p-4">
      <div className="flex items-center gap-3 px-1 pb-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-[12px] bg-field text-brand-strong">
          {icon}
        </span>
        <div className="min-w-0">
          <h2 className="text-[15px] font-semibold text-txt">{title}</h2>
          <p className="mt-0.5 text-[12.5px] text-txt2">{sub}</p>
        </div>
      </div>
      <div className="rounded-[14px] border border-line bg-black/25 px-4 py-3">{children}</div>
    </section>
  )
}

export function Settings() {
  const { t, lang, setLang } = useI18n()
  const { update, updateState, checkUpdates, applyUpdate, updatePct } = useApp()
  const [startup, setStartup] = useState(false)
  const [startupBusy, setStartupBusy] = useState(false)
  const [startupMsg, setStartupMsg] = useState("")

  useEffect(() => {
    let alive = true
    api
      .autostartGet()
      .then((v) => {
        if (alive) setStartup(v === true)
      })
      .catch(() => {
        /* engine without the setting: stays off */
      })
    return () => {
      alive = false
    }
  }, [])

  const flipStartup = async (next: boolean) => {
    if (startupBusy) return
    setStartupBusy(true)
    setStartupMsg("")
    try {
      await api.autostartSet(next)
      setStartup(next)
    } catch {
      setStartupMsg(t("startupFail"))
    } finally {
      setStartupBusy(false)
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
    <div className="flex flex-col gap-4">
      <div className="px-1">
        <h1 className="text-[22px] font-semibold text-txt">{t("tabSettings")}</h1>
        <p className="mt-0.5 text-[13px] text-txt2">{t("settingsSub")}</p>
      </div>

      <Card
        icon={<Settings2 className="size-5" aria-hidden />}
        title={t("secGeneral")}
        sub={t("secGeneralSub")}
      >
        <div className="flex items-center justify-between gap-6">
          <div className="min-w-0">
            <h3 className="text-[13.5px] font-semibold text-txt">{t("language")}</h3>
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
      </Card>

      <Card
        icon={<Power className="size-5" aria-hidden />}
        title={t("secStartup")}
        sub={t("secStartupSub")}
      >
        <div className="flex items-center justify-between gap-6">
          <div className="min-w-0">
            <h3 className="text-[13.5px] font-semibold text-txt">{t("startupTitle")}</h3>
            <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("startupBody")}</p>
            {startupMsg && <p className="mt-1 text-[12px] text-txt2">{startupMsg}</p>}
          </div>
          <Toggle
            on={startup}
            busy={startupBusy}
            onFlip={() => void flipStartup(!startup)}
            label={t("startupTitle")}
          />
        </div>
      </Card>

      <Card
        icon={<RefreshCw className="size-5" aria-hidden />}
        title={t("upTitle")}
        sub={t("secUpdatesSub")}
      >
        <div className="flex items-center justify-between gap-6">
          <div className="min-w-0">
            <h3 className="text-[13.5px] font-semibold text-txt">{t("updRow")}</h3>
            <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("upRowBody")}</p>
          </div>
          <div className="flex shrink-0 items-center gap-3">
            <span aria-live="polite" className="text-[12px] text-txt3">
              {updateText}
            </span>
            <Button
              variant="secondary"
              className="h-9 gap-2 rounded-[10px] px-3.5 text-[13px]"
              onClick={() => void checkUpdates()}
              disabled={updateState === "checking"}
            >
              <RefreshCw className="size-4" aria-hidden />
              {updateState === "checking" ? t("upChecking") : t("upCheck")}
            </Button>
            {(updateState === "available" || updateState === "installing") && (
              <Button
                className="h-9 rounded-[10px] px-3.5 text-[13px]"
                onClick={() => void applyUpdate()}
                disabled={updateState === "installing"}
              >
                {updateState === "installing"
                  ? updatePct !== null
                    ? `${updatePct}%`
                    : t("upInstalling")
                  : t("upGet")}
              </Button>
            )}
          </div>
        </div>
      </Card>
    </div>
  )
}
