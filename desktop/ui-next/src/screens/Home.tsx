import { useState } from "react"
import { motion, useReducedMotion } from "motion/react"
import { Building2, ChevronDown, ChevronRight, Copy, Gamepad2, Globe, LayoutGrid, Link2, MapPin, Tv } from "lucide-react"
import { useGeo, useServerAddr, useTick, useTraffic } from "@/components/Hero"
import { Dial, type DialState } from "@/components/Dial"
import { WeQuotaCard } from "@/components/WeQuotaCard"
import { PickerDialog, type PickerItem } from "@/components/PickerDialog"
import { Segmented } from "@/components/Segmented"
import { Tip } from "@/components/Tip"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { copyText } from "@/lib/clipboard"
import { displayHost, fmtDuration } from "@/lib/format"
import { useApp, type PresetKind } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { CUSTOM_SNI, DEFAULT_SNI, SNIS, labelForSni } from "@/lib/snis"
import { cn } from "@/lib/utils"

// Home, the console: the profiles are a switch on the left, the command bar and
// the six settings cells hold the centre, the WE line takes the right column.
// One action, one reading, no dead space.
export function Home({ onOpenApps }: { onOpenApps: () => void }) {
  const { t } = useI18n()
  // motion is a report on state, never a decoration: with reduced motion the
  // values still change, they just arrive instead of travelling
  const calm = useReducedMotion()
  const {
    cards,
    card,
    pickCard,
    preset,
    setPreset,
    ensurePresetCard,
    applyDomain,
    transport,
    setTransport,
    appsMode,
    apps,
    connected,
    busy,
    phase,
    toggle,
    rx,
    tx,
    sessionStart,
    serverIp,
    status,
    setStatus,
  } = useApp()

  // The command bar reads the same numbers the stage never hid.
  useTick(1000, connected)
  const traffic = useTraffic(rx, tx)
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const working = phase === "connecting"

  const [domainOpen, setDomainOpen] = useState(false)
  const [customOpen, setCustomOpen] = useState(false)
  const [customVal, setCustomVal] = useState("")
  const [domainBusy, setDomainBusy] = useState(false)
  const [domainErr, setDomainErr] = useState("")

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
      setDomainErr("")
    } else {
      setDomainErr(r.msg || t("weGenericFail"))
    }
  }

  const [creating, setCreating] = useState<PresetKind | null>(null)
  const routing =
    appsMode === "all"
      ? `${t("vpnFor")} ${t("wholeDevice")}`
      : appsMode === "allow"
        ? t("appsOnly")
        : t("appsExcept")
  const routingCount = appsMode !== "all" && apps.length ? ` · ${apps.length}` : ""

  // The readings: where this machine sits and appears from right now.
  const geo = useGeo(connected)
  const serverAddr = useServerAddr(connected, serverIp)
  const ipText = connected ? serverAddr || geo?.ip || "-" : geo?.ip || "-"
  const [copied, setCopied] = useState(false)
  const take = async () => {
    if (!ipText || ipText === "-") return
    if (!(await copyText(ipText))) {
      setStatus(t("copyFailed"))
      return
    }
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1400)
  }

  const cells: Array<{ id: string; label: string; icon?: typeof Globe; body: React.ReactNode; cls: string; chevron?: "down" | "right"; onClick?: () => void }> = [
    { id: "domain", label: t("domainSni"), icon: Globe, cls: "col-span-3 border-e border-b border-line", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{domText}</span>, chevron: "down", onClick: () => setDomainOpen(true) },
    { id: "routing", label: t("routing"), icon: LayoutGrid, cls: "col-span-3 border-b border-line", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{routing}{routingCount}</span>, chevron: "right", onClick: onOpenApps },
    { id: "transport", label: t("transport"), icon: Link2, cls: "col-span-6 border-b border-line", body: <Segmented id="transport" value={transport} onChange={setTransport} boxed options={[
      { value: "vless", label: t("trStandard") },
      { value: "hy2", label: "Hysteria2", hint: t("trNoteHy2") },
      { value: "wg", label: t("trWg"), hint: t("trNoteWg") },
    ]} /> },
    { id: "ip", label: t("yourIp"), icon: MapPin, cls: "col-span-2 border-e border-line", body: (
      <Tip text={t("copy")}>
        <button
          type="button"
          onClick={() => void take()}
          aria-label={`${t("copy")} ${ipText}`}
          className="press group inline-flex min-w-0 items-center gap-1.5 text-[12.5px] tabular-nums transition-colors"
        >
          <span className={cn("truncate", copied ? "text-[var(--green)]" : "text-txt")} dir="auto">
            {copied ? t("copied") : ipText}
          </span>
          <Copy className="size-3 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
        </button>
      </Tip>
    ) },
    { id: "location", label: t("yourLocation"), icon: MapPin, cls: "col-span-2 border-e border-line", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{geo?.place || "-"}</span> },
    { id: "isp", label: t("ispLbl"), icon: Building2, cls: "col-span-2", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{geo?.isp || "-"}</span> },
  ]

  const controls = (
    <section className="grid shrink-0 grid-cols-3 overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
      {cells.slice(0, 3).map((c, i) => (
        <div
          key={c.id}
          className={cn("flex min-h-0 min-w-0 flex-col justify-center gap-1.5 px-4 py-3", i < 2 && "border-e border-line")}
        >
          <span className="flex min-w-0 items-center gap-1.5 text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">
            {c.icon ? <c.icon className="size-3.5 shrink-0 text-txt3" aria-hidden /> : null}
            <span className="truncate">{c.label}</span>
          </span>
          {c.id === "domain" && customOpen ? (
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
                className="h-9 min-w-0 rounded-[7px] border-line bg-white/[0.02] px-2 text-[12.5px]"
              />
              <Button
                size="sm"
                className="h-9 shrink-0 rounded-[7px] px-2.5 text-[12px]"
                disabled={domainBusy || !customVal.trim()}
                onClick={() => void submitCustom()}
              >
                {t("apply")}
              </Button>
            </div>
          ) : c.onClick ? (
            <button
              type="button"
              onClick={c.onClick}
              aria-busy={c.id === "domain" ? domainBusy : undefined}
              className="press group flex h-9 w-full min-w-0 items-center justify-between gap-2 rounded-[7px] border border-line bg-white/[0.02] px-2.5 text-txt transition-colors duration-150 hover:border-[var(--brand-line)]"
            >
              {c.body}
              {c.chevron === "down" ? (
                <ChevronDown className="size-3.5 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
              ) : (
                <ChevronRight className="size-3.5 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
              )}
            </button>
          ) : (
            <span className="flex min-w-0">{c.body}</span>
          )}
          {c.id === "domain" && domainErr ? (
            <span className="truncate text-[11px] text-[var(--red)]" role="alert">{domainErr}</span>
          ) : null}
        </div>
      ))}
    </section>
  )

  // what the session reports, said once in a line instead of in second slab
  const readings = (
    <div className="flex shrink-0 flex-wrap items-center gap-x-5 gap-y-1 px-1 pb-0.5 text-[11.5px]">
      {cells.slice(3).map((c) => (
        <span key={c.id} className="flex min-w-0 items-center gap-1.5">
          <span className="shrink-0 text-txt3">{c.label}</span>
          <span className="min-w-0 truncate text-txt">{c.body}</span>
        </span>
      ))}
    </div>
  )

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1180px] flex-col gap-2.5">
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-2.5">
        <div className="flex min-h-0 flex-col">
          {/* one hero: the control sits in the middle of its own room, and
              everything else on this card answers it */}
          <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
            <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3.5 px-5 py-4">
              <Dial size={196} state={state} onClick={toggle} disabled={busy && phase !== "connecting"} />
              <div className="flex min-w-0 flex-col items-center gap-1 text-center">
                <span className="flex min-w-0 items-center justify-center gap-2">
                  <span
                    aria-hidden
                    className={cn("size-2 shrink-0 rounded-full", connected ? "bg-[var(--green)]" : working ? "pulse-dot bg-[var(--brand)]" : "bg-txt3")}
                  />
                  <motion.span
                    key={connected ? "on" : working ? "connecting" : "off"}
                    initial={calm ? { opacity: 0 } : { opacity: 0, y: 2 }}
                    animate={calm ? { opacity: 1 } : { opacity: 1, y: 0 }}
                    transition={{ duration: calm ? 0.12 : 0.18, ease: [0.23, 1, 0.32, 1] }}
                    className={cn(
                      "truncate font-display text-[22px] font-semibold leading-tight tracking-[-0.02em]",
                      connected ? "text-[var(--green)]" : "text-txt",
                    )}
                  >
                    {connected ? t("connected") : working ? t("working") : t("notConnected")}
                  </motion.span>
                </span>
                <span className="max-w-[260px] truncate font-mono text-[11.5px] text-txt2" dir="auto">
                  {displayHost(card?.sni) || "-"}
                </span>
                {status ? (
                  <span className="mt-0.5 line-clamp-2 max-w-[280px] text-[11.5px] leading-snug text-txt2" role="status">{status}</span>
                ) : null}
              </div>
              <div className="grid w-full max-w-[420px] shrink-0 grid-cols-2 gap-2.5" role="group" aria-label={t("setupTitle")}>
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
                    "press relative flex min-h-0 min-w-0 items-center gap-2.5 overflow-hidden rounded-[var(--r-card)] px-3 py-2.5 text-start transition-colors duration-150 disabled:cursor-wait disabled:opacity-70",
                    isActive ? "text-txt" : "text-txt2 hover:bg-white/[0.02] hover:text-txt",
                  )}
                >
                  {isActive ? (
                    <motion.span
                      layoutId="profile-ring"
                      aria-hidden
                      className="absolute inset-0 rounded-[var(--r-card)] border border-[var(--brand-line)] bg-[var(--brand-bg)]"
                      transition={calm ? { duration: 0 } : { type: "spring", stiffness: 520, damping: 40 }}
                    />
                  ) : null}
                  <span
                    className={cn(
                      "relative grid size-8 shrink-0 place-items-center rounded-[9px] bg-white/[0.04] transition-colors duration-150",
                      isActive ? "text-brand-strong" : "text-txt3",
                    )}
                  >
                    <Icon className="size-4" aria-hidden />
                  </span>
                  <span className="relative min-w-0 flex-1">
                    <span className="block truncate text-[12.5px] font-medium">{t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}</span>
                    <span className="mt-0.5 block truncate font-mono text-[11px] text-txt2" dir="auto">{sni}</span>
                  </span>
                </button>
              )
            })}
              </div>

            <div className="mt-1 grid w-full shrink-0 grid-cols-3 border-t border-line">
              {[
                { label: t("sessLabel"), value: connected && sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-" },
                { label: t("statDown"), value: connected ? `${traffic.down.num} ${traffic.down.unit}` : "-" },
                { label: t("statUp"), value: connected ? `${traffic.up.num} ${traffic.up.unit}` : "-" },
              ].map((r, i) => (
                <span
                  key={r.label}
                  className={cn("flex min-w-0 flex-col items-center gap-0.5 px-3 py-2.5", i < 2 && "border-e border-line")}
                >
                  <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{r.label}</span>
                  <span className="truncate text-[13.5px] tabular-nums text-txt">{r.value}</span>
                </span>
              ))}
            </div>
            </div>          </section>
        </div>

        <aside className="flex min-h-0 flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
          <WeQuotaCard />
        </aside>
      </div>

      {controls}
      {readings}

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
          void applyDomain(v).then((r) => {
            if (!r.ok) setDomainErr(r.msg || t("weGenericFail"))
            else setDomainErr("")
          })
        }}
      />
    </div>
  )
}
