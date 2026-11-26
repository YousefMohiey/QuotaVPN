import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { ArrowDownToLine, Gauge, History as HistoryIcon, Home as HomeIcon, Settings as SettingsIcon, Users, Wifi } from "@/components/icons"
import type { ComponentType } from "react"
import { ValorantMark } from "@/components/ValorantMark"
import { cn } from "@/lib/utils"
import { useEffect, useState } from "react"
import { useI18n, type StrKey } from "@/lib/i18n"
import { api } from "@/lib/ipc"
import { useApp } from "@/state/app"
import { useTheme } from "@/state/theme"

export type Tab = "home" | "speed" | "voice" | "history" | "result" | "apps" | "network" | "spaces" | "settings"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const

const PRIMARY: Array<{ id: Tab; icon: ComponentType<{ className?: string; strokeWidth?: number; "aria-hidden"?: boolean }>; key: StrKey }> = [
  { id: "home", icon: HomeIcon, key: "tabHome" },
  { id: "voice", icon: ValorantMark, key: "tabVoice" },
  { id: "network", icon: Wifi, key: "tabNetwork" },
  { id: "spaces", icon: Users, key: "tabSpaces" },
  { id: "speed", icon: Gauge, key: "tabSpeed" },
  { id: "history", icon: HistoryIcon, key: "history" },
]

/** The rail: identity, the five places, the live state, settings. One tone
    step above the page, one hairline to the right, nothing else. */
export function Sidebar({ tab, onTab }: { tab: Tab; onTab: (t: Tab) => void }) {
  const { t } = useI18n()
  const calm = useReducedMotion()
  const { version, update, updateState } = useApp()
  const { theme, setTheme, themes } = useTheme()
  // The tunnel exists on Linux; the network scanner, the speed test and the
  // voice helper do not. Leave their doors out rather than open them onto
  // a page that cannot work.
  const [plat, setPlat] = useState("")
  useEffect(() => {
    void api
      .platform()
      .then((p) => setPlat(p))
      .catch(() => setPlat(""))
  }, [])
  const HIDDEN = plat === "linux" ? new Set<Tab>(["voice", "network", "speed"]) : new Set<Tab>()
  const visible = PRIMARY.filter((p) => !HIDDEN.has(p.id))

  const rows = [
    ...visible.map((p) => ({ ...p, active: tab === p.id || (p.id === "home" && tab === "apps") || (p.id === "history" && tab === "result") })),
    { id: "settings" as Tab, icon: SettingsIcon as typeof SettingsIcon, key: "tabSettings" as StrKey, active: tab === "settings" },
  ]

  return (
    <aside className="flex w-[250px] shrink-0 flex-col border-e border-line bg-[var(--side)]">
      {/* identity: mark, wordmark, build number on one line */}
      <div className="px-4 pb-4 pt-6">
        <div className="flex items-center gap-3">
          <img src="./icon.png" alt="" aria-hidden className="size-11 shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="truncate font-display text-lg font-semibold tracking-[-0.01em] text-txt">
              QuotaVPN
            </div>
            {version && <div className="mt-0.5 truncate font-mono text-xs text-txt3">v{version}</div>}
          </div>
        </div>

        {/* one quiet line when a new build exists: it slides in, it never blocks */}
        <AnimatePresence initial={false}>
          {updateState === "available" && update && (
            <motion.button
              type="button"
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.3, ease: EASE_OUT }}
              onClick={() => onTab("settings")}
              className="mt-3 flex w-full items-center gap-2 rounded-lg border border-line px-2 py-2 text-start transition-colors hover:border-[var(--brand-line)]"
            >
              <ArrowDownToLine className="size-3.5 shrink-0 text-brand-strong" aria-hidden />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-xs font-medium text-brand-strong" dir="auto">
                  {t("updRemind")} · v{update.latest}
                </span>
                <span className="block truncate text-xs text-txt3" dir="auto">
                  {update.notes || t("updRemindSub")}
                </span>
              </span>
            </motion.button>
          )}
        </AnimatePresence>
      </div>

      <nav className="flex flex-col gap-1 px-3" aria-label="Main">
        {rows.map(({ id, icon: Icon, key, active }) => (
          <button
            key={id}
            onClick={() => onTab(id)}
            aria-current={active ? "page" : undefined}
            className={cn(
              "relative flex h-12 items-center gap-3 rounded-xl px-3 text-sm transition-colors duration-200",
              active ? "font-medium text-txt" : "text-txt2 hover:bg-[var(--surface-2)] hover:text-txt",
            )}
          >
            {active ? (
              <motion.span
                layoutId="nav-active"
                aria-hidden
                className="absolute inset-0 rounded-xl bg-[var(--surface-3)]"
                transition={calm ? { duration: 0 } : { type: "spring", stiffness: 480, damping: 42 }}
              />
            ) : null}
            <Icon
              className={cn("relative z-10 size-[22px] shrink-0", active ? "text-brand-strong" : "text-txt2")}
              strokeWidth={1.8}
              aria-hidden
            />
            <span className="relative z-10 truncate">{t(key)}</span>
          </button>
        ))}
      </nav>

      {/* the live state sits in the hero; the rail keeps the places and the
          one control that changes the whole room */}
      <div className="mt-auto px-4 pb-6 pt-6">
        <div className="flex items-center justify-between gap-3">
          <span className="text-xs font-medium text-txt3">{t("theme")}</span>
          <div className="flex items-center gap-2" role="group" aria-label={t("theme")}>
            {themes.map((option) => {
              const active = option.id === theme
              return (
                <button
                  key={option.id}
                  type="button"
                  onClick={() => setTheme(option.id)}
                  aria-label={t(option.labelKey)}
                  aria-pressed={active}
                  title={t(option.labelKey)}
                  className={cn(
                    "size-6 rounded-full border border-white/20 transition-transform duration-200 hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-strong)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--side)]",
                    active && "ring-2 ring-[var(--brand-strong)] ring-offset-2 ring-offset-[var(--side)]",
                  )}
                  style={{ background: option.accent }}
                />
              )
            })}
          </div>
        </div>
      </div>
    </aside>
  )
}
