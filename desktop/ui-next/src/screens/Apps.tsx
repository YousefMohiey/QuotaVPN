import { useEffect, useMemo, useRef, useState } from "react"
import { Check, ChevronRight, Info, Search } from "lucide-react"
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
  const { appsMode, setAppsMode, apps, setApps, loadApps } = useApp()
  const [list, setList] = useState<Row[]>([])
  const [loading, setLoading] = useState(true)
  const [query, setQuery] = useState("")
  // Roving focus over the visible rows. DOM focus only follows moves that come
  // from the keyboard; typing in search resets the index without stealing it.
  const [focusIdx, setFocusIdx] = useState(0)
  const [listActive, setListActive] = useState(false)
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
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">
      <div className="px-1">
        <nav aria-label="Breadcrumb" className="flex items-center gap-0.5 text-[12.5px]">
          <button
            type="button"
            onClick={onBack}
            className="rounded-[8px] px-1.5 py-0.5 text-txt3 transition-colors hover:bg-white/[0.04] hover:text-txt2"
          >
            {t("tabHome")}
          </button>
          <ChevronRight className="size-3.5 shrink-0 text-txt3" aria-hidden />
          <span className="px-1.5 py-0.5 text-txt2">{t("routing")}</span>
        </nav>
        <h1 className="mt-2 text-[30px] font-semibold leading-tight text-txt">{t("routing")}</h1>
        <p className="mt-1 text-[15px] text-txt2">{t("appsBody")}</p>
        <p className="mt-0.5 text-[13.5px] text-txt3">{t("appsHint")}</p>
      </div>

      {/* the controls stay up while the list scrolls beneath them */}
      <div className="sticky top-0 z-10 bg-[var(--bg)] pb-3">
        <div className="overflow-hidden rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
          <div className="flex flex-wrap items-center gap-2 p-3">
            <span className="flex items-center gap-1.5 text-[13.5px] font-semibold text-txt">
              {t("appsModeLbl")}
              <Info className="size-3.5 text-txt3" aria-hidden />
            </span>
            <Segmented
              id="apps-mode"
              value={appsMode}
              onChange={(m: AppsMode) => setAppsMode(m)}
              options={[
                { value: "all", label: t("appsAll") },
                { value: "allow", label: t("appsOnly") },
                { value: "block", label: t("appsExcept") },
              ]}
            />
            <span aria-hidden className="hidden h-6 w-px shrink-0 bg-white/[0.08] lg:block" />
            <div className="relative min-w-[120px] flex-1">
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
                className="h-9 w-full rounded-[10px] border border-line bg-white/[0.02] ps-8 text-[13.5px] text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
              />
            </div>
            <button
              type="button"
              disabled={bulkOff || filtered.length === 0}
              onClick={selectMatches}
              className="h-9 shrink-0 rounded-[10px] border border-[var(--brand-line)] px-2.5 text-[12.5px] text-brand-strong transition-colors hover:bg-[var(--brand-bg)] disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
            >
              {q
                ? t("appsSelectMatches").replace("{m}", String(filtered.length))
                : t("appsSelectAll").replace("{t}", String(list.length))}
            </button>
            <button
              type="button"
              disabled={bulkOff || apps.length === 0}
              onClick={() => setApps([])}
              className="h-9 shrink-0 rounded-[10px] border border-line px-2.5 text-[12.5px] text-[rgb(207_112_120/0.8)] transition-colors hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)] disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-line disabled:hover:bg-transparent disabled:hover:text-[rgb(207_112_120/0.8)]"
            >
              {t("appsClearSel")}
            </button>
          </div>
        </div>
      </div>

      <div className="overflow-hidden rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
        {/* one count only: the list header repeats it, so the right hand
            figure was saying the same thing twice */}
        <div className="flex flex-wrap items-start justify-between gap-3 px-4 pb-3 pt-3.5">
          <div className="min-w-0">
            <h2 className="text-[15px] font-semibold text-txt">{t("appsListTitle")}</h2>
            <p aria-live="polite" className="mt-0.5 text-[12.5px] text-txt3" dir="auto">
              {summary}
            </p>
          </div>
        </div>

        <div
          className="flex flex-col gap-2 px-3 pb-3"
          role="listbox"
          aria-multiselectable="true"
          onFocus={() => setListActive(true)}
          onBlur={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) setListActive(false)
          }}
        >
          {filtered.length === 0 ? (
            <p className="px-1 py-6 text-[12.5px] text-txt3">{loading ? t("appsLoading") : t("appsEmpty")}</p>
          ) : (
            filtered.map((row, i) => {
              const on = apps.includes(row.pkg)
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
                    "flex w-full scroll-mt-36 items-center gap-3 rounded-[12px] border border-line bg-[rgb(21_29_46/0.62)] px-3.5 py-2.5 text-start transition-colors hover:border-line-strong focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-[var(--brand-line)]",
                    appsMode === "all" && "opacity-70",
                    on ? "border-[var(--brand-line)] bg-[var(--brand-bg)]" : focused && "bg-white/[0.04]",
                  )}
                >
                  <span
                    aria-hidden
                    className="grid size-8 shrink-0 place-items-center overflow-hidden rounded-[8px] border border-line bg-white/[0.03]"
                  >
                    {icon ? (
                      <img src={icon} alt="" draggable={false} className="size-5 select-none" />
                    ) : (
                      <span className="text-[12.5px] font-semibold uppercase text-txt3">
                        {row.label.slice(0, 1)}
                      </span>
                    )}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className={cn("block truncate text-[13.5px]", on ? "text-txt" : "text-txt2")} dir="auto">
                      {row.label}
                    </span>
                    <span className="block truncate text-[11px] text-txt3" dir="auto">
                      {row.pkg}
                    </span>
                  </span>
                  <span
                    aria-hidden
                    className={cn(
                      "grid size-4 shrink-0 place-items-center rounded-[5px] border transition-colors",
                      on ? "border-[var(--brand-line)] bg-[var(--brand-bg)]" : "border-line-strong",
                    )}
                  >
                    {on && <Check className="size-3 text-brand-strong" strokeWidth={3} aria-hidden />}
                  </span>
                </button>
              )
            })
          )}
        </div>
      </div>
    </div>
  )
}
