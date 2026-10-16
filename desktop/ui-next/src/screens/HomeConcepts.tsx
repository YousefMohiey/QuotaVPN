import { useState, type ReactNode } from "react"
import { ChevronDown, ChevronRight, Gamepad2, SlidersHorizontal, Tv, Users } from "lucide-react"
import { cn } from "@/lib/utils"
import { Dial, type DialState } from "@/components/Dial"
import { WeQuotaCard } from "@/components/WeQuotaCard"
import { FactsCard, useGeo, useServerAddr, useTick, useTraffic } from "@/components/Hero"
import { PickerDialog, type PickerItem } from "@/components/PickerDialog"
import { Segmented } from "@/components/Segmented"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useApp, type PresetKind } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { CUSTOM_SNI, DEFAULT_SNI, SNIS, labelForSni } from "@/lib/snis"
import { displayHost, fmtBytes, fmtDuration } from "@/lib/format"

/* TEMP review build: three Home arrangements to pick from, opened with
   ?alt=1 / ?alt=2 / ?alt=3. After the pick the winner moves into Home.tsx
   and this file goes away. */

/** The dial, centered, reading under it - the connect block every
    arrangement shares. The band version carries the facts at its foot. */
function ConnectStack({ strip = false, stretch = false, profiles }: { strip?: boolean; stretch?: boolean; profiles?: ReactNode }) {
  const { t } = useI18n()
  const { phase, connected, busy, toggle, card, rx, tx, sessionStart, status } = useApp()
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const working = phase === "connecting"
  useTick(1000, connected)
  const traffic = useTraffic(rx, tx)

  return (
    <section
      className={cn(
        "flex min-h-0 flex-col items-center rounded-[16px] border bg-[rgb(18_19_23/0.72)] px-5 pt-5 text-center",
        connected ? "border-[rgb(92_178_142/0.45)]" : "border-line",
        strip ? "pb-4" : "pb-5",
        strip && !stretch ? "flex-none" : "flex-1",
      )}
    >
      <div className="flex w-full min-w-0 flex-1 flex-col items-center justify-center">
        <Dial state={state} onClick={toggle} disabled={busy && phase !== "connecting"} />
        <div className="mt-3 flex min-w-0 flex-col items-center">
        <span
          className={cn(
            "inline-flex items-center gap-2 text-[13px] font-medium",
            connected ? "text-[var(--green)]" : working ? "text-brand-strong" : "text-txt2",
          )}
        >
          <span
            aria-hidden
            className={cn(
              "size-1.5 rounded-full",
              connected ? "bg-[var(--green)]" : working ? "pulse-dot bg-[var(--brand)]" : "bg-txt3",
            )}
          />
          {connected ? t("connected") : working ? t("working") : t("notConnected")}
        </span>
        {profiles ? <div className="mt-2.5 flex w-full justify-center">{profiles}</div> : null}
        {connected ? (
          <p className="mt-1.5 max-w-full truncate text-[12.5px] tabular-nums text-txt2">
            <span className="text-txt3">{t("sessLabel")} </span>
            {sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
            <span className="text-txt3"> · ↓ {fmtBytes(rx)} ↑ {fmtBytes(tx)} · </span>↓ {traffic.down.num}{" "}
            {traffic.down.unit} ↑ {traffic.up.num} {traffic.up.unit}
          </p>
        ) : (
          <p className="mt-1.5 max-w-full truncate font-mono text-[11.5px] text-txt2" dir="auto">
            {displayHost(card?.sni)}
          </p>
        )}
        {status ? <div className="mt-1 max-w-full select-text truncate text-[11px] text-txt3">{status}</div> : null}
        </div>
      </div>
      {strip ? <FactsStrip /> : null}
    </section>
  )
}

/** The facts as one strip of three cells with hairlines between them. */
function FactsStrip() {
  const { t } = useI18n()
  const { connected, serverIp } = useApp()
  const geo = useGeo(connected)
  const serverAddr = useServerAddr(connected, serverIp)
  const ipText = connected ? serverAddr || geo?.ip || "-" : geo?.ip || "-"

  return (
    <div className="mt-2.5 grid w-full grid-cols-3 items-start divide-x divide-line border-t border-line pt-2.5 text-start">
      <div className="flex min-w-0 flex-col pe-5">
        <span className="text-[11px] font-semibold uppercase tracking-[0.07em] text-txt2">{t("yourLocation")}</span>
        <span className="mt-1 truncate text-[13.5px] font-medium text-txt" dir="auto" title={geo?.place || ""}>
          {geo?.place || "-"}
        </span>
      </div>
      <div className="flex min-w-0 flex-col px-5">
        <span className="text-[11px] font-semibold uppercase tracking-[0.07em] text-txt2">{t("yourIp")}</span>
        <span className="mt-1 select-text truncate text-[13.5px] font-medium tabular-nums text-txt" dir="auto">
          {ipText}
        </span>
      </div>
      <div className="flex min-w-0 flex-col ps-5">
        <span className="text-[11px] font-semibold uppercase tracking-[0.07em] text-txt2">{t("ispLbl")}</span>
        <span className="mt-1 truncate text-[13.5px] font-medium text-txt" dir="auto" title={geo?.isp || ""}>
          {geo?.isp || "-"}
        </span>
      </div>
    </div>
  )
}

/** The three arrangements, sharing every control below the connect block. */
export function HomeConcept({ alt, onOpenApps }: { alt: string; onOpenApps: () => void }) {
  const { t } = useI18n()
  const { cards, card, pickCard, preset, setPreset, ensurePresetCard, applyDomain, transport, setTransport, appsMode, apps } =
    useApp()
  const [domainOpen, setDomainOpen] = useState(false)
  const [customOpen, setCustomOpen] = useState(false)
  const [customVal, setCustomVal] = useState("")
  const [domainBusy, setDomainBusy] = useState(false)
  const [creating, setCreating] = useState<PresetKind | null>(null)

  const activeKind: PresetKind =
    card?.card_type === "Streamerz" ? "Streamerz" : card?.card_type === "Gamerz" ? "Gamerz" : preset
  const activeName = (card?.name ?? "").split(" (")[0]
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
  const routing =
    appsMode === "all"
      ? `${t("vpnFor")} ${t("wholeDevice")}`
      : appsMode === "allow"
        ? t("appsOnly")
        : t("appsExcept")
  const routingCount = appsMode !== "all" && apps.length ? ` · ${apps.length}` : ""
  const trHint = transport === "hy2" ? t("trNoteHy2") : transport === "wg" ? t("trNoteWg") : ""

  const profileButton = (kind: PresetKind) => {
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
          "flex items-center gap-3 rounded-[12px] border p-3 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",
          isActive
            ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
            : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",
        )}
      >
        <span
          className={cn(
            "grid size-10 shrink-0 place-items-center rounded-[10px] border transition-colors",
            isActive ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong" : "border-line bg-white/[0.03] text-txt3",
          )}
        >
          <Icon className="size-[18px]" aria-hidden />
        </span>
        <span className="min-w-0 flex-1">
          <span className={cn("block truncate text-[14px] font-semibold", isActive ? "text-txt" : "text-txt2")} dir="auto">
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
  }

  /* The profile switch, sized to sit right under the connect button: who
     you connect as, one tap, without leaving the hero. */
  const profileSwitcher = (
    <div
      className="mx-auto flex w-[340px] max-w-full items-center gap-1 rounded-[12px] border border-line bg-white/[0.02] p-1"
      role="group"
      aria-label={t("profilesTitle")}
    >
      {(["Gamerz", "Streamerz"] as const).map((kind) => {
        const mine = cards.find((c) => c.card_type === kind)
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
              "flex h-9 min-w-0 flex-1 items-center justify-center gap-2 rounded-[9px] text-[13px] transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",
              isActive
                ? "bg-[var(--brand-bg)] font-medium text-txt shadow-[inset_0_0_0_1px_var(--brand-line)]"
                : "text-txt2 hover:bg-white/[0.03] hover:text-txt",
            )}
          >
            <Icon className="size-4 shrink-0" aria-hidden />
            <span className="truncate">{t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}</span>
          </button>
        )
      })}
    </div>
  )

  const rowField = (label: string, hint: string, children: ReactNode) => (
    <div className="flex items-center gap-3 py-2">
      <span className="w-24 shrink-0 truncate text-[12.5px] text-txt2">{label}</span>
      <div className="min-w-0 flex-1">
        {children}
        {hint ? <p className="mt-1 text-[11px] text-txt3">{hint}</p> : null}
      </div>
    </div>
  )

  const rowsInner = (
    <div className="flex flex-col divide-y divide-line">
      {rowField(
        t("domainSni"),
        "",
        customOpen ? (
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
            <span className={cn("truncate", domText.length > 17 && "text-[11px]")} dir="auto">
              {domText}
            </span>
            <ChevronDown
              className="size-4 shrink-0 text-txt2 transition-[color,transform] duration-200 group-hover:translate-y-px group-hover:text-brand-strong"
              aria-hidden
            />
          </button>
        ),
      )}
      {rowField(
        t("routing"),
        "",
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
        </button>,
      )}
      {rowField(
        t("transport"),
        trHint,
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
        />,
      )}
    </div>
  )

  const rowsCard = (
    <section className="flex flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      {rowsInner}
    </section>
  )

  const setupCard = (
    <section className="flex flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <div className="flex items-center gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
          <SlidersHorizontal className="size-[18px]" aria-hidden />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[13.5px] font-semibold text-txt">{t("setupTitle")}</h2>
          <p className="mt-0.5 truncate text-[11px] text-txt2" dir="auto">
            {activeName || "-"}
          </p>
        </div>
      </div>
      <div className="mt-3 grid grid-cols-2 gap-3" role="group" aria-label={t("setupTitle")}>
        {profileButton("Gamerz")}
        {profileButton("Streamerz")}
      </div>
      <div className="my-2.5 border-t border-line" aria-hidden />
      {rowsInner}
    </section>
  )

  const profilesCard = (
    <section className="flex flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <div className="flex items-center gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
          <Users className="size-[18px]" aria-hidden />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[13.5px] font-semibold text-txt">{t("profilesTitle")}</h2>
        </div>
      </div>
      <div className="mt-3 flex flex-col gap-3" role="group" aria-label={t("profilesTitle")}>
        {profileButton("Gamerz")}
        {profileButton("Streamerz")}
      </div>
    </section>
  )

  const connectionCard = (
    <section className="flex flex-col justify-center rounded-[16px] border border-line bg-[rgb(18_19_23/0.72)] px-5 py-4">
      <div className="flex items-center gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-full border border-line bg-[rgb(255_255_255/0.02)] text-txt">
          <SlidersHorizontal className="size-[18px]" aria-hidden />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[13.5px] font-semibold text-txt">{t("setupTitle")}</h2>
          <p className="mt-0.5 truncate text-[11px] text-txt2" dir="auto">
            {activeName || "-"}
          </p>
        </div>
      </div>
      <div className="mt-3">{rowsInner}</div>
    </section>
  )

  const dialog = (
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
  )

  if (alt === "1") {
    /* 1: the band on top with the facts at its foot, setup and package under */
    return (
      <>
        <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">
          <ConnectStack strip />
          <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,7fr)_minmax(0,5fr)] gap-3">
            {setupCard}
            <WeQuotaCard />
          </div>
        </div>
        {dialog}
      </>
    )
  }

  if (alt === "2") {
    /* 2: two rows of two - connection and package on top, setup and facts under */
    return (
      <>
        <div className="mx-auto grid h-full min-h-0 w-full max-w-[1040px] grid-cols-[minmax(0,7fr)_minmax(0,5fr)] grid-rows-[minmax(0,1fr)_auto] gap-3">
          <ConnectStack />
          <WeQuotaCard />
          {setupCard}
          <div className="flex min-h-0 flex-col [&>section]:flex [&>section]:min-h-0 [&>section]:flex-1 [&>section]:flex-col [&>section>div]:flex-1 [&>section>div]:justify-between">
            <FactsCard />
          </div>
        </div>
        {dialog}
      </>
    )
  }

  if (alt === "4") {
    /* 4: connect on top with the profile switch right under the button,
       then the two cards that carry the rest: setup and package */
    return (
      <>
        <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">
          <div className="flex min-h-0 flex-[3_1_auto] flex-col">
            <ConnectStack strip stretch profiles={profileSwitcher} />
          </div>
          <div className="grid min-h-0 flex-[1_1_auto] grid-cols-[minmax(0,6.5fr)_minmax(0,5.5fr)] gap-3">
            {connectionCard}
            <WeQuotaCard />
          </div>
        </div>
        {dialog}
      </>
    )
  }

  /* 3: the rail - connect and settings left, package, profiles and facts right */
  return (
    <>
      <div className="mx-auto grid h-full min-h-0 w-full max-w-[1040px] grid-cols-[minmax(0,7fr)_minmax(0,5fr)] grid-rows-[minmax(0,1fr)] gap-3">
        <div className="flex min-h-0 flex-col gap-3">
          <ConnectStack />
          {rowsCard}
        </div>
        <div className="flex min-h-0 flex-col gap-3">
          <WeQuotaCard />
          {profilesCard}
          <FactsCard />
        </div>
      </div>
      {dialog}
    </>
  )
}
