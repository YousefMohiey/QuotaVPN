import { useEffect, useMemo, useRef, useState } from "react"
import { Check, ChevronRight, Search } from "@/components/icons"
import { motion, useReducedMotion } from "motion/react"
import { Segmented } from "@/components/Segmented"
import { useApp, type AppsMode } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { useAppIcons } from "@/lib/appicons"
import { cn } from "@/lib/utils"

type Row = { pkg: string; label: string }

// The App routing page in the app's card language: a breadcrumb and a real
// page header, one controls card (mode, search, bulk actions), then the
// applications card whose rows carry each app's own icon and checkbox.
// Picking any row while "All apps" is on flips the mode instead of ignoring
// the tap.
export function Apps({ onBack }: { onBack: () => void }) {
  const { t } = useI18n()
  const calm = useReducedMotion()
  const { appsMode, setAppsMode, apps, setApps, loadApps } = useApp()
  const [list, setList] = useState<Row[]>([])
  const [loading, setLoading] = useState(true)
  const [query, setQuery] = useState("")
  // Roving focus over the visible rows. DOM focus only follows moves that come
  // from the keyboard; typing in search resets the index without stealing it.
  const [focusIdx, setFocusIdx] = useState(0)
  const [listActive, setListActive] = useState(false)
  const [atEnd, setAtEnd] = useState(false)
  const rowRefs = useRef<Array<HTMLButtonElement | null>>([])
  const pendingFocus = useRef<number | null>(null)

  useEffect(() => {
    let alive = true
    void loadApps().then((l) => {
      if (!alive) return
      setList(l)
      setLoading(false)
    })
    return () => {
      alive = false
    }
  }, [loadApps])

  // The 0.2.x list was alphabetical and stayed that way.
  const sorted = useMemo(
    () => [...list].sort((a, b) => a.label.localeCompare(b.label, undefined, { sensitivity: "base" })),
    [list],
  )

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return sorted
    return sorted.filter((r) => r.label.toLowerCase().includes(q) || r.pkg.toLowerCase().includes(q))
  }, [sorted, query])

  // Focus survives filtering by moving to the first match.
  useEffect(() => {
    setFocusIdx(0)
  }, [query, sorted])

  useEffect(() => {
    if (pendingFocus.current === null || filtered.length === 0) return
    const idx = Math.min(pendingFocus.current, filtered.length - 1)
    pendingFocus.current = null
    const el = rowRefs.current[idx]
    if (el) {
      el.focus({ preventScroll: true })
      el.scrollIntoView({ block: "nearest" })
    }
  }, [focusIdx, filtered.length])

  const moveFocus = (next: number) => {
    if (filtered.length === 0) return
    const clamped = Math.max(0, Math.min(filtered.length - 1, next))
    pendingFocus.current = clamped
    setFocusIdx(clamped)
  }

  const toggle = (pkg: string) => {
    // Picking anything while "all apps" is on means the user wants a subset,
    // so flip the mode instead of ignoring the tap.
    if (appsMode === "all") {
      setAppsMode("allow")
      setApps([pkg])
      return
    }
    setApps(apps.includes(pkg) ? apps.filter((n) => n !== pkg) : [...apps, pkg])
  }

  const selectMatches = () => {
    if (appsMode === "all" || filtered.length === 0) return
    setApps(filtered.map((r) => r.pkg))
  }

  const iconOf = useAppIcons(useMemo(() => list.map((r) => r.pkg), [list]))

  const activeIdx = filtered.length === 0 ? 0 : Math.min(focusIdx, filtered.length - 1)
  const q = query.trim()
  const summary = loading
    ? t("appsLoading")
    : q
      ? t("appsSummaryMatch")
          .replace("{s}", String(apps.length))
          .replace("{t}", String(list.length))
          .replace("{m}", String(filtered.length))
          .replace("{q}", "\u2068" + q + "\u2069")
      : t("appsSummary").replace("{s}", String(apps.length)).replace("{t}", String(list.length))

  const bulkOff = appsMode === "all"

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-3">
      {/* the list owns the height and scrolls itself, the rules sit in the band */}
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <div className="flex shrink-0 items-center gap-3 border-b border-line px-6 py-4">
          <button
            type="button"
            onClick={onBack}
            aria-label={t("tabHome")}
            title={t("tabHome")}
            className="press -ms-1 grid size-7 shrink-0 place-items-center rounded-full text-txt3 transition-colors hover:bg-white/[0.05] hover:text-txt2"
          >
            <ChevronRight className="size-4 rotate-180" aria-hidden />
          </button>
          <span className="min-w-0 flex-1 truncate text-xs font-medium uppercase tracking-[0.1em] text-txt2">
            {t("routing")}
          </span>
          <span className="shrink-0 text-xs tabular-nums text-txt3" dir="auto">
            {appsMode === "all" ? t("appsEveryApp") : `${apps.length} / ${list.length} ${t("selOf")}`}
          </span>
        </div>

        <div className="relative min-h-0 flex-1">
        <div
          className="app-scroll h-full overflow-y-auto overscroll-contain"
          onScroll={(e) => {
            const el = e.currentTarget
            setAtEnd(el.scrollTop + el.clientHeight >= el.scrollHeight - 4)
          }}
          role="listbox"
          aria-multiselectable="true"
          onFocus={() => setListActive(true)}
          onBlur={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) setListActive(false)
          }}
        >
          {filtered.length === 0 ? (
            <div className="flex min-h-[240px] flex-col items-center justify-center gap-3 px-6 text-center">
              <span className="grid size-12 place-items-center rounded-full border border-line bg-white/[0.03] text-txt3">
                <Search className="size-5" strokeWidth={1.7} aria-hidden />
              </span>
              <p className="text-sm text-txt2">{loading ? t("appsLoading") : t("appsEmpty")}</p>
              {!loading && q ? (
                <button
                  type="button"
                  onClick={() => setQuery("")}
                  className="press flex h-9 items-center gap-2 rounded-lg border border-[var(--brand-line)] bg-[var(--brand-bg)] px-4 text-xs font-medium text-brand-strong transition-colors hover:bg-[var(--brand)]/20"
                >
                  {t("appsClearSearch")}
                </button>
              ) : null}
            </div>
          ) : (
            <div className="flex flex-col">
              {filtered.map((row, i) => {
                const on = appsMode === "all" || apps.includes(row.pkg)
                const tinted = appsMode !== "all" && on
                const focused = i === activeIdx && listActive
                const icon = iconOf(row.pkg)
                return (
                  <button
                    key={row.pkg}
                    ref={(el) => {
                      rowRefs.current[i] = el
                    }}
                    type="button"
                    role="checkbox"
                    aria-checked={on}
                    tabIndex={i === activeIdx ? 0 : -1}
                    onClick={() => toggle(row.pkg)}
                    onFocus={() => setFocusIdx(i)}
                    onKeyDown={(e) => {
                      if (e.key === "ArrowDown") {
                        e.preventDefault()
                        moveFocus(i + 1)
                      } else if (e.key === "ArrowUp") {
                        e.preventDefault()
                        moveFocus(i - 1)
                      } else if (e.key === "Home") {
                        e.preventDefault()
                        moveFocus(0)
                      } else if (e.key === "End") {
                        e.preventDefault()
                        moveFocus(filtered.length - 1)
                      } else if (e.key === " ") {
                        e.preventDefault()
                        toggle(row.pkg)
                      }
                    }}
                    className={cn(
                      "flex w-full scroll-mt-2 items-center gap-3 border-b border-line px-6 py-3 text-start transition-colors last:border-b-0 hover:bg-white/[0.03] focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-[var(--brand-line)]",
                      tinted ? "bg-[var(--brand)]/[0.055]" : focused && "bg-white/[0.04]",
                    )}
                  >
                    <span
                      aria-hidden
                      className="grid size-7 shrink-0 place-items-center overflow-hidden rounded-lg bg-white/[0.03]"
                    >
                      {icon ? (
                        <img src={icon} alt="" draggable={false} className="size-4 select-none" />
                      ) : (
                        <span className="text-xs font-semibold uppercase text-txt3">{row.label.slice(0, 1)}</span>
                      )}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className={cn("block truncate text-sm", on ? "text-txt" : "text-txt2")} dir="auto">
                        {row.label}
                      </span>
                      <span className="block truncate text-xs text-txt3" dir="auto">
                        {row.pkg}
                      </span>
                    </span>
                    <span
                      aria-hidden
                      className={cn(
                        "grid size-4 shrink-0 place-items-center rounded-md border transition-colors",
                        on ? "border-[var(--brand-line)] bg-[var(--brand-bg)]" : "border-line-strong",
                      )}
                    >
                      {on && (
                        <motion.span
                          initial={calm ? false : { scale: 0.6, opacity: 0 }}
                          animate={{ scale: 1, opacity: 1 }}
                          transition={calm ? { duration: 0 } : { type: "spring", stiffness: 520, damping: 30 }}
                        >
                          <Check className="size-3 text-brand-strong" strokeWidth={3} aria-hidden />
                        </motion.span>
                      )}
                    </span>
                  </button>
                )
              })}
            </div>
          )}
        </div>
        {!atEnd && filtered.length > 0 ? (
          <div
            aria-hidden
            className="pointer-events-none absolute inset-x-0 bottom-0 h-7 bg-[var(--panel)]"
          />
        ) : null}
        </div>
      </div>

      {/* the rules the list answers to: what counts, which ones, and the ways
          to set them without walking the list */}
      <section className="grid shrink-0 grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)_auto] items-start gap-6 rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-6 py-4">
        <div className="flex min-w-0 flex-col gap-2">
          <span className="text-xs font-medium uppercase tracking-[0.1em] text-txt2">{t("appsModeLbl")}</span>
          <Segmented
            id="apps-mode"
            value={appsMode}
            onChange={(m: AppsMode) => setAppsMode(m)}
            fill
            options={[
              { value: "all", label: t("appsAll") },
              { value: "allow", label: t("appsOnly") },
              { value: "block", label: t("appsExcept") },
            ]}
          />
          <p className="text-xs leading-relaxed text-txt3">{t("appsBody")}</p>
        </div>

        <div className="flex min-w-0 flex-col gap-2">
          <span className="text-xs font-medium uppercase tracking-[0.1em] text-txt2">{t("appsSearch")}</span>
          <div className="relative">
            <Search className="absolute start-2.5 top-1/2 size-3.5 -translate-y-1/2 text-txt3" aria-hidden />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setQuery("")
                else if (e.key === "ArrowDown") {
                  e.preventDefault()
                  moveFocus(0)
                }
              }}
              placeholder={t("appsSearch")}
              aria-label={t("appsSearch")}
              className="h-9 w-full rounded-lg border border-line bg-[var(--surface-2)] ps-8 pe-2 text-xs text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
            />
          </div>
        </div>

        <div className="flex min-w-0 flex-col gap-2">
          <span className="text-xs font-medium uppercase tracking-[0.1em] text-txt2">{t("appsBulkLbl")}</span>
          {bulkOff ? (
            <span className="flex h-9 items-center text-xs leading-snug text-txt3">{t("appsBulkOff")}</span>
          ) : (
            <div className="flex items-center gap-2">
              <button
                type="button"
                disabled={filtered.length === 0}
                onClick={selectMatches}
                className="press h-9 whitespace-nowrap rounded-lg border border-[var(--brand-line)] px-3 text-xs text-brand-strong transition-colors hover:bg-[var(--brand-bg)] disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
              >
                {q
                  ? t("appsSelectMatches").replace("{m}", String(filtered.length))
                  : t("appsSelectAll").replace("{t}", String(list.length))}
              </button>
              <button
                type="button"
                disabled={apps.length === 0}
                onClick={() => setApps([])}
                className="press h-9 whitespace-nowrap rounded-lg border border-line px-3 text-xs text-[var(--red)] transition-colors hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)] disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-line disabled:hover:bg-transparent disabled:hover:text-[var(--red)]"
              >
                {t("appsClearSel")}
              </button>
            </div>
          )}
        </div>
      </section>

      <p aria-live="polite" className="sr-only" dir="auto">
        {summary}
      </p>
    </div>
  )
}
