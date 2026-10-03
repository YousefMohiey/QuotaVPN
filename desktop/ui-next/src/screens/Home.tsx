import { useEffect, useState } from "react"
import { ArrowUpDown, ChevronDown, ChevronRight, Gamepad2, Globe, MapPin, TriangleAlert, Tv, type LucideIcon } from "lucide-react"
import { Hero } from "@/components/Hero"
import { PickerDialog, type PickerItem } from "@/components/PickerDialog"
import { Panel, Row } from "@/components/Row"
import { Segmented } from "@/components/Segmented"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useApp, type PresetKind } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { netInfo } from "@/lib/ipc"
import { displayHost } from "@/lib/format"
import { CUSTOM_SNI, DEFAULT_SNI, SNIS, labelForSni } from "@/lib/snis"
import { cn } from "@/lib/utils"

export function Home({ onOpenApps }: { onOpenApps: () => void }) {
  const { t } = useI18n()
  const { cards, card, pickCard, preset, setPreset, ensurePresetCard, applyDomain, transport, setTransport, appsMode, apps, connected, serverIp } = useApp()
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

  // Footer facts: the exit address while connected, plus the server's
  // location (remembered across sessions so it survives a disconnect).
  const [exit, setExit] = useState<{ ip: string; place: string } | null>(null)
  const [srvPlace, setSrvPlace] = useState<string>(() => {
    try {
      return localStorage.getItem("qc-srv-place") || ""
    } catch {
      return ""
    }
  })
  useEffect(() => {
    let alive = true
    void netInfo()
      .then((n) => {
        if (alive && n) setExit({ ip: n.ip, place: n.place })
      })
      .catch(() => {
        /* the facts fall back to what we already know */
      })
    return () => {
      alive = false
    }
  }, [connected])
  useEffect(() => {
    if (!connected || !exit?.place) return
    setSrvPlace(exit.place)
    try {
      localStorage.setItem("qc-srv-place", exit.place)
    } catch {
      /* private mode */
    }
  }, [connected, exit])
  const placeText = connected ? exit?.place || srvPlace || displayHost(serverIp) : srvPlace || "-"
  const ipText = exit?.ip || "-"

  const routing =
    appsMode === "all"
      ? `${t("vpnFor")} ${t("wholeDevice")}`
      : appsMode === "allow"
        ? t("appsOnly")
        : t("appsExcept")
  const routingCount = appsMode !== "all" && apps.length ? ` · ${apps.length}` : ""

  return (
    <div className="flex flex-col gap-3">
      <Hero />

      {/* one instrument panel: preset pair on top, then the card, routing
          and connection rows below it, all on the same glass surface */}
      <Panel>
        {/* one-tap preset: Streaming vs Gaming, each with its domain shown */}
        <div className="grid grid-cols-2 gap-3 border-b border-line p-3">
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
                  "flex min-h-[58px] items-center gap-3 rounded-[14px] border px-3.5 py-2 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",
                  isActive
                    ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
                    : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",
                )}
              >
                <span
                  className={cn(
                    "grid size-9 shrink-0 place-items-center rounded-[10px] border transition-colors",
                    isActive
                      ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                      : "border-line bg-white/[0.03] text-txt3",
                  )}
                >
                  <Icon className="size-4" aria-hidden />
                </span>
                <span className="min-w-0 flex-1">
                  <span className={cn("block truncate text-[13.5px] font-semibold", isActive ? "text-txt" : "text-txt2")} dir="auto">
                    {t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}
                  </span>
                  <span className="block truncate font-mono text-[11px] tabular-nums text-txt3">{sni}</span>
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

        <Row label={t("domainSni")}>
          {customOpen ? (
            <div className="flex w-full max-w-[400px] items-center gap-2">
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
                className="h-11 rounded-[12px] border-line bg-white/[0.02] text-[13px]"
              />
              <Button
                size="sm"
                className="h-11 shrink-0 rounded-[12px] px-4 text-[13px]"
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
              className="group flex h-11 w-full max-w-[400px] items-center justify-between gap-3 rounded-[12px] border border-line bg-white/[0.02] px-3.5 text-[13px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"
            >
              <span className="truncate" dir="auto">{domText}</span>
              <ChevronDown className="size-4 shrink-0 text-txt2 transition-[color,transform] duration-200 group-hover:translate-y-px group-hover:text-brand-strong" aria-hidden />
            </button>
          )}
        </Row>

        <Row label={t("routing")}>
          <button
            type="button"
            onClick={onOpenApps}
            className="group flex h-11 w-full max-w-[400px] items-center justify-between gap-3 rounded-[12px] border border-line bg-white/[0.02] px-3.5 text-[13px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"
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
        </Row>

        <Row label={t("transport")} align="start">
          <div>
            <Segmented
              id="transport"
              value={transport}
              onChange={setTransport}
              options={[
                { value: "vless", label: t("trStandard") },
                { value: "wg", label: t("trWg") },
                { value: "hy2", label: "Hysteria2" },
              ]}
            />
            <p className="mt-1.5 text-[11.5px] text-txt3">
              {transport === "vless" ? t("trNoteVless") : transport === "wg" ? t("trNoteWg") : t("trNoteHy2")}
            </p>
            {transport !== "vless" && (
              <p
                role="status"
                className="mt-2 flex items-start gap-2 rounded-[10px] border border-warn-line bg-warn-bg px-2.5 py-1.5 text-[11.5px] leading-[1.45] text-warn"
              >
                <TriangleAlert className="mt-px size-3.5 shrink-0" aria-hidden />
                <span>{t("quotaWarn")}</span>
              </p>
            )}
          </div>
        </Row>
      </Panel>

      {/* the footer facts, one card each on the app material: where the
          server sits, the address the world sees, and the live state */}
      <div className="grid grid-cols-3 gap-3">
        <Fact icon={Globe} label={t("srvLocation")} value={placeText} />
        <Fact icon={MapPin} label={t("yourIp")} value={ipText} />
        <Fact
          icon={ArrowUpDown}
          label={t("statusLbl")}
          value={connected ? t("connected") : t("notConnected")}
          dot={connected}
        />
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

/** One footer fact: icon tile, quiet label, one value line. */
function Fact({
  icon: Icon,
  label,
  value,
  dot,
}: {
  icon: LucideIcon
  label: string
  value: string
  dot?: boolean
}) {
  return (
    <div className="glass flex items-center gap-3 rounded-[16px] px-3.5 py-2">
      <span className="grid size-9 shrink-0 place-items-center rounded-[11px] border border-line bg-white/[0.03] text-brand-strong">
        <Icon className="size-4" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="text-[11px] text-txt3">{label}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[13px] text-txt">
          {dot !== undefined && (
            <span className={cn("size-1.5 shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-txt3")} aria-hidden />
          )}
          <span className="truncate" dir="auto">
            {value}
          </span>
        </div>
      </div>
    </div>
  )
}
