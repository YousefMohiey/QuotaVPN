import { useState, type ReactNode } from "react"
import { ChevronDown, ChevronRight, Gamepad2, Tv } from "lucide-react"
import { Hero, FactsCard } from "@/components/Hero"
import { WeQuotaCard } from "@/components/WeQuotaCard"
import { PickerDialog, type PickerItem } from "@/components/PickerDialog"
import { Segmented } from "@/components/Segmented"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useApp, type PresetKind } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { CUSTOM_SNI, DEFAULT_SNI, SNIS, labelForSni } from "@/lib/snis"
import { cn } from "@/lib/utils"

// The Home screen is two columns: the left stack acts (connect, then the
// controls, then the facts) and the right rail reads (the WE package, then
// the profiles). Nothing travels between states.
export function Home({ onOpenApps }: { onOpenApps: () => void }) {
  const { t } = useI18n()
  const { cards, card, pickCard, preset, setPreset, ensurePresetCard, applyDomain, transport, setTransport, appsMode, apps } = useApp()
  // Domain control: the everyday choice lives here, not in the card list.
  const [domainOpen, setDomainOpen] = useState(false)
  const [customOpen, setCustomOpen] = useState(false)
  const [customVal, setCustomVal] = useState("")
  const [domainBusy, setDomainBusy] = useState(false)

  const activeKind: PresetKind =
    card?.card_type === "Streamerz" ? "Streamerz" : card?.card_type === "Gamerz" ? "Gamerz" : preset
  const currentSni = card?.sni || DEFAULT_SNI[activeKind]
  const domLabel = labelForSni(currentSni)
  const domText = domLabel === currentSni ? currentSni : `${domLabel} · ${currentSni}`
  const domainItems: PickerItem[] = [
    ...SNIS[activeKind].map(([label, domain]) => ({ value: domain, label, sub: domain })),
    { value: CUSTOM_SNI, label: t("customDomainOpt") },
  ]

  const submitCustom = async () => {
    const v = customVal.trim()
    if (!v) return
    setDomainBusy(true)
    const r = await applyDomain(v)
    setDomainBusy(false)
    if (r.ok) {
      setCustomOpen(false)
      setCustomVal("")
    }
  }
  // In-flight preset creation: the preset flips instantly, the tap below
  // creates and selects the missing card without any connection.
  const [creating, setCreating] = useState<PresetKind | null>(null)

  const routing =
    appsMode === "all"
      ? `${t("vpnFor")} ${t("wholeDevice")}`
      : appsMode === "allow"
        ? t("appsOnly")
        : t("appsExcept")
  const routingCount = appsMode !== "all" && apps.length ? ` · ${apps.length}` : ""
  const trHint = transport === "hy2" ? t("trNoteHy2") : transport === "wg" ? t("trNoteWg") : ""

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col">
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,7fr)_minmax(0,5fr)] gap-3">
        {/* left: the action stack */}
        <div className="flex min-h-0 flex-col gap-3">
          <Hero />

          <section className="flex flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] p-5">
            <div className="flex flex-col divide-y divide-line">
              <RowField label={t("domainSni")}>
                {customOpen ? (
                  <div className="flex w-full items-center gap-2">
                    <Input
                      autoFocus
                      value={customVal}
                      onChange={(e) => setCustomVal(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void submitCustom()
                        if (e.key === "Escape") setCustomOpen(false)
                      }}
                      placeholder="example.com"
                      aria-label={t("domainSni")}
                      className="h-10 rounded-[10px] border-line bg-white/[0.02] text-[13px]"
                    />
                    <Button
                      size="sm"
                      className="h-10 shrink-0 rounded-[10px] px-3.5 text-[13px]"
                      disabled={domainBusy || !customVal.trim()}
                      onClick={() => void submitCustom()}
                    >
                      {t("apply")}
                    </Button>
                  </div>
                ) : (
                  <button
                    type="button"
                    aria-busy={domainBusy}
                    onClick={() => setDomainOpen(true)}
                    className="group flex h-10 w-full items-center justify-between gap-3 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"
                  >
                    <span className={cn("truncate", domText.length > 17 && "text-[11px]")} dir="auto">{domText}</span>
                    <ChevronDown className="size-4 shrink-0 text-txt2 transition-[color,transform] duration-200 group-hover:translate-y-px group-hover:text-brand-strong" aria-hidden />
                  </button>
                )}
              </RowField>

              <RowField label={t("routing")}>
                <button
                  type="button"
                  onClick={onOpenApps}
                  className="group flex h-10 w-full items-center justify-between gap-3 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"
                >
                  <span className="truncate" dir="auto">
                    {routing}
                    {routingCount}
                  </span>
                  <ChevronRight
                    className="size-4 shrink-0 text-txt2 transition-transform duration-200 group-hover:translate-x-0.5 group-hover:text-brand-strong"
                    aria-hidden
                  />
                </button>
              </RowField>

              <RowField label={t("transport")} hint={trHint}>
                <Segmented
                  id="transport"
                  value={transport}
                  onChange={setTransport}
                  boxed
                  options={[
                    { value: "vless", label: t("trStandard") },
                    { value: "hy2", label: "Hysteria2", hint: t("trNoteHy2") },
                    { value: "wg", label: t("trWg"), hint: t("trNoteWg") },
                  ]}
                />
              </RowField>
            </div>
          </section>
        </div>

        {/* right: the reading rail */}
        <div className="flex min-h-0 flex-col gap-3">
          <WeQuotaCard />

          <section className="flex flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] p-5">
            <div className="flex flex-col gap-3" role="group" aria-label={t("setupTitle")}>
              {(["Gamerz", "Streamerz"] as const).map((kind: PresetKind) => {
                const mine = cards.find((c) => c.card_type === kind)
                const sni = mine?.sni ?? DEFAULT_SNI[kind]
                const isActive = preset === kind
                const Icon = kind === "Gamerz" ? Gamepad2 : Tv
                return (
                  <button
                    key={kind}
                    type="button"
                    aria-pressed={isActive}
                    disabled={creating !== null}
                    onClick={() => {
                      if (creating) return
                      if (mine) {
                        setPreset(kind)
                        pickCard(mine.uuid)
                        return
                      }
                      setCreating(kind)
                      void ensurePresetCard(kind).finally(() => setCreating(null))
                    }}
                    className={cn(
                      "flex items-center gap-3 rounded-[12px] border p-4 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",
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
                      <Icon className="size-[18px]" aria-hidden />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span
                        className={cn("block truncate text-[14px] font-semibold", isActive ? "text-txt" : "text-txt2")}
                        dir="auto"
                      >
                        {t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}
                      </span>
                      <span className="mt-0.5 block truncate font-mono text-[11px] text-txt3">{sni}</span>
                    </span>
                    <span
                      aria-hidden
                      className={cn(
                        "grid size-4 shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
                        isActive ? "border-[var(--brand)]" : "border-line-strong",
                      )}
                    >
                      {isActive && <span className="size-[7px] rounded-full bg-[var(--brand)]" />}
                    </span>
                  </button>
                )
              })}
            </div>
          </section>

          <FactsCard />
        </div>
      </div>

      <PickerDialog
        open={domainOpen}
        onOpenChange={setDomainOpen}
        title={t("domainSni")}
        search={t("sheetSearch")}
        items={domainItems}
        value={currentSni}
        onPick={(v) => {
          if (v === CUSTOM_SNI) {
            setCustomOpen(true)
            setCustomVal("")
            return
          }
          void applyDomain(v)
        }}
      />
    </div>
  )
}

/** One config row: the label sits left, the control takes the right. */
function RowField({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: ReactNode
}) {
  return (
    <div className="flex items-center gap-3 py-3">
      <span className="w-28 shrink-0 truncate text-[12px] text-txt2">{label}</span>
      <div className="min-w-0 flex-1">
        {children}
        {hint ? <p className="mt-1 text-[11px] text-txt3">{hint}</p> : null}
      </div>
    </div>
  )
}
