import { useState, type ReactNode } from "react"
import { Cable, ChevronDown, ChevronRight, Gamepad2, LayoutGrid, Server as ServerIcon, Tv, type LucideIcon } from "lucide-react"
import { Hero } from "@/components/Hero"
import { PickerDialog, type PickerItem } from "@/components/PickerDialog"
import { Segmented } from "@/components/Segmented"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useApp, type PresetKind } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { CUSTOM_SNI, DEFAULT_SNI, SNIS, labelForSni } from "@/lib/snis"
import { cn } from "@/lib/utils"

// The Home screen is two calm cards: the connection (dial, reading, facts)
// and the setup (story on the left, controls on the right behind the same
// hairline divider the Valorant cards use). Everything else stays out.
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

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">
      <Hero />

      <section className="flex min-h-[186px] flex-1 flex-col justify-center rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
        <div>
          <h2 className="text-[16.5px] font-semibold text-txt">{t("setupTitle")}</h2>
          <p className="mt-1 max-w-[430px] text-[13.5px] leading-relaxed text-txt2">{t("setupBody")}</p>
        </div>
        <div className="mt-4 flex flex-col gap-5 lg:flex-row lg:items-stretch">
          {/* the presets: two stacked rows, each carrying its own story */}
          <div className="flex min-w-0 flex-1 flex-col gap-2.5">
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
                      "grid size-11 shrink-0 place-items-center rounded-[10px] border transition-colors",
                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                        : "border-line bg-white/[0.03] text-txt3",
                    )}
                  >
                    <Icon className="size-[19px]" aria-hidden />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span
                      className={cn("block truncate text-[14.5px] font-semibold", isActive ? "text-txt" : "text-txt2")}
                      dir="auto"
                    >
                      {t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}
                    </span>
                    <span className="mt-1 block truncate font-mono text-[11.5px] text-txt3">{sni}</span>
                  </span>
                  <span
                    aria-hidden
                    className={cn(
                      "grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
                      isActive ? "border-[var(--brand)]" : "border-line-strong",
                    )}
                  >
                    {isActive && <span className="size-2 rounded-full bg-[var(--brand)]" />}
                  </span>
                </button>
              )
            })}
          </div>

          {/* the controls, one continuous column */}
          <div className="flex shrink-0 flex-col justify-center gap-3 lg:w-[364px] lg:border-l lg:border-line lg:pl-6">
            <div>
              <h3 className="text-[13.5px] font-semibold text-txt">{t("configHeading")}</h3>
              <p className="mt-0.5 text-[12px] text-txt3">{t("configBody")}</p>
            </div>
            <ControlRow label={t("domainSni")} icon={ServerIcon}>
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
            </ControlRow>

            <ControlRow label={t("routing")} icon={LayoutGrid}>
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
            </ControlRow>

            <ControlRow label={t("transport")} align="start" icon={Cable}>
              <div>
                <Segmented
                  id="transport"
                  value={transport}
                  onChange={setTransport}
                  fill
                  options={[
                    { value: "vless", label: t("trStandard") },
                    { value: "hy2", label: "Hysteria2" },
                    { value: "wg", label: t("trWg") },
                  ]}
                />
                {transport !== "vless" && (
                  <p className="mt-2.5 text-[11.5px] text-txt3">{transport === "wg" ? t("trNoteWg") : t("trNoteHy2")}</p>
                )}
              </div>
            </ControlRow>
          </div>
        </div>
      </section>

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

/** One control row: a quiet label with its glyph, then the control. */
function ControlRow({
  label,
  children,
  align = "center",
  icon: Icon,
}: {
  label: string
  children: ReactNode
  align?: "center" | "start"
  icon?: LucideIcon
}) {
  return (
    <div className={cn("flex gap-3", align === "center" ? "items-center" : "items-start")}>
      <div className="flex w-[138px] shrink-0 items-center gap-2 pt-[2px] text-[12px] text-txt2">
        {/* the icon slot is reserved even without an icon so every label
            starts on the same x, rows included */}
        <span
          className={cn(
            "grid size-[26px] shrink-0 place-items-center rounded-[8px]",
            Icon && "border border-line bg-white/[0.03] text-txt2",
          )}
        >
          {Icon ? <Icon className="size-[13px]" aria-hidden /> : null}
        </span>
        <span className="min-w-0 truncate">{label}</span>
      </div>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  )
}
