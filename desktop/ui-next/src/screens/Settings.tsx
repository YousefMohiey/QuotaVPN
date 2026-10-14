import { useEffect, useRef, useState, type ReactNode } from "react"
import { Gauge, Languages, Power, RefreshCw } from "lucide-react"
import { Toggle } from "@/components/Toggle"
import { Segmented } from "@/components/Segmented"
import { useApp } from "@/state/app"
import {
  fmtQuota,
  quotaLink,
  quotaLogout,
  quotaRefresh,
  savedQuotaPass,
  useQuota,
} from "@/state/quota"
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
  const [number, setNumber] = useState(q.number)
  const [password, setPassword] = useState("")
  const [code, setCode] = useState("")
  const busy = q.phase === "busy"

  const link = (withCode: string) =>
    quotaLink(number.trim(), password || savedQuotaPass(), withCode)

  return (
    <section className="rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <div className="flex items-center gap-4">
        <span className="grid size-12 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
          <Gauge className="size-5" strokeWidth={1.7} aria-hidden />
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
                setPassword("")
                setCode("")
              }}
              className="flex h-9 items-center rounded-[10px] border border-line bg-white/[0.02] px-3.5 text-[13px] text-txt2 transition-colors hover:text-txt"
            >
              {t("weUnlink")}
            </button>
          </div>
        )}
      </div>

      {q.linked ? (
        <div className="mt-3 flex flex-col gap-2.5">
          {q.units.map((u, i) => {
            const pct = u.total > 0 ? Math.max(0, Math.min(100, (u.remain / u.total) * 100)) : 0
            return (
              <div key={`${u.offer}-${i}`}>
                <div className="flex items-baseline justify-between gap-3 text-[12.5px]">
                  <span className="truncate text-txt2">{u.offer || "WE"}</span>
                  <span className="shrink-0 text-txt">
                    {fmtQuota(u.remain)} {t("weLeft")} {fmtQuota(u.total)}
                  </span>
                </div>
                <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
                  <div
                    className="h-full rounded-full bg-[var(--brand-vivid)]"
                    style={{ width: `${pct}%` }}
                  />
                </div>
                <p className="mt-0.5 text-end text-[11px] text-txt3">
                  {t("weUsed")} {fmtQuota(u.used)}
                </p>
              </div>
            )
          })}
          {q.updatedAt ? (
            <p className="text-[11.5px] text-txt3">
              {t("weUpdated")} {new Date(q.updatedAt).toLocaleTimeString()}
            </p>
          ) : null}
        </div>
      ) : (
        <div className="mt-3 flex flex-col gap-2.5">
          <div className="grid grid-cols-2 gap-2.5">
            <input
              value={number}
              onChange={(e) => setNumber(e.target.value)}
              inputMode="tel"
              autoComplete="off"
              placeholder={t("weNumber")}
              aria-label={t("weNumber")}
              className="h-9 w-full rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
            <input
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              type="password"
              autoComplete="off"
              placeholder={t("wePassword")}
              aria-label={t("wePassword")}
              className="h-9 w-full rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
          </div>
          {q.phase === "need-code" && (
            <div className="flex items-center gap-2.5">
              {q.challengeImage ? (
                <img
                  src={q.challengeImage}
                  alt={t("weCode")}
                  className="h-9 shrink-0 rounded-[8px] border border-line"
                />
              ) : null}
              <input
                value={code}
                onChange={(e) => setCode(e.target.value)}
                autoComplete="off"
                placeholder={t("weCode")}
                aria-label={t("weCode")}
                className="h-9 w-full rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
              />
              <button
                type="button"
                onClick={() => void link(code.trim())}
                disabled={busy || !code.trim()}
                className="flex h-9 shrink-0 items-center rounded-[10px] bg-[var(--brand-vivid)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
              >
                {t("weConfirm")}
              </button>
            </div>
          )}
          <div className="flex items-center gap-3">
            {q.phase !== "need-code" && (
              <button
                type="button"
                onClick={() => void link("")}
                disabled={busy || !number.trim()}
                className="flex h-9 items-center rounded-[10px] bg-[var(--brand-vivid)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-wait"
              >
                {t("weLink")}
              </button>
            )}
            <p className="text-[11.5px] text-txt3">{t("weLinkHint")}</p>
          </div>
          {q.phase === "error" && q.msgKey ? (
            <p className="text-[12px] text-[var(--red)]">
              {t(q.msgKey as never)}
              {q.msgRaw ? ` (${q.msgRaw})` : ""}
            </p>
          ) : null}
        </div>
      )}
    </section>
  )
}

export function Settings() {
  const { t, lang, setLang } = useI18n()
  const { update, updateState, checkUpdates, applyUpdate, updatePct, version } = useApp()
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

      <SettingCard
        icon={<RefreshCw className="size-5" strokeWidth={1.7} aria-hidden />}
        title={t("updRow")}
        sub={t("upRowBody")}
        control={
          <div className="flex items-center gap-3">
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
        }
      />
      </div>
    </div>
  )
}
