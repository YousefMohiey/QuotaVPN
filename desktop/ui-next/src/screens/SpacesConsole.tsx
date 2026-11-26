import { useState } from "react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import {
  Cable,
  Check,
  Copy,
  Crown,
  Loader2,
  LogIn,
  Plus,
  Power,
  RefreshCw,
  RotateCw,
  Server,
  Trash2,
  UserMinus,
  Users,
  WifiOff,
  X,
} from "lucide-react"
import { useI18n } from "@/lib/i18n"
import { copyText } from "@/lib/clipboard"
import type { OpenRadPeer, OpenRadStatus, SpaceMember, SpacesState } from "@/lib/ipc"
import { cn } from "@/lib/utils"

type SpaceRow = SpacesState["spaces"][number]

export type SpacesConsoleProps = {
  spaces: SpaceRow[]
  selected: string
  activeId: string
  space: SpaceRow | null
  members: SpaceMember[]
  active: boolean
  live: boolean
  lan: OpenRadStatus | null
  lanPeers: OpenRadPeer[]
  lanBusy: boolean
  lanError: string
  busy: string | null
  error: string
  note: string
  apiBase: string
  onSelect: (id: string) => void
  onRefresh: () => void
  onActivate: () => void
  onRotate: () => void
  onLanConnect: () => void
  onLanStop: () => void
  onRemove: () => void
  onKick: (memberId: string, name: string) => void
  onCreate: () => void
  onJoin: () => void
  onCoordinator: () => void
  onErrorClear: () => void
}

function initial(name: string) {
  return (name.trim()[0] ?? "?").toUpperCase()
}

function orbit(index: number, count: number) {
  const angle = -Math.PI / 2 + (index * Math.PI * 2) / Math.max(count, 1)
  return {
    x: 50 + Math.cos(angle) * 37,
    y: 50 + Math.sin(angle) * 33,
    angle,
  }
}

