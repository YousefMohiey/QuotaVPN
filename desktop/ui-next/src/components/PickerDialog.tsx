import { useEffect, useMemo, useState, type ReactNode } from "react"
import { Check, Search } from "@/components/icons"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"
import { pushCloser } from "@/lib/navStack"

export type PickerItem = { value: string; label: string; sub?: string }

/**
 * The list picker from the 0.2.x sheets: search on top, plain rows, a check on
 * the one in use. Used for the domain choice and for the ping target, so both
 * read exactly like the picker that was already there.
 */
export function PickerDialog({
  open,
  onOpenChange,
  title,
  search,
  items,
  value,
  onPick,
  children,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void

  title: string
  search: string
  items: PickerItem[]
  value: string
  onPick: (v: string) => void
  children?: ReactNode
}) {
  // Back closes this before it touches the page under it.
  useEffect(() => {
    if (!open) return
    return pushCloser(() => onOpenChange(false))
  }, [open, onOpenChange])

  const [q, setQ] = useState("")

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase()
    if (!s) return items
    return items.filter((i) => `${i.label} ${i.sub ?? ""}`.toLowerCase().includes(s))
  }, [items, q])

  const close = (o: boolean) => {
    onOpenChange(o)
    if (!o) setQ("")
  }

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="max-w-[380px] gap-0 overflow-hidden rounded-[var(--r-card)] border-line bg-[var(--popover)] p-0">
        <div className="border-b border-line px-4 pb-4 pt-4">
          <DialogTitle className="pe-7 text-sm font-semibold text-txt">{title}</DialogTitle>
          <div className="relative mt-3">
            <Search className="absolute start-3 top-1/2 size-3.5 -translate-y-1/2 text-txt3" aria-hidden />
            <Input
              autoFocus
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder={search}
              aria-label={search}
              className="h-10 rounded-xl border-line bg-[var(--surface-2)] ps-9 text-sm focus-visible:border-[var(--brand-line)] focus-visible:ring-[3px] focus-visible:ring-[var(--brand-line)]"
            />
          </div>
        </div>

        <div className="qc-scroll max-h-[360px] overflow-y-auto p-2">
          {filtered.map((it) => {
            const on = it.value === value
            return (
              <button
                key={it.value}
                type="button"
                role="option"
                aria-selected={on}
                onClick={() => {
                  onPick(it.value)
                  close(false)
                }}
                className={cn(
                  "flex w-full items-center justify-between gap-3 rounded-xl px-3 py-3 text-start transition-colors hover:bg-white/[0.04]",
                  on && "bg-[var(--brand-bg)]",
                )}
              >
                <span className="min-w-0">
                  <span className="block truncate text-sm text-txt" dir="auto">
                    {it.label}
                  </span>
                  {it.sub && (
                    <span className="mt-0.5 block truncate text-xs text-txt3" dir="auto">
                      {it.sub}
                    </span>
                  )}
                </span>
                {on && (
                  <span
                    aria-hidden
                    className="grid size-5 shrink-0 place-items-center rounded-full border border-[var(--brand-line)] bg-[var(--brand-bg)]"
                  >
                    <Check className="size-3 text-brand-strong" />
                  </span>
                )}
              </button>
            )
          })}
          {filtered.length === 0 && <p className="px-3 py-6 text-center text-xs text-txt3">-</p>}
        </div>

        {children}
      </DialogContent>
    </Dialog>
  )
}
