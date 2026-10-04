import { useEffect, useMemo, useRef, useState } from "react"
import { ArrowLeft, Check, Search } from "lucide-react"
import { Segmented } from "@/components/Segmented"
import { useApp, type AppsMode } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

type Row = { pkg: string; label: string }

// The App routing page in the app's card language: a real page header, one
// controls card (mode, search, bulk actions, live summary), then the list as
// rows on dividers with a checkbox per app. Picking any row while "All apps"
// is on flips the mode instead of ignoring the tap.
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
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-3">
      <div className="px-1">
        <button
          type="button"
          onClick={onBack}
          className="mb-1.5 inline-flex items-center gap-1.5 rounded-[10px] px-2 py-1 text-[12px] text-txt3 transition-colors hover:bg-white/[0.04] hover:text-txt2"
        >
          <ArrowLeft className="size-3.5" aria-hidden />
          {t("back")}
        </button>
        <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("routing")}</h1>
        <p className="mt-1 text-[15px] text-txt2">{t("appsHint")}</p>
      </div>

      {/* the controls stay up while the list scrolls beneath them */}
      <div className="sticky top-0 z-10 bg-[var(--bg)] pb-3">
        <div className="overflow-hidden rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
          <div className="flex flex-wrap items-center justify-between gap-3 p-3">
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
            <div className="flex flex-wrap items-center gap-2">
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
                  className="h-9 w-[220px] rounded-[10px] border border-line bg-white/[0.02] ps-8 text-[13px] text-txt outline-none transition-colors placeholder:text-txt3 focus:border-[var(--brand-line)]"
                />
              </div>
              <button
                type="button"
                disabled={bulkOff || filtered.length === 0}
                onClick={selectMatches}
                className="h-8 rounded-[10px] px-2.5 text-[12px] text-txt2 transition-colors hover:bg-white/[0.04] hover:text-txt disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-txt2"
              >
                {q
                  ? t("appsSelectMatches").replace("{m}", String(filtered.length))
                  : t("appsSelectAll").replace("{t}", String(list.length))}
              </button>
              <button
                type="button"
                disabled={bulkOff || apps.length === 0}
                onClick={() => setApps([])}
                className="h-8 rounded-[10px] px-2.5 text-[12px] text-txt2 transition-colors hover:bg-white/[0.04] hover:text-txt disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-txt2"
              >
                {t("appsClearSel")}
              </button>
            </div>
          </div>
          <p aria-live="polite" className="border-t border-line px-4 py-2 text-[12px] text-txt3" dir="auto">
            {summary}
          </p>
        </div>
      </div>

      <div className="overflow-hidden rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
        <div
          role="listbox"
          aria-multiselectable="true"
          onFocus={() => setListActive(true)}
          onBlur={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) setListActive(false)
          }}
        >
          {filtered.length === 0 ? (
            <p className="px-4 py-6 text-[12.5px] text-txt3">{loading ? t("appsLoading") : t("appsEmpty")}</p>
          ) : (
            filtered.map((row, i) => {
              const on = apps.includes(row.pkg)
              const focused = i === activeIdx && listActive
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
                    "flex w-full scroll-mt-36 items-center gap-3 border-b border-line px-4 py-2.5 text-start transition-colors last:border-b-0 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-[var(--brand-line)]",
                    appsMode === "all" && "opacity-70",
                    focused ? "bg-white/[0.04]" : "hover:bg-white/[0.02]",
                  )}
                >
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
                      "grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
                      on ? "border-[var(--brand)] bg-[var(--brand)]" : "border-line-strong",
                    )}
                  >
                    {on && <Check className="size-3 text-white" strokeWidth={3} aria-hidden />}
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