function LanPanel({
  lan,
  peers,
  busy,
  error,
  onConnect,
  onStop,
}: {
  lan: OpenRadStatus | null
  peers: OpenRadPeer[]
  busy: boolean
  error: string
  onConnect: () => void
  onStop: () => void
}) {
  const { t } = useI18n()
  const calm = useReducedMotion()
  const on = lan?.phase === "connected"
  const vip = lan?.snapshot?.vip ?? ""
  const device = lan?.node_name ?? ""
  const word = on
    ? peers.length > 0
      ? t("lanReady")
      : t("lanWaitPeer")
    : busy
      ? t("lanConnectingShort")
      : error
        ? t("lanFailed")
        : t("lanOff")
  const hint = on
    ? peers.length > 0
      ? t("lanReadyHint")
      : t("lanWaitHint")
    : busy
      ? t("lanBusyHint")
      : t("lanOffHint")

  return (
    <div className="border-t border-line px-5 pb-4 pt-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[10.5px] uppercase tracking-[0.14em] text-txt3">
            <span
              className={cn(
                "size-1.5 rounded-full",
                on ? "bg-[var(--green)]" : busy ? "bg-[var(--amber)]" : "bg-txt3/40",
              )}
            />
            {t("lanTitle")}
            {device ? <span className="normal-case tracking-normal text-txt3/70">{device}</span> : null}
            {vip ? <span className="font-mono normal-case tracking-normal text-txt3/80">{vip}</span> : null}
          </div>
          <AnimatePresence mode="wait" initial={false}>
            <motion.h3
              key={word}
              initial={calm ? false : { opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={calm ? undefined : { opacity: 0, y: -4 }}
              transition={{ duration: 0.18 }}
              className={cn(
                "mt-0.5 font-display text-[28px] font-semibold leading-none tracking-[-0.03em]",
                on ? "text-[var(--green)]" : error ? "text-[var(--red)]" : "text-txt",
              )}
            >
              {word}
            </motion.h3>
          </AnimatePresence>
          <p className="mt-1.5 max-w-[400px] text-[12px] leading-relaxed text-txt2">{hint}</p>
          {error ? (
            <p className="mt-1 max-w-[400px] break-words text-[11.5px] leading-snug text-[var(--red)]">
              {error}
            </p>
          ) : null}
          {on ? (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {peers.length === 0 ? (
                <span className="text-[11.5px] text-txt3">{t("lanNoPeers")}</span>
              ) : (
                peers.map((peer) => (
                  <span
                    key={peer.rid}
                    className="flex items-center gap-1.5 rounded-[6px] border border-line bg-white/[0.03] px-2 py-1 text-[11px]"
                  >
                    <span className="size-1.5 rounded-full bg-[var(--green)]" />
                    <span className="max-w-[130px] truncate text-txt">{peer.name}</span>
                    <span className="font-mono text-[10px] text-txt3">{peer.vip}</span>
                    <span
                      className={cn(
                        "rounded-[3px] px-1 py-0.5 text-[9px] uppercase tracking-[0.06em]",
                        peer.transport === "Relay"
                          ? "bg-[var(--amber)]/10 text-[var(--amber)]"
                          : "bg-[var(--green)]/10 text-[var(--green)]",
                      )}
                    >
                      {peer.transport === "Relay" ? t("lanRelay") : t("lanDirect")}
                    </span>
                    {peer.rtt_ms != null ? (
                      <span className="font-mono text-[10px] text-txt3">{Math.round(peer.rtt_ms)} ms</span>
                    ) : null}
                  </span>
                ))
              )}
            </div>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {on ? (
            <button
              type="button"
              onClick={onStop}
              disabled={busy}
              className="press flex h-9 items-center gap-2 rounded-[8px] border border-line bg-white/[0.03] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--red-line)] disabled:opacity-60"
            >
              {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <Power className="size-3.5" aria-hidden />}
              {t("lanStop")}
            </button>
          ) : (
            <button
              type="button"
              onClick={onConnect}
              disabled={busy}
              className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-4 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110 disabled:opacity-50"
            >
              {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <Cable className="size-3.5" aria-hidden />}
              {busy ? t("lanConnecting") : t("lanConnect")}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

export default function SpacesConsole(props: SpacesConsoleProps) {
  const { t } = useI18n()
  const calm = useReducedMotion()
  const [copied, setCopied] = useState("")

  const copy = async (value: string, key: string) => {
    await copyText(value)
    setCopied(key)
    window.setTimeout(() => setCopied((cur) => (cur === key ? "" : cur)), 1600)
  }

  const peers = props.members.filter((m) => !m.self_pc)
  const self = props.members.find((m) => m.self_pc)
  const online = props.members.filter((m) => m.online).length

  return (
    <div
      className="grid min-h-[600px] flex-1 gap-3 lg:h-full lg:min-h-0 lg:grid-cols-[232px_minmax(0,1fr)] xl:grid-cols-[232px_minmax(0,1fr)_286px]"
      data-space-live={props.live ? "1" : "0"}
      data-space-vpn={props.lan?.phase === "connected" ? "1" : "0"}
    >
      <aside className="flex min-h-0 flex-col overflow-hidden rounded-[18px] border border-line bg-[var(--panel)]/80">
        <div className="flex items-center justify-between px-4 pb-1 pt-4">
          <span className="text-[11px] uppercase tracking-[0.16em] text-txt3">{t("spacesTitle")}</span>
          <span className="font-mono text-[11px] text-txt3">{props.spaces.length}</span>
        </div>
        <button
          type="button"
          onClick={props.onCoordinator}
          className="mx-3 mb-2 flex min-w-0 items-center gap-2 rounded-[8px] px-2 py-1.5 text-start text-[11px] text-txt3 transition-colors hover:bg-white/[0.04] hover:text-txt2"
        >
          <Server className="size-3.5 shrink-0" aria-hidden />
          <span className="truncate font-mono" dir="ltr">
            {props.apiBase || t("spacesCoordinatorNone")}
          </span>
        </button>

        <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
          {props.spaces.map((s) => {
            const isSelected = s.id === props.selected
            const isActive = s.id === props.activeId
            const count = s.members.filter((m) => m.online).length
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => props.onSelect(s.id)}
                className={cn(
                  "relative flex w-full items-center gap-2.5 rounded-[10px] px-2.5 py-2.5 text-start transition-colors",
                  isSelected ? "bg-white/[0.05]" : "hover:bg-white/[0.025]",
                )}
              >
                {isSelected ? (
                  <motion.span
                    layoutId="spaces-rail-accent"
                    transition={calm ? { duration: 0 } : { type: "spring", stiffness: 420, damping: 36 }}
                    className="absolute inset-y-2 start-0 w-[2px] rounded-full bg-[var(--brand-strong)]"
                  />
                ) : null}
                <span
                  className={cn(
                    "grid size-9 shrink-0 place-items-center rounded-[9px] border font-mono text-[12px]",
                    isSelected
                      ? "border-[var(--brand-line)] bg-[var(--brand)]/10 text-brand-strong"
                      : "border-line bg-white/[0.02] text-txt3",
                  )}
                >
                  {initial(s.name)}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] text-txt">{s.name}</span>
                  <span className="mt-0.5 flex items-center gap-1.5 font-mono text-[10.5px] text-txt3">
                    {s.address}
                    <span className="text-txt3/40">/</span>
                    {count} {t("spacesOnline")}
                  </span>
                </span>
                {isActive ? <span className="size-1.5 shrink-0 rounded-full bg-[var(--brand)]" /> : null}
              </button>
            )
          })}
        </div>

        <div className="border-t border-line p-2.5">
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={props.onJoin}
              className="press flex h-9 items-center justify-center gap-1.5 rounded-[8px] border border-line text-[12px] text-txt2 transition-colors hover:border-[var(--brand-line)] hover:text-txt"
            >
              <LogIn className="size-3.5" aria-hidden />
              {t("spacesJoin")}
            </button>
            <button
              type="button"
              onClick={props.onCreate}
              className="press flex h-9 items-center justify-center gap-1.5 rounded-[8px] bg-[var(--brand)] text-[12px] font-medium text-white transition-[filter] hover:brightness-110"
            >
              <Plus className="size-3.5" aria-hidden />
              {t("spacesCreate")}
            </button>
          </div>
          <p className="mt-2 flex items-start gap-1.5 px-0.5 text-[10.5px] leading-snug text-txt3">
            <Users className="mt-0.5 size-3 shrink-0" aria-hidden />
            {t("spacesOnlyInvite")}
          </p>
        </div>
      </aside>

      <main className="flex min-h-0 flex-col overflow-hidden rounded-[18px] border border-line bg-[var(--panel)]">
        {!props.space ? (
          <>
            <div className="relative flex flex-1 flex-col items-center justify-center gap-4 overflow-hidden px-8 text-center">
            <div className="pointer-events-none absolute inset-0 opacity-60">
              <div className="absolute left-1/2 top-1/2 size-[360px] -translate-x-1/2 -translate-y-1/2 rounded-full border border-[var(--brand-line)]/30" />
              <div className="absolute left-1/2 top-1/2 size-[240px] -translate-x-1/2 -translate-y-1/2 rounded-full border border-line" />
              <div className="absolute left-1/2 top-1/2 size-[110px] -translate-x-1/2 -translate-y-1/2 rounded-full border border-[var(--brand-line)]/40 bg-[var(--brand)]/[0.04]" />
            </div>
            <span className="relative grid size-14 place-items-center rounded-[16px] border border-[var(--brand-line)] bg-[var(--panel)] text-brand-strong">
              <Users className="size-6" aria-hidden />
            </span>
            <h2 className="relative font-display text-[28px] font-semibold tracking-[-0.03em] text-txt">
              {t("spacesEmpty")}
            </h2>
            <p className="relative max-w-[380px] text-[13px] leading-relaxed text-txt2">{t("spacesEmptyBody")}</p>
            <div className="relative mt-1 flex gap-2">
              <button
                type="button"
                onClick={props.onJoin}
                className="press flex h-10 items-center gap-2 rounded-[9px] border border-line px-4 text-[13px] text-txt2 hover:text-txt"
              >
                <LogIn className="size-4" aria-hidden />
                {t("spacesJoin")}
              </button>
              <button
                type="button"
                onClick={props.onCreate}
                className="press flex h-10 items-center gap-2 rounded-[9px] bg-[var(--brand)] px-4 text-[13px] font-medium text-white hover:brightness-110"
              >
                <Plus className="size-4" aria-hidden />
                {t("spacesCreate")}
              </button>
            </div>
            </div>
            <LanPanel
              lan={props.lan}
              peers={props.lanPeers}
              busy={props.lanBusy}
              error={props.lanError}
              onConnect={props.onLanConnect}
              onStop={props.onLanStop}
            />
          </>
        ) : (
          <>
            <header className="flex shrink-0 items-start justify-between gap-3 px-5 pt-4">
              <div className="min-w-0">
                <div className="flex min-w-0 items-center gap-2">
                  <h2 className="truncate font-display text-[25px] font-semibold leading-none tracking-[-0.03em] text-txt">
                    {props.space.name}
                  </h2>
                  {props.space.owner ? (
                    <span className="flex shrink-0 items-center gap-1 rounded-[5px] border border-[var(--amber-line)] bg-[var(--amber)]/[0.08] px-1.5 py-0.5 text-[9.5px] uppercase tracking-[0.1em] text-[var(--amber)]">
                      <Crown className="size-2.5" aria-hidden />
                      {t("spacesOwner")}
                    </span>
                  ) : null}
                </div>
                <div className="mt-1.5 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11.5px] text-txt3">
                  <span className="flex items-center gap-1 font-mono text-txt2">
                    {props.space.address}
                    <button
                      type="button"
                      onClick={() => void copy(props.space!.address, "address")}
                      className="press grid size-5 place-items-center rounded text-txt3 hover:text-txt"
                      aria-label={t("copy")}
                    >
                      {copied === "address" ? <Check className="size-3" /> : <Copy className="size-3" />}
                    </button>
                  </span>
                  <span>{online} {t("spacesOnline")}</span>
                  <span className="font-mono">{props.space.subnet}</span>
                  <span
                    className={cn(
                      "rounded-[4px] border px-1.5 py-0.5 text-[10px] tracking-[0.04em]",
                      props.live
                        ? "border-[var(--green-line)] bg-[var(--green)]/[0.08] text-[var(--green)]"
                        : props.active
                          ? "border-[var(--brand-line)] bg-[var(--brand)]/[0.08] text-brand-strong"
                          : "border-[var(--line)] text-txt3",
                    )}
                  >
                    {props.live ? t("spacesLive") : props.active ? t("spacesReady") : t("spacesOff")}
                  </span>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                {!props.active ? (
                  <button
                    type="button"
                    onClick={props.onActivate}
                    disabled={props.busy === "activate"}
                    className="press flex h-8 items-center gap-1.5 rounded-[7px] border border-[var(--brand-line)] px-2.5 text-[11.5px] text-brand-strong transition-colors hover:bg-[var(--brand)]/[0.08] disabled:opacity-50"
                  >
                    {props.busy === "activate" ? (
                      <Loader2 className="size-3 animate-spin" aria-hidden />
                    ) : null}
                    {t("spacesConnect")}
                  </button>
                ) : null}
                <button
                  type="button"
                  onClick={props.onRefresh}
                  disabled={props.busy === "refresh"}
                  aria-label={t("refresh")}
                  className="press grid size-8 place-items-center rounded-[7px] text-txt3 hover:bg-white/[0.06] hover:text-txt disabled:opacity-50"
                >
                  {props.busy === "refresh" ? (
                    <Loader2 className="size-3.5 animate-spin" aria-hidden />
                  ) : (
                    <RefreshCw className="size-3.5" aria-hidden />
                  )}
                </button>
              </div>
            </header>

            {props.error ? (
              <div className="mx-5 mt-2 flex items-start gap-2 rounded-[8px] border border-[var(--red-line)] bg-[var(--red)]/[0.05] px-3 py-2 text-[12px] leading-snug text-[var(--red)]">
                <span className="min-w-0 flex-1">{props.error}</span>
                <button type="button" onClick={props.onErrorClear} className="press shrink-0" aria-label={t("cancel")}>
                  <X className="size-3.5" aria-hidden />
                </button>
              </div>
            ) : null}

            <div className="relative min-h-[260px] flex-1">
              <div className="absolute inset-0">
                <svg className="size-full" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden>
                  <ellipse cx="50" cy="50" rx="37" ry="33" fill="none" stroke="var(--line)" strokeWidth="0.18" />
                  <ellipse cx="50" cy="50" rx="25" ry="22" fill="none" stroke="var(--line)" strokeWidth="0.18" strokeDasharray="1.4 1.4" />
                  {peers.map((m, i) => {
                    const p = orbit(i, peers.length)
                    return (
                      <motion.path
                        key={m.id}
                        d={`M 50 50 L ${p.x} ${p.y}`}
                        fill="none"
                        stroke={props.live ? "var(--green)" : m.online ? "var(--brand-strong)" : "var(--txt3)"}
                        strokeWidth="0.22"
                        strokeOpacity={m.online ? 0.42 : 0.14}
                        initial={calm ? false : { pathLength: 0, opacity: 0 }}
                        animate={{ pathLength: 1, opacity: 1 }}
                        transition={calm ? { duration: 0 } : { duration: 0.8, delay: 0.12 + i * 0.09, ease: "easeOut" }}
                      />
                    )
                  })}
                </svg>
              </div>

              <motion.div
                animate={
                  calm
                    ? undefined
                    : props.live
                      ? { boxShadow: ["0 0 0 0 rgba(95,176,140,0)", "0 0 0 12px rgba(95,176,140,0.06)", "0 0 0 0 rgba(95,176,140,0)"] }
                      : undefined
                }
                transition={calm ? undefined : { duration: 3.2, repeat: Infinity, ease: "easeInOut" }}
                className={cn(
                  "absolute left-1/2 top-1/2 z-10 grid size-[92px] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full border bg-[var(--panel)]",
                  props.live
                    ? "border-[var(--green-line)]"
                    : props.active
                      ? "border-[var(--brand-line)]"
                      : "border-line",
                )}
              >
                <span
                  className={cn(
                    "absolute inset-[7px] rounded-full border",
                    props.live
                      ? "border-[var(--green)]/25"
                      : props.active
                        ? "border-[var(--brand-strong)]/30"
                        : "border-white/[0.05]",
                  )}
                />
                <span className="relative font-display text-[25px] font-semibold text-txt">
                  {initial(self?.name ?? "You")}
                </span>
                <span className="absolute -bottom-6 left-1/2 w-[140px] -translate-x-1/2 truncate text-center text-[10.5px] text-txt3">
                  {self?.name ?? t("spacesThisPc")}
                </span>
              </motion.div>

              {peers.map((m, i) => {
                const p = orbit(i, peers.length)
                return (
                  <motion.div
                    key={m.id}
                    initial={calm ? false : { opacity: 0, scale: 0.86, y: 6 }}
                    animate={{ opacity: 1, scale: 1, y: 0 }}
                    transition={calm ? { duration: 0 } : { type: "spring", stiffness: 320, damping: 26, delay: 0.18 + i * 0.08 }}
                    className="absolute z-10 -translate-x-1/2 -translate-y-1/2"
                    style={{ left: `${p.x}%`, top: `${p.y}%` }}
                  >
                    <div
                      className={cn(
                        "flex w-[124px] flex-col items-center gap-1 text-center",
                        m.online ? "opacity-100" : "opacity-55",
                      )}
                    >
                      <span
                        className={cn(
                          "relative grid size-10 place-items-center rounded-full border bg-[var(--panel)] font-mono text-[12px]",
                          m.online ? "border-[var(--brand-line)] text-brand-strong" : "border-line text-txt3",
                        )}
                      >
                        {initial(m.name)}
                        <span
                          className={cn(
                            "absolute -right-0.5 -top-0.5 size-2 rounded-full border-2 border-[var(--panel)]",
                            m.online ? "bg-[var(--green)]" : "bg-txt3/40",
                          )}
                        />
                      </span>
                      <span className="max-w-full truncate text-[11.5px] leading-tight text-txt">{m.name}</span>
                      <span className="font-mono text-[9.5px] text-txt3">{m.address}</span>
                    </div>
                  </motion.div>
                )
              })}

              {peers.length === 0 ? (
                <div className="absolute inset-x-0 bottom-4 text-center text-[11px] uppercase tracking-[0.14em] text-txt3/70">
                  {t("spacesMembersEmpty")}
                </div>
              ) : null}
            </div>

            <LanPanel
              lan={props.lan}
              peers={props.lanPeers}
              busy={props.lanBusy}
              error={props.lanError}
              onConnect={props.onLanConnect}
              onStop={props.onLanStop}
            />
          </>
        )}
      </main>

      <aside className="flex min-h-0 flex-col overflow-hidden rounded-[18px] border border-line bg-[var(--panel)]/80 lg:col-span-2 xl:col-span-1">
        {props.space ? (
          <>
            {props.space.owner ? (
              <section className="border-b border-line p-4">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-[10.5px] uppercase tracking-[0.14em] text-txt3">{t("spacesInvite")}</span>
                  <button
                    type="button"
                    onClick={props.onRotate}
                    disabled={props.busy === "rotate"}
                    className="press flex items-center gap-1 text-[11px] text-brand-strong hover:underline disabled:opacity-50"
                  >
                    {props.busy === "rotate" ? (
                      <Loader2 className="size-3 animate-spin" aria-hidden />
                    ) : (
                      <RotateCw className="size-3" aria-hidden />
                    )}
                    {t("spacesRotate")}
                  </button>
                </div>
                <div className="mt-2 flex items-center gap-1.5">
                  <code className="min-w-0 flex-1 truncate font-mono text-[10.5px] text-txt2" dir="ltr">
                    {props.space.invite}
                  </code>
                  <button
                    type="button"
                    onClick={() => void copy(props.space!.invite, "invite")}
                    className="press grid size-7 shrink-0 place-items-center rounded-[6px] border border-line text-txt3 hover:text-txt"
                    aria-label={t("copy")}
                  >
                    {copied === "invite" ? <Check className="size-3" /> : <Copy className="size-3" />}
                  </button>
                </div>
                <p className="mt-1.5 text-[10.5px] leading-snug text-txt3">{t("spacesInviteHint")}</p>
              </section>
            ) : null}

            <section className="min-h-0 flex-1 overflow-y-auto border-b border-line p-4">
              <div className="flex items-center justify-between">
                <span className="text-[10.5px] uppercase tracking-[0.14em] text-txt3">{t("spacesMembers")}</span>
                <span className="font-mono text-[10.5px] text-txt3">{props.members.length}</span>
              </div>
              <div className="mt-2 grid gap-1">
                {props.members.map((m) => (
                  <div
                    key={m.id}
                    className={cn(
                      "group flex items-center gap-2.5 rounded-[8px] px-1.5 py-1.5",
                      m.self_pc ? "bg-white/[0.035]" : "hover:bg-white/[0.02]",
                    )}
                  >
                    <span
                      className={cn(
                        "relative grid size-7 shrink-0 place-items-center rounded-full border font-mono text-[10px]",
                        m.online ? "border-[var(--brand-line)] text-brand-strong" : "border-line text-txt3",
                      )}
                    >
                      {initial(m.name)}
                      <span
                        className={cn(
                          "absolute -right-0.5 -top-0.5 size-1.5 rounded-full border border-[var(--panel)]",
                          m.online ? "bg-[var(--green)]" : "bg-txt3/40",
                        )}
                      />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex min-w-0 items-center gap-1.5">
                        <span className="truncate text-[12px] text-txt">{m.name}</span>
                        {m.self_pc ? (
                          <span className="shrink-0 text-[9px] uppercase tracking-[0.08em] text-txt3">
                            {t("spacesThisPc")}
                          </span>
                        ) : null}
                        {m.owner ? <Crown className="size-2.5 shrink-0 text-[var(--amber)]" aria-hidden /> : null}
                      </span>
                      <span className="block font-mono text-[9.5px] text-txt3">{m.address}</span>
                    </span>
                    {props.space?.owner && !m.self_pc ? (
                      <button
                        type="button"
                        onClick={() => props.onKick(m.id, m.name)}
                        aria-label={t("spacesKick")}
                        title={t("spacesKick")}
                        className="press grid size-6 shrink-0 place-items-center rounded text-txt3 opacity-0 transition-opacity hover:text-[var(--red)] group-hover:opacity-100"
                      >
                        <UserMinus className="size-3" aria-hidden />
                      </button>
                    ) : null}
                  </div>
                ))}
              </div>
            </section>

            <div className="p-3">
              <button
                type="button"
                onClick={props.onRemove}
                className="press flex w-full items-center justify-center gap-1.5 rounded-[7px] py-2 text-[11.5px] text-txt3 transition-colors hover:bg-[var(--red)]/[0.06] hover:text-[var(--red)]"
              >
                <Trash2 className="size-3.5" aria-hidden />
                {props.space.owner ? t("spacesDelete") : t("spacesLeave")}
              </button>
            </div>
          </>
        ) : (
          <div className="grid flex-1 place-items-center p-6 text-center text-[11.5px] text-txt3">
            <WifiOff className="size-5" aria-hidden />
          </div>
        )}
      </aside>
    </div>
  )
}
