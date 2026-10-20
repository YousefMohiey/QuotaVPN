import { AnimatePresence, motion } from "motion/react"
import { ArrowDownToLine, Gauge, History as HistoryIcon, Home as HomeIcon, Settings as SettingsIcon } from "lucide-react"
import type { ComponentType } from "react"
import { ValorantMark } from "@/components/ValorantMark"
import { cn } from "@/lib/utils"
import { useI18n, type StrKey } from "@/lib/i18n"
import { useApp } from "@/state/app"

export type Tab = "home" | "speed" | "voice" | "history" | "result" | "apps" | "settings"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const

const PRIMARY: Array<{ id: Tab; icon: ComponentType<{ className?: string; strokeWidth?: number; "aria-hidden"?: boolean }>; key: StrKey }> = [
  { id: "home", icon: HomeIcon, key: "tabHome" },
  { id: "voice", icon: ValorantMark, key: "tabVoice" },
  { id: "speed", icon: Gauge, key: "tabSpeed" },
  { id: "history", icon: HistoryIcon, key: "history" },
]

/** The rail: identity, the four places, the live state, settings. One tone
    step above the page, one hairline to the right, nothing else. */
export function Sidebar({ tab, onTab }: { tab: Tab; onTab: (t: Tab) => void }) {
  const { t } = useI18n()
  const { version, update, updateState } = useApp()

  const rows = [
    ...PRIMARY.map((p) => ({ ...p, active: tab === p.id || (p.id === "home" && tab === "apps") || (p.id === "history" && tab === "result") })),
    { id: "settings" as Tab, icon: SettingsIcon as typeof SettingsIcon, key: "tabSettings" as StrKey, active: tab === "settings" },
  ]

  return (
    <aside className="flex w-[250px] shrink-0 flex-col border-e border-line bg-[var(--side)]">
      {/* identity: mark, wordmark, build number on one line */}
      <div className="px-4 pb-4 pt-5">
        <div className="flex items-center gap-3">
          <img src="./icon.png" alt="" aria-hidden className="size-11 shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-1.5">
              <span className="truncate font-display text-[17px] font-semibold tracking-[-0.01em] text-txt">QuotaVPN</span>
              {version && <span className="shrink-0 font-mono text-[11px] text-txt3">v{version}</span>}
            </div>
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
              className="mt-2.5 flex w-full items-center gap-2 rounded-[8px] border border-line px-2 py-1.5 text-start transition-colors hover:border-[var(--brand-line)]"
            >
              <ArrowDownToLine className="size-3.5 shrink-0 text-brand-strong" aria-hidden />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[11px] font-medium text-brand-strong" dir="auto">
                  {t("updRemind")} · v{update.latest}
                </span>
                <span className="block truncate text-[11px] text-txt3" dir="auto">
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
              "flex h-12 items-center gap-3 rounded-[10px] px-3 text-[15px] transition-colors duration-150",
              active ? "bg-white/[0.055] font-medium text-txt" : "text-txt2 hover:bg-white/[0.03] hover:text-txt",
            )}
          >
            <Icon className={cn("size-[22px] shrink-0", active ? "text-brand-strong" : "text-txt2")} strokeWidth={1.8} aria-hidden />
            <span className="truncate">{t(key)}</span>
          </button>
        ))}
      </nav>

      {/* the live state sits in the hero; the rail keeps only the places */}
    </aside>
  )
}
