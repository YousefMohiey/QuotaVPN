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
        "group relative rounded-[14px] border border-line bg-[rgb(18_19_23/0.72)] transition-colors hover:border-line-strong",
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
              <span className="flex items-center gap-1.5 text-[10.5px] text-txt3">
                {c.icon && <c.icon className="size-3 shrink-0" strokeWidth={1.7} aria-hidden />}
                <span className="truncate">{c.label}</span>
              </span>
              <span className="mt-1 flex items-baseline gap-1">
                <span className={cn("text-[16px] font-semibold leading-none tabular-nums", c.value === "-" ? "text-txt3" : "text-txt")}>
                  {c.value}
                </span>
                <span className="text-[10px] text-txt3">{c.unit}</span>
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

  const chip =
    "flex h-9 items-center gap-1.5 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"

  const bestCells: Array<{ icon: typeof ArrowDown; label: string; value: number | null; unit: string; round?: boolean }> = [
    { icon: ArrowDown, label: t("bestDown"), value: best.down, unit: t("mbps") },
    { icon: ArrowUp, label: t("bestUp"), value: best.up, unit: t("mbps") },
    { icon: Activity, label: t("bestPing"), value: best.ping, unit: t("ms"), round: true },
  ]

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-4">
      <div className="flex items-end justify-between gap-6">
        <div className="min-w-0 flex-1">
          <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("history")}</h1>
          <p className="mt-1 text-[15px] text-txt2">{t("histTag")}</p>
        </div>
        {history.length > 0 && (
          <div className="flex shrink-0 items-center gap-2.5">
            <button
              type="button"
              className={chip}
              onClick={() => {
                setSelMode((v) => !v)
                setPicked([])
              }}
            >
              {selMode ? t("done") : t("select")}
            </button>
            {!selMode && (
              <button
                type="button"
                className={cn(chip, "text-[var(--red)] hover:border-[var(--red-line)] hover:bg-[var(--red-bg)]")}
                onClick={() => setPending({ kind: "all" })}
              >
                <Trash2 className="size-3.5" aria-hidden />
                {t("clearAll")}
              </button>
            )}
          </div>
        )}
      </div>

      {history.length === 0 ? (
        <section className="flex flex-1 flex-col items-center justify-center gap-3 rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-12 text-center">
          <span className="grid size-12 place-items-center rounded-full border border-line bg-white/[0.02] text-txt3">
            <HistoryIcon className="size-5" strokeWidth={1.7} aria-hidden />
          </span>
          <p className="text-[13.5px] text-txt2">{t("histEmpty")}</p>
        </section>
      ) : (
        <>
          {/* the log in three numbers, drawn like the speed page strip */}
          <section className="grid grid-cols-3 divide-x divide-line rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)]">
            {bestCells.map((s) => (
              <div key={s.label} className="flex min-w-0 flex-col gap-2 px-5 py-3.5">
                <span className="flex items-center gap-2 text-[11px] text-txt3">
                  <s.icon className="size-3.5 shrink-0" strokeWidth={1.7} aria-hidden />
                  <span className="truncate">{s.label}</span>
                </span>
                <span className="flex items-baseline gap-1.5">
                  <span
                    className={cn(
                      "text-[24px] leading-none font-semibold tabular-nums",
                      s.value === null ? "text-txt3" : "text-txt",
                    )}
                  >
                    {s.round ? (s.value === null ? "-" : Math.round(s.value)) : num(s.value)}
                  </span>
                  <span className="text-[11px] text-txt3">{s.unit}</span>
                </span>
              </div>
            ))}
          </section>

          {/* the log, day by day */}
          <div className="flex flex-col gap-4">
            {days.map((d) => (
              <div key={d.key}>
                <div className="flex items-center gap-3 px-0.5">
                  <span className="text-[10.5px] font-medium tracking-[0.08em] text-txt3 uppercase">
                    {dayLabel(d.at)}
                  </span>
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
        </>
      )}

      {/* selection bar: what will go, and the one button that does it */}
      {selMode && (
        <div className="sticky bottom-0 flex items-center justify-between gap-3 rounded-[12px] border border-line bg-[rgb(21_29_46/0.9)] px-4 py-2.5 backdrop-blur-xl">
          <span className="text-[12.5px] text-txt3">
            {t("selected")}: <span className="tabular-nums text-txt2">{picked.length}</span>
          </span>
          <button
            type="button"
            disabled={picked.length === 0}
            onClick={() => setPending({ kind: "many" })}
            className="flex h-9 items-center gap-1.5 rounded-[10px] bg-[var(--brand-vivid)] px-3.5 text-[13px] font-medium text-white transition-[filter] hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
          >
            <Trash2 className="size-3.5" aria-hidden />
            {t("delete")}
          </button>
        </div>
      )}

      <Dialog open={pending !== null} onOpenChange={(o) => !o && setPending(null)}>
        <DialogContent className="max-w-[360px] gap-0 overflow-hidden rounded-[16px] p-0">
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
