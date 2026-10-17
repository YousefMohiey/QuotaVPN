import { useEffect, useRef, useState, type ReactNode } from "react"
import { FlaskConical, Languages, Power, RefreshCw, ShieldCheck } from "lucide-react"
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

/** One setting as its own card, built like the voice chat cards: a
    hairline icon chip, a bold title, one quiet line under it, and the
    control at the right edge. */
function SettingCard({
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
    <section className="flex items-center gap-4 rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <span className="grid size-12 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
        {icon}
      </span>
      <div className="min-w-0 flex-1">
        <h2 className="text-[15px] font-semibold text-txt">{title}</h2>
        <div className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{sub}</div>
      </div>
      <div className="shrink-0">{control}</div>
    </section>
  )
}

const AUTOSTART_CACHE = "qc-autostart"

/** The WE quota card: link the landline once, the remaining reads itself. */
function WeQuotaCard() {
  const { t } = useI18n()
  const q = useQuota()
  const [quotaOpen, setQuotaOpen] = useState(false)
  const busy = q.phase === "busy"

  return (
    <section className="rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <div className="flex items-center gap-4">
        <span className="grid size-12 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
          <WeMark className="w-[27px]" />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="text-[15px] font-semibold text-txt">{t("weQuota")}</h2>
          <p className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">
            {t("weQuotaSub")}
          </p>
        </div>
        {q.linked && (
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              onClick={() => void quotaRefresh(true)}
              disabled={busy}
              className="flex h-9 items-center gap-2 rounded-[10px] border border-line bg-white/[0.02] px-3.5 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-wait"
            >
              <RefreshCw className={cn("size-3.5", busy && "animate-spin")} aria-hidden />
              {t("weRefresh")}
            </button>
            <button
              type="button"
              onClick={() => {
                void quotaLogout()
              }}
              className="flex h-9 items-center rounded-[10px] border border-line bg-white/[0.02] px-3.5 text-[13px] text-txt2 transition-colors hover:text-txt"
            >
              {t("weUnlink")}
            </button>
          </div>
        )}
      </div>

      {!q.linked && (
        <div className="mt-3 flex items-center gap-3">
          <button
            type="button"
            onClick={() => setQuotaOpen(true)}
            className="flex h-9 items-center rounded-[10px] bg-[var(--brand-vivid)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110"
          >
            {t("weLink")}
          </button>
          <p className="text-[11.5px] text-txt3">{t("weLinkHint")}</p>
        </div>
      )}
      <WeQuotaDialog open={quotaOpen} onOpenChange={setQuotaOpen} />
    </section>
  )
}

export function Settings() {
  const { t, lang, setLang } = useI18n()
  const { update, updateState, checkUpdates, applyUpdate, updatePct, version, updateChannel, setUpdateChannel } =
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
              ? t("upFail")
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

  /** One release channel as a selectable card: icon, name, one quiet line,
      and the radio dot at the edge. Switching it re-checks right away. */
  const channelCard = (ch: "normal" | "experimental") => {
    const isActive = updateChannel === ch
    const Icon = ch === "experimental" ? FlaskConical : ShieldCheck
    return (
      <button
        key={ch}
        type="button"
        aria-pressed={isActive}
        onClick={() => {
          if (!isActive) void setUpdateChannel(ch)
        }}
        className={cn(
          "flex items-start gap-3 rounded-[12px] border p-3.5 text-start transition-colors duration-200",
          isActive
            ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
            : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",
        )}
      >
        <span
          className={cn(
            "grid size-10 shrink-0 place-items-center rounded-[10px] border transition-colors",
            isActive
              ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
              : "border-line bg-white/[0.03] text-txt3",
          )}
        >
          <Icon className="size-[18px]" strokeWidth={1.8} aria-hidden />
        </span>
        <span className="min-w-0 flex-1">
          <span className={cn("block text-[14px] font-semibold", isActive ? "text-txt" : "text-txt2")}>
            {ch === "experimental" ? t("chExp") : t("chStable")}
          </span>
          <span className="mt-0.5 block text-[11.5px] leading-snug text-txt3">
            {ch === "experimental" ? t("chExpBody") : t("chStableBody")}
          </span>
        </span>
        <span
          aria-hidden
          className={cn(
            "mt-0.5 grid size-4 shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
            isActive ? "border-[var(--brand)]" : "border-line-strong",
          )}
        >
          {isActive && <span className="size-[7px] rounded-full bg-[var(--brand)]" />}
        </span>
      </button>
    )
  }

  return (
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-3">
      <div className="mb-1 min-w-0">
        <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("tabSettings")}</h1>
        <p className="mt-1 text-[15px] text-txt2">{t("settingsSub")}</p>
      </div>

      {/* the cards sit right under the header, as the owner wants */}
      <div className="flex flex-col gap-3">
      <WeQuotaCard />

      <SettingCard
        icon={<Languages className="size-5" strokeWidth={1.7} aria-hidden />}
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

      <SettingCard
        icon={<Power className="size-5" strokeWidth={1.7} aria-hidden />}
        title={t("startupTitle")}
        sub={
          <>
            {t("startupBody")}
            {startupMsg ? <p className="mt-1 text-[12px] text-[var(--red)]">{startupMsg}</p> : null}
          </>
        }
        control={
          <Toggle
            on={startup}
            busy={false}
            onFlip={() => void flipStartup(!startup)}
            label={t("startupTitle")}
          />
        }
      />

      <section className="rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
        <div className="flex items-center gap-4">
          <span className="grid size-12 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
            <RefreshCw className="size-5" strokeWidth={1.7} aria-hidden />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h2 className="text-[15px] font-semibold text-txt">{t("updRow")}</h2>
              {updateChannel === "experimental" ? (
                <span className="rounded-full border border-[var(--brand-line)] bg-[var(--brand-bg)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.07em] text-brand-strong">
                  {t("chBadge")}
                </span>
              ) : null}
            </div>
            <div className="mt-0.5 text-[12.5px] leading-relaxed text-txt2">{t("upRowBody")}</div>
          </div>
          <div className="flex shrink-0 items-center gap-3">
            <span aria-live="polite" className="flex items-center gap-2 text-[12px] text-txt3">
              {updateText ? (
                <>
                  <span aria-hidden className={cn("size-1.5 rounded-full", upTone)} />
                  {updateText}
                </>
              ) : null}
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
        </div>

        <div className="mt-4 border-t border-line pt-4">
          <p className="text-[12px] font-semibold uppercase tracking-[0.07em] text-txt2">{t("chTitle")}</p>
          <div className="mt-3 grid grid-cols-2 gap-3" role="group" aria-label={t("chTitle")}>
            {channelCard("normal")}
            {channelCard("experimental")}
          </div>
        </div>
      </section>
      </div>
    </div>
  )
}
