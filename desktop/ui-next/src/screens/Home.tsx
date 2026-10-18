import { useState, type ReactNode } from "react"
import { ChevronDown, ChevronRight, Gamepad2, Tv } from "lucide-react"
import { Hero, useGeo, useServerAddr } from "@/components/Hero"
import { WeQuotaCard } from "@/components/WeQuotaCard"
import { PickerDialog, type PickerItem } from "@/components/PickerDialog"
import { Segmented } from "@/components/Segmented"
import { Tip } from "@/components/Tip"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { copyText } from "@/lib/clipboard"
import { useApp, type PresetKind } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { CUSTOM_SNI, DEFAULT_SNI, SNIS, labelForSni } from "@/lib/snis"
import { cn } from "@/lib/utils"

// Home, the instrument: one slab for the connection and the line, one strip
// for the three settings that shape it, one strip for the readings. Structure
// comes from hairlines, not from stacked cards.
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
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1120px] flex-col gap-3">
      {/* the slab: the dial and its reading on the left, the line and the
          two profiles on the right, one hairline between them */}
      <section className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.32fr)_minmax(0,1fr)] overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <div className="flex min-h-0 flex-col justify-center p-7">
          <Hero />
        </div>

        <aside className="flex min-h-0 flex-col border-s border-line">
          <WeQuotaCard />

          <div className="border-t border-line p-3.5" role="group" aria-label={t("setupTitle")}>
            <div className="grid grid-cols-2 gap-2">
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
                      "flex min-w-0 items-center gap-2 rounded-[8px] border px-2.5 py-2 text-start transition-colors duration-150 disabled:cursor-wait disabled:opacity-70",
                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-txt"
                        : "border-line text-txt2 hover:border-line-strong hover:text-txt",
                    )}
                  >
                    <Icon className={cn("size-4 shrink-0", isActive ? "text-brand-strong" : "text-txt3")} aria-hidden />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[12px] font-medium" dir="auto">
                        {t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}
                      </span>
                      <span className="mt-px block truncate font-mono text-[9.5px] text-txt3" dir="auto">
                        {sni}
                      </span>
                    </span>
                    <span
                      aria-hidden
                      className={cn(
                        "size-1.5 shrink-0 rounded-full",
                        isActive ? "bg-[var(--brand-vivid)]" : "bg-transparent",
                      )}
                    />
                  </button>
                )
              })}
            </div>
          </div>
        </aside>
      </section>

      {/* the settings strip: three controls, one line each */}
      <section className="grid grid-cols-[1fr_1.12fr_1.34fr] divide-x divide-line overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
        <Field label={t("domainSni")}>
          {customOpen ? (
            <div className="flex w-full min-w-0 items-center gap-1.5">
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
                className="h-8 min-w-0 rounded-[7px] border-line bg-white/[0.02] px-2 text-[12px]"
              />
              <Button
                size="sm"
                className="h-8 shrink-0 rounded-[7px] px-2.5 text-[12px]"
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
              className="group flex h-8 w-full min-w-0 items-center justify-between gap-2 rounded-[7px] border border-line bg-white/[0.02] px-2.5 text-[12px] text-txt transition-colors duration-150 hover:border-[var(--brand-line)]"
            >
              <span className="truncate" dir="auto">
                {domText}
              </span>
              <ChevronDown className="size-3.5 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
            </button>
          )}
        </Field>

        <Field label={t("routing")}>
          <button
            type="button"
            onClick={onOpenApps}
            className="group flex h-8 w-full min-w-0 items-center justify-between gap-2 rounded-[7px] border border-line bg-white/[0.02] px-2.5 text-[12px] text-txt transition-colors duration-150 hover:border-[var(--brand-line)]"
          >
            <span className="truncate" dir="auto">
              {routing}
              {routingCount}
            </span>
            <ChevronRight className="size-3.5 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
          </button>
        </Field>

        <Field label={t("transport")}>
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
        </Field>
      </section>

      {/* the readings strip: four facts, one line each */}
      <FactsStrip />

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

/** One cell of the settings strip: a label above, the control under it. */
function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5 px-4 py-2.5">
      <span className="truncate text-[10.5px] font-medium uppercase tracking-[0.09em] text-txt2">{label}</span>
      <div className="min-w-0">{children}</div>
    </div>
  )
}

/** The readings: where this machine sits and appears from right now. */
function FactsStrip() {
  const { t } = useI18n()
  const { connected, serverIp } = useApp()
  const geo = useGeo(connected)
  const serverAddr = useServerAddr(connected, serverIp)
  const ipText = connected ? serverAddr || geo?.ip || "-" : geo?.ip || "-"
  const [copied, setCopied] = useState(false)

  const take = async () => {
    if (!ipText || ipText === "-") return
    if (!(await copyText(ipText))) return
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1400)
  }

  return (
    <section className="grid grid-cols-3 divide-x divide-line overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
      <Fact label={t("yourIp")}>
        <Tip text={t("copy")}>
          <button
            type="button"
            onClick={() => void take()}
            aria-label={`${t("copy")} ${ipText}`}
            className={cn(
              "min-w-0 cursor-pointer truncate rounded-[4px] text-[12.5px] tabular-nums transition-colors hover:text-brand-strong",
              copied ? "text-[var(--green)]" : "text-txt",
            )}
            dir="auto"
          >
            {copied ? t("copied") : ipText}
          </button>
        </Tip>
      </Fact>
      <Fact label={t("yourLocation")} value={geo?.place || "-"} />
      <Fact label={t("ispLbl")} value={geo?.isp || "-"} />
    </section>
  )
}

function Fact({ label, value, dot, children }: { label: string; value?: string; dot?: "on" | "off"; children?: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 px-4 py-2.5">
      <span className="truncate text-[10.5px] font-medium uppercase tracking-[0.09em] text-txt2">{label}</span>
      <span className="flex min-w-0 items-center gap-1.5">
        {dot ? (
          <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", dot === "on" ? "bg-[var(--green)]" : "bg-txt3")} />
        ) : null}
        {children ?? (
          <span className="truncate text-[12.5px] text-txt" dir="auto">
            {value}
          </span>
        )}
      </span>
    </div>
  )
}
