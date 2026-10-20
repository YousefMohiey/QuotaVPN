import { useMemo, useState } from "react"
import { Activity, ArrowDown, ArrowUp, Check, ChevronRight, History as HistoryIcon, Trash2 } from "lucide-react"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"
import { loadHistory, saveHistory, type Run } from "./Speed"

type Pending = { kind: "one"; at: number } | { kind: "many" } | { kind: "all" } | null

const dayKey = (at: number) => {
  const d = new Date(at)
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
}

/** A run as one row of measurements, drawn with the same divided strip the
    speed page uses, so a glance reads it the same way. */
function RunCard({
  h,
  select,
  picked,
  onAct,
  onDelete,
  deleteLabel,
  serverLabel,
}: {
  h: Run
  select: boolean
  picked: boolean
  onAct: () => void
  onDelete?: () => void
  deleteLabel: string
  serverLabel: string
}) {
  const { t } = useI18n()
  const num = (v: number | null, digits = 1) => (v === null ? "-" : v >= 100 ? v.toFixed(0) : v.toFixed(digits))
  const time = new Date(h.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
  const cells: Array<{ icon: typeof ArrowDown | null; label: string; value: string; unit: string }> = [
    { icon: ArrowDown, label: t("chDown"), value: num(h.down), unit: t("mbps") },
    { icon: ArrowUp, label: t("chUp"), value: num(h.up), unit: t("mbps") },
    { icon: null, label: t("pingTitle"), value: num(h.ping, 0), unit: t("ms") },
    { icon: null, label: t("jitter"), value: num(h.jitter, 0), unit: t("ms") },
  ]
  return (
    <div
      className={cn(
        "group relative rounded-[14px] border border-line bg-[var(--panel)] transition-colors hover:border-line-strong",
        picked && "border-[var(--brand-line)] bg-[var(--brand-bg)]",
      )}
    >
      <button
        type="button"
        onClick={onAct}
        aria-pressed={select ? picked : undefined}
        className="block w-full px-4 pt-3 pb-0 text-start"
      >
        <div className="flex items-center gap-3 pe-16">
          <span className="text-[11px] tabular-nums text-txt3">{time}</span>
          <span className="min-w-0 truncate text-[11.5px] text-txt2" dir="auto">
            {h.target || serverLabel}
          </span>
        </div>
        <div className="-mx-4 mt-2.5 grid grid-cols-4 divide-x divide-line border-t border-line">
          {cells.map((c, i) => (
            <span key={c.label + i} className="min-w-0 px-3.5 py-2.5">
              <span className="flex items-center gap-1.5 text-[11px] text-txt3">
                {c.icon && <c.icon className="size-3 shrink-0" strokeWidth={1.7} aria-hidden />}
                <span className="truncate">{c.label}</span>
              </span>
              <span className="mt-1 flex items-baseline gap-1">
                <span className={cn("text-[16px] font-semibold leading-none tabular-nums", c.value === "-" ? "text-txt3" : "text-txt")}>
                  {c.value}
                </span>
                <span className="text-[11px] text-txt3">{c.unit}</span>
              </span>
            </span>
          ))}
        </div>
      </button>
      <div className="absolute top-2.5 right-3 flex items-center gap-1">
        {select ? (
          <span
            className={cn(
              "grid size-4 place-items-center rounded-[5px] border transition-colors",
              picked ? "border-[var(--brand-line)] bg-[var(--brand-bg)]" : "border-line-strong",
            )}
            aria-hidden
          >
            {picked && <Check className="size-3 text-brand-strong" />}
          </span>
        ) : (
          <>
            <button
              type="button"
              aria-label={deleteLabel}
              title={deleteLabel}
              onClick={onDelete}
              className="grid size-7 place-items-center rounded-[8px] text-txt3 opacity-0 transition-opacity duration-150 hover:bg-[var(--red-bg)] hover:text-[var(--red)] focus-visible:opacity-100 group-hover:opacity-100"
            >
              <Trash2 className="size-3.5" aria-hidden />
            </button>
            <ChevronRight
              className="size-4 shrink-0 text-txt3 transition-[color,transform] duration-200 group-hover:translate-x-0.5 group-hover:text-brand-strong"
              aria-hidden
            />
          </>
        )}
      </div>
    </div>
  )
}

/** The log as days of runs, each run a strip of its numbers, with a small
    summary of the bests on top. Selection turns the rows into checkboxes so
    one, several or everything can go, and nothing is removed without a
    confirmation. */
export function SpeedHistory({ onOpenResult }: { onOpenResult: (at: number) => void }) {
  const { t } = useI18n()
  const [history, setHistory] = useState<Run[]>(() => loadHistory())
  const [selMode, setSelMode] = useState(false)
  const [picked, setPicked] = useState<number[]>([])
  const [pending, setPending] = useState<Pending>(null)

  const write = (next: Run[]) => {
    setHistory(next)
    saveHistory(next)
  }

  const toggle = (at: number) => setPicked((p) => (p.includes(at) ? p.filter((x) => x !== at) : [...p, at]))

  const confirm = () => {
    if (!pending) return
    if (pending.kind === "one") write(history.filter((h) => h.at !== pending.at))
    else if (pending.kind === "many") write(history.filter((h) => !picked.includes(h.at)))
    else write([])
    setPicked([])
    setSelMode(false)
    setPending(null)
  }

  const title =
    pending?.kind === "one" ? t("delOne") : pending?.kind === "many" ? t("delMany") : t("clearTitle")
  const body = pending?.kind === "all" ? t("clearBody") : t("delBody")

  const rowsToGo = useMemo(() => {
    if (pending?.kind === "one") return 1
    if (pending?.kind === "many") return picked.length
    return history.length
  }, [pending, picked.length, history.length])

  /* newest first, cut into days */
  const days = useMemo(() => {
    const out: Array<{ key: string; at: number; runs: Run[] }> = []
    for (const h of history) {
      const key = dayKey(h.at)
      const last = out[out.length - 1]
      if (last && last.key === key) last.runs.push(h)
      else out.push({ key, at: h.at, runs: [h] })
    }
    return out
  }, [history])

  /* what the whole log says: the biggest numbers it ever produced */
  const best = useMemo(() => {
    const nums = (pick: (h: Run) => number | null) =>
      history.map(pick).filter((v): v is number => v !== null && Number.isFinite(v))
    const down = nums((h) => h.down)
    const up = nums((h) => h.up)
    const ping = nums((h) => h.ping)
    return {
      down: down.length ? Math.max(...down) : null,
      up: up.length ? Math.max(...up) : null,
      ping: ping.length ? Math.min(...ping) : null,
    }
  }, [history])

  const dayLabel = (at: number) => {
    const today = Date.now()
    if (dayKey(at) === dayKey(today)) return t("today")
    if (dayKey(at) === dayKey(today - 86400000)) return t("yesterday")
    return new Date(at).toLocaleDateString([], { month: "short", day: "numeric" })
  }

  const num = (v: number | null) => (v === null ? "-" : v >= 100 ? v.toFixed(0) : v.toFixed(1))

  const bestCells: Array<{ icon: typeof ArrowDown; label: string; value: number | null; unit: string; round?: boolean }> = [
    { icon: ArrowDown, label: t("bestDown"), value: best.down, unit: t("mbps") },
    { icon: ArrowUp, label: t("bestUp"), value: best.up, unit: t("mbps") },
    { icon: Activity, label: t("bestPing"), value: best.ping, unit: t("ms"), round: true },
  ]

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-3">
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        {/* the log: one column, newest first, cut into days */}
        <section className="flex min-h-0 flex-1 flex-col">
          <div className="flex shrink-0 items-center gap-2.5 border-b border-line px-5 py-3.5">
            <HistoryIcon className="size-4 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
            <span className="min-w-0 flex-1 truncate text-[11px] font-medium uppercase tracking-[0.1em] text-txt2">
              {t("history")}
            </span>
            {history.length > 0 ? (
              <span className="shrink-0 text-[11.5px] tabular-nums text-txt3">{history.length}</span>
            ) : null}
            {history.length > 0 ? (
              <div className="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setSelMode((v) => !v)
                    setPicked([])
                  }}
                  className="press flex h-7 items-center rounded-full px-2.5 text-[12px] text-txt2 transition-colors hover:bg-white/[0.05] hover:text-txt"
                >
                  {selMode ? t("done") : t("select")}
                </button>
                {!selMode ? (
                  <button
                    type="button"
                    onClick={() => setPending({ kind: "all" })}
                    aria-label={t("clearAll")}
                    title={t("clearAll")}
                    className="press grid size-7 place-items-center rounded-full text-txt3 transition-colors hover:bg-[var(--red-bg)] hover:text-[var(--red)]"
                  >
                    <Trash2 className="size-3.5" aria-hidden />
                  </button>
                ) : null}
              </div>
            ) : null}
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {history.length === 0 ? (
              <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
                <span className="grid size-12 place-items-center rounded-full bg-white/[0.04] text-txt3">
                  <HistoryIcon className="size-5" strokeWidth={1.7} aria-hidden />
                </span>
                <p className="max-w-[300px] text-[12.5px] leading-relaxed text-txt2">{t("histEmpty")}</p>
              </div>
            ) : (
              <div className="flex flex-col gap-4">
                {days.map((d) => (
                  <div key={d.key}>
                    <div className="flex items-center gap-3">
                      <span className="text-[11px] font-medium uppercase tracking-[0.09em] text-txt3">{dayLabel(d.at)}</span>
                      <span className="h-px flex-1 bg-[var(--line)]" aria-hidden />
                    </div>
                    <div className="mt-2.5 flex flex-col gap-2">
                      {d.runs.map((h) => {
                        const on = picked.includes(h.at)
                        return (
                          <RunCard
                            key={h.at}
                            h={h}
                            select={selMode}
                            picked={on}
                            onAct={() => (selMode ? toggle(h.at) : onOpenResult(h.at))}
                            onDelete={() => setPending({ kind: "one", at: h.at })}
                            deleteLabel={t("delete")}
                            serverLabel={t("targetServer")}
                          />
                        )
                      })}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {selMode ? (
            <div className="flex shrink-0 items-center justify-between gap-3 border-t border-line bg-[var(--side)] px-4 py-2.5">
              <span className="text-[12.5px] text-txt3">
                {t("selected")}: <span className="tabular-nums text-txt2">{picked.length}</span>
              </span>
              <button
                type="button"
                disabled={picked.length === 0}
                onClick={() => setPending({ kind: "many" })}
                className="press flex h-8 items-center gap-1.5 rounded-full bg-[var(--brand)] px-3.5 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
              >
                <Trash2 className="size-3.5" aria-hidden />
                {t("delete")}
              </button>
            </div>
          ) : null}
        </section>

        </div>

      <section className="grid shrink-0 grid-cols-[0.9fr_repeat(3,1.1fr)] overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <div className="flex min-w-0 flex-col justify-center gap-1.5 border-e border-line px-5 py-4">
          <span className="text-[11px] font-medium uppercase tracking-[0.1em] text-txt2">{t("allTime")}</span>
          <span className="truncate text-[12px] text-txt3">
            {history.length} · {t("history")}
          </span>
        </div>
        {bestCells.map((c, i) => (
          <div
            key={c.label}
            className={cn("flex min-w-0 flex-col justify-center gap-2 px-5 py-4", i < 2 && "border-e border-line")}
          >
            <span className="flex min-w-0 items-center gap-1.5 text-[11px] font-medium uppercase tracking-[0.1em] text-txt2">
              <c.icon className="size-3.5 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
              <span className="truncate">{c.label}</span>
            </span>
            <span className="flex min-w-0 items-baseline gap-1.5">
              <span className={cn("font-display text-[24px] font-semibold leading-none tabular-nums", c.value === null ? "text-txt3" : "text-txt")}>
                {c.round ? (c.value === null ? "-" : Math.round(c.value)) : num(c.value)}
              </span>
              <span className="text-[11.5px] text-txt3">{c.unit}</span>
            </span>
          </div>
        ))}
      </section>

      <Dialog open={pending !== null} onOpenChange={(o) => !o && setPending(null)}>
        <DialogContent className="max-w-[360px] gap-0 overflow-hidden rounded-[var(--r-card)] p-0">
          <div className="px-5 pb-4 pt-4">
            <DialogHeader className="gap-1.5">
              <DialogTitle className="text-[14px] font-semibold">{title}</DialogTitle>
              <DialogDescription className="text-[12.5px] leading-relaxed text-txt3">{body}</DialogDescription>
            </DialogHeader>
            <p className="mt-3 text-[12.5px] text-txt3">
              {t("history")}: <span className="tabular-nums text-txt2">{rowsToGo}</span>
            </p>
          </div>
          <div className="flex items-center justify-end gap-2 border-t border-line px-5 py-3">
            <button
              type="button"
              className="flex h-9 items-center rounded-[10px] border border-line px-4 text-[13px] text-txt transition-colors hover:border-line-strong"
              onClick={() => setPending(null)}
            >
              {t("cancel")}
            </button>
            <button
              type="button"
              className="flex h-9 items-center gap-1.5 rounded-[10px] border border-[var(--red-line)] bg-[rgb(207_112_120/0.2)] px-4 text-[13px] font-medium text-[#e6999f] transition-colors hover:bg-[rgb(207_112_120/0.26)]"
              onClick={confirm}
            >
              <Trash2 className="size-3.5" aria-hidden />
              {t("delete")}
            </button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}
