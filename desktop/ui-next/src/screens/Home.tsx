import { useState } from "react"
import { ChevronDown, ChevronRight, Gamepad2, Tv } from "lucide-react"
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

  const cells: Array<{ id: string; label: string; body: React.ReactNode; cls: string; chevron?: "down" | "right"; onClick?: () => void }> = [
    { id: "domain", label: t("domainSni"), cls: "col-span-3 border-e border-b border-line", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{domText}</span>, chevron: "down", onClick: () => setDomainOpen(true) },
    { id: "routing", label: t("routing"), cls: "col-span-3 border-b border-line", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{routing}{routingCount}</span>, chevron: "right", onClick: onOpenApps },
    { id: "transport", label: t("transport"), cls: "col-span-6 border-b border-line", body: <Segmented id="transport" value={transport} onChange={setTransport} boxed options={[
      { value: "vless", label: t("trStandard") },
      { value: "hy2", label: "Hysteria2", hint: t("trNoteHy2") },
      { value: "wg", label: t("trWg"), hint: t("trNoteWg") },
    ]} /> },
    { id: "ip", label: t("yourIp"), cls: "col-span-2 border-e border-line", body: (
      <Tip text={t("copy")}>
        <button
          type="button"
          onClick={() => void take()}
          aria-label={`${t("copy")} ${ipText}`}
          className={cn("press min-w-0 cursor-pointer truncate rounded-[4px] text-[12.5px] tabular-nums transition-colors hover:text-brand-strong", copied ? "text-[var(--green)]" : "text-txt")}
          dir="auto"
        >
          {copied ? t("copied") : ipText}
        </button>
      </Tip>
    ) },
    { id: "location", label: t("yourLocation"), cls: "col-span-2 border-e border-line", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{geo?.place || "-"}</span> },
    { id: "isp", label: t("ispLbl"), cls: "col-span-2", body: <span className="truncate text-[12.5px] text-txt" dir="auto">{geo?.isp || "-"}</span> },
  ]

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1120px] flex-col gap-2 overflow-y-auto [scrollbar-width:none]">
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,330px)_minmax(0,1fr)] gap-2.5">
        {/* who you connect as, and what the line holds */}
        <div className="flex min-h-0 flex-col gap-3">
          <div className="flex shrink-0 flex-col gap-2" role="group" aria-label={t("setupTitle")}>
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
                  "press flex flex-col gap-1.5 rounded-[var(--r-card)] border p-3 text-start transition-colors duration-150 disabled:cursor-wait disabled:opacity-70",
                  isActive
                    ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-txt"
                    : "border-line bg-[var(--panel)] text-txt2 hover:border-line-strong hover:text-txt",
                )}
              >
                <span className="flex min-w-0 items-center gap-2">
                  <Icon className={cn("size-4 shrink-0", isActive ? "text-brand-strong" : "text-txt3")} aria-hidden />
                  <span className="truncate text-[13px] font-medium">{t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}</span>
                  <span aria-hidden className={cn("ml-auto size-1.5 shrink-0 rounded-full", isActive ? "bg-[var(--brand-vivid)]" : "bg-txt3/40")} />
                </span>
                <span className="truncate font-mono text-[10.5px] text-txt3" dir="auto">{sni}</span>
              </button>
            )
          })}
          </div>

          <aside className="flex min-h-0 flex-none flex-col overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
            <WeQuotaCard />
          </aside>

          <div className="flex shrink-0 flex-col gap-1.5 rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-4 py-2.5">
            {[
              { label: t("sessLabel"), value: connected && sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-" },
              { label: t("statDown"), value: connected ? `${traffic.down.num} ${traffic.down.unit}` : "-" },
              { label: t("statUp"), value: connected ? `${traffic.up.num} ${traffic.up.unit}` : "-" },
            ].map((r) => (
              <span key={r.label} className="flex min-w-0 items-baseline justify-between gap-2">
                <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{r.label}</span>
                <span className="truncate text-[13px] tabular-nums text-txt">{r.value}</span>
              </span>
            ))}
          </div>
        </div>

        {/* the centre: the command bar over the six cells */}
        <div className="flex min-h-0 flex-col gap-3">
          <section className="flex min-h-0 flex-[0.8] items-center gap-6 rounded-[var(--r-card)] border border-line bg-[var(--panel)] px-5 py-4">
            <Dial size={152} state={state} onClick={toggle} disabled={busy && phase !== "connecting"} />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span
                  aria-hidden
                  className={cn("size-2 shrink-0 rounded-full", connected ? "bg-[var(--green)]" : working ? "pulse-dot bg-[var(--brand)]" : "bg-txt3")}
                />
                <span className={cn("truncate font-display text-[26px] font-semibold leading-tight tracking-[-0.02em] transition-colors duration-300", connected ? "text-[var(--green)]" : "text-txt")}>
                  {connected ? t("connected") : working ? t("working") : t("notConnected")}
                </span>
              </div>
              <div className="mt-1 truncate font-mono text-[11.5px] text-txt3" dir="auto">
                {displayHost(card?.sni) || "-"}
              </div>
              {status ? (
                <div className="mt-1.5 line-clamp-2 text-[11.5px] leading-snug text-txt2" role="status">{status}</div>
              ) : null}
            </div>
          </section>

          <section className="grid min-h-0 flex-1 grid-cols-6 grid-rows-3 overflow-hidden rounded-[var(--r-card)] border border-line bg-[var(--panel)]">
            {cells.map((c) => (
              <div
                key={c.id}
                className={cn("flex min-h-0 min-w-0 flex-col justify-center gap-1.5 px-4 py-2.5", c.cls)}
              >
                <span className="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-txt2">{c.label}</span>
                {c.onClick ? (
                  c.id === "domain" && customOpen ? (
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
                      onClick={c.onClick}
                      aria-busy={c.label === t("domainSni") ? domainBusy : undefined}
                      className="press group flex h-8 w-full min-w-0 items-center justify-between gap-2 rounded-[7px] border border-line bg-white/[0.02] px-2.5 text-txt transition-colors duration-150 hover:border-[var(--brand-line)]"
                    >
                      {c.body}
                      {c.chevron === "down" ? (
                        <ChevronDown className="size-3.5 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
                      ) : (
                        <ChevronRight className="size-3.5 shrink-0 text-txt3 transition-colors group-hover:text-brand-strong" aria-hidden />
                      )}
                    </button>
                  )
                ) : (
                  <span className="flex min-w-0">{c.body}</span>
                )}
                {c.id === "domain" && domainErr ? (
                  <span className="truncate text-[11px] text-[var(--amber)]" role="alert">{domainErr}</span>
                ) : null}
              </div>
            ))}
          </section>
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
          void applyDomain(v).then((r) => {
            if (!r.ok) setDomainErr(r.msg || t("weGenericFail"))
            else setDomainErr("")
          })
        }}
      />
    </div>
  )
}
