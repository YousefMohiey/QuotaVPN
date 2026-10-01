import { useEffect, useState } from "react"
import { RefreshCw } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { Panel, Row, GroupLabel, PageTitle } from "@/components/Row"
import { Segmented } from "@/components/Segmented"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { api } from "@/lib/ipc"

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
      <PageTitle>{t("tabSettings")}</PageTitle>

      <section>
        <GroupLabel>{t("secGeneral")}</GroupLabel>
        <Panel>
          <Row label={t("language")}>
            <Segmented
              id="lang"
              value={lang}
              onChange={(l) => setLang(l)}
              options={[
                { value: "en", label: "EN" },
                { value: "ar", label: "عربي" },
              ]}
            />
          </Row>
        </Panel>
      </section>

      <section>
        <GroupLabel>{t("secStartup")}</GroupLabel>
        <Panel>
          <Row label={t("startupTitle")} align="start">
            <div className="flex items-start justify-between gap-6">
              <div className="min-w-0">
                <p className="text-[13px] leading-relaxed text-txt2">{t("startupBody")}</p>
                {startupMsg && <p className="mt-1 text-[12px] text-txt2">{startupMsg}</p>}
              </div>
              <Switch
                checked={startup}
                disabled={startupBusy}
                onCheckedChange={() => void flipStartup(!startup)}
                aria-label={t("startupTitle")}
                className="shrink-0"
              />
            </div>
          </Row>
        </Panel>
      </section>

      <section>
        <GroupLabel>{t("upTitle")}</GroupLabel>
        <Panel>
          <Row label={t("updRow")} align="start">
            <div className="flex flex-wrap items-center gap-3">
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
              <span aria-live="polite" className="text-[12px] text-txt3">
                {updateText}
              </span>
            </div>
          </Row>
        </Panel>
      </section>
    </div>
  )
}
