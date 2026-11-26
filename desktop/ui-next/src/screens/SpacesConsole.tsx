import { useState } from "react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import {
  Cable,
  Check,
  Copy,
  Crown,
  Gamepad2,
  Loader2,
  LogIn,
  Monitor,
  Plus,
  Power,
  RefreshCw,
  RotateCw,
  Server,
  Settings2,
  ShieldCheck,
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

function StatusChip({ tone, children }: { tone: "live" | "ready" | "off"; children: React.ReactNode }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium",
        tone === "live"
          ? "border-[var(--green-line)] bg-[var(--green-bg)] text-[var(--green)]"
          : tone === "ready"
            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
            : "border-line bg-[var(--surface-2)] text-txt3",
      )}
    >
      <span
        className={cn(
          "size-1.5 rounded-full",
          tone === "live" ? "bg-[var(--green)]" : tone === "ready" ? "bg-[var(--brand-strong)]" : "bg-txt3/50",
        )}
      />
      {children}
    </span>
  )
}

function IconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string
  onClick: () => void
  disabled?: boolean
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
      className="grid size-9 place-items-center rounded-xl border border-line bg-[var(--surface-2)] text-txt2 transition-colors duration-200 hover:border-[var(--brand-line)] hover:text-txt focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)] disabled:opacity-40"
    >
      {children}
    </button>
  )
}

function Chip({
  children,
  mono,
}: {
  children: React.ReactNode
  mono?: boolean
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-lg border border-line bg-[var(--surface-2)] px-2.5 py-1.5 text-xs text-txt2",
        mono && "font-mono",
      )}
    >
      {children}
    </span>
  )
}

function PeerRow({ peer, self }: { peer: OpenRadPeer; self?: boolean }) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3 rounded-xl px-2 py-2 transition-colors duration-200 hover:bg-[var(--panel-hover)]">
      <span
        className={cn(
          "grid size-9 shrink-0 place-items-center rounded-full border font-mono text-xs",
          self
            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
            : "border-line bg-[var(--panel)] text-txt2",
        )}
      >
        {initial(peer.name)}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-1.5">
          <span className="truncate text-sm text-txt">{peer.name}</span>
          {self ? (
            <span className="shrink-0 rounded bg-[var(--brand-bg)] px-1.5 py-0.5 text-xs font-medium text-brand-strong">
              {t("spacesThisPc")}
            </span>
          ) : null}
        </div>
        <div className="font-mono text-xs text-txt3">{peer.vip}</div>
      </div>
      {!self ? (
        <div className="flex shrink-0 flex-col items-end gap-1">
          <span
            className={cn(
              "rounded-md px-1.5 py-0.5 text-xs font-medium uppercase tracking-[0.08em]",
              peer.transport === "Relay"
                ? "bg-[var(--amber-bg)] text-[var(--amber)]"
                : "bg-[var(--green-bg)] text-[var(--green)]",
            )}
          >
            {peer.transport === "Relay" ? t("lanRelay") : t("lanDirect")}
          </span>
          {peer.rtt_ms != null ? (
            <span className="font-mono text-xs text-txt3">{Math.round(peer.rtt_ms)} ms</span>
          ) : null}
        </div>
      ) : null}
    </div>
  )
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
  const ready = on && peers.length > 0
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
      : error
        ? t("lanFailedHint")
        : t("lanOffHint")
  const selfPeer: OpenRadPeer | null = on
    ? {
        rid: "self",
        name: device || t("spacesThisPc"),
        vip,
        connected: true,
        transport: null,
        rtt_ms: null,
      }
    : null

  return (
    <section className="overflow-hidden rounded-2xl border border-line bg-[var(--panel)] shadow-[var(--sh-1)]">
      <div className="flex flex-col gap-6 p-6 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-xs font-semibold uppercase tracking-[0.16em] text-txt3">
            <span
              className={cn(
                "size-1.5 rounded-full",
                ready
                  ? "bg-[var(--green)]"
                  : busy
                    ? "bg-[var(--amber)]"
                    : on
                      ? "bg-[var(--brand-strong)]"
                      : "bg-txt3/50",
              )}
            />
            {t("lanTitle")}
            {device ? (
              <span className="font-medium normal-case tracking-normal text-txt3/80">{device}</span>
            ) : null}
            {vip ? (
              <span className="font-mono font-normal normal-case tracking-normal text-txt3/80">{vip}</span>
            ) : null}
          </div>

          <AnimatePresence mode="wait" initial={false}>
            <motion.h2
              key={word}
              initial={calm ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={calm ? undefined : { opacity: 0, y: -6 }}
              transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
              className={cn(
                "mt-3 font-display text-3xl font-semibold leading-tight tracking-[-0.03em]",
                ready
                  ? "text-[var(--green)]"
                  : on
                    ? "text-brand-strong"
                    : error
                      ? "text-[var(--red)]"
                      : "text-txt",
              )}
            >
              {word}
            </motion.h2>
          </AnimatePresence>

          <p className="mt-2 max-w-[46ch] text-sm leading-relaxed text-txt2">{hint}</p>
          {error ? (
            <p className="mt-2 max-w-[46ch] break-words text-sm leading-relaxed text-[var(--red)]">{error}</p>
          ) : null}

          {on ? (
            <div className="mt-4 flex flex-wrap items-center gap-2">
              <Chip>{device || t("spacesThisPc")}</Chip>
              <Chip mono>{vip}</Chip>
            </div>
          ) : null}

          <div className="mt-5 flex flex-wrap items-center gap-3">
            {on ? (
              <button
                type="button"
                onClick={onStop}
                disabled={busy}
                className="press flex h-10 items-center gap-2 whitespace-nowrap rounded-xl border border-line bg-[var(--surface-2)] px-4 text-sm font-medium text-txt transition-colors duration-200 hover:border-[var(--red-line)] hover:text-[var(--red)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)] disabled:opacity-50"
              >
                {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <Power className="size-4" aria-hidden />}
                {t("lanStop")}
              </button>
            ) : (
              <button
                type="button"
                onClick={onConnect}
                disabled={busy}
                className="press flex h-10 items-center gap-2 whitespace-nowrap rounded-xl bg-[var(--brand)] px-4 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)] disabled:opacity-50"
              >
                {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <Cable className="size-4" aria-hidden />}
                {busy ? t("lanConnecting") : t("lanConnect")}
              </button>
            )}
          </div>
          {on ? (
            <p className="mt-2 flex items-center gap-1.5 text-xs text-txt3">
              <ShieldCheck className="size-3.5" aria-hidden />
              {t("lanTrust")}
            </p>
          ) : null}
        </div>

        <div className="w-full shrink-0 lg:w-[310px]">
          <div className="rounded-xl border border-line bg-[var(--surface-2)] p-3">
            <div className="flex items-center justify-between px-1 pb-2">
              <span className="text-xs font-medium text-txt3">{t("lanPeers")}</span>
              <span className="font-mono text-xs text-txt3">{on ? peers.length + 1 : 0}</span>
            </div>
            {on ? (
              <div className="grid gap-0.5">
                {selfPeer ? <PeerRow peer={selfPeer} self /> : null}
                {peers.map((peer) => (
                  <PeerRow key={peer.rid} peer={peer} />
                ))}
                {peers.length === 0 ? (
                  <div className="m-1 grid place-items-center rounded-lg border border-dashed border-line px-4 py-5 text-center">
                    <Monitor className="size-5 text-txt3" aria-hidden />
                    <p className="mt-2 text-xs leading-relaxed text-txt3">{t("lanNoPeers")}</p>
                  </div>
                ) : null}
              </div>
            ) : (
              <div className="grid place-items-center rounded-lg border border-dashed border-line px-4 py-8 text-center">
                <Cable className="size-5 text-txt3" aria-hidden />
                <p className="mt-2 max-w-[22ch] text-xs leading-relaxed text-txt3">{t("lanPeersOff")}</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  )
}

function MemberRow({
  member,
  owner,
  onKick,
}: {
  member: SpaceMember
  owner: boolean
  onKick: (memberId: string, name: string) => void
}) {
  const { t } = useI18n()
  return (
    <div className="group flex items-center gap-2.5 rounded-lg px-1.5 py-1.5 transition-colors duration-200 hover:bg-[var(--panel-hover)]">
      <span
        className={cn(
          "relative grid size-8 shrink-0 place-items-center rounded-full border font-mono text-xs",
          member.online
            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
            : "border-line bg-[var(--surface-2)] text-txt3",
        )}
      >
        {initial(member.name)}
        <span
          className={cn(
            "absolute -right-0.5 -top-0.5 size-1.5 rounded-full border-2 border-[var(--panel)]",
            member.online ? "bg-[var(--green)]" : "bg-txt3/40",
          )}
        />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-1.5">
          <span className="truncate text-sm text-txt">{member.name}</span>
          {member.self_pc ? (
            <span className="shrink-0 rounded bg-[var(--surface-3)] px-1.5 py-0.5 text-xs text-txt3">
              {t("spacesThisPc")}
            </span>
          ) : null}
          {member.owner ? <Crown className="size-3 shrink-0 text-[var(--amber)]" aria-hidden /> : null}
        </div>
        <div className="font-mono text-xs text-txt3">{member.address}</div>
      </div>
      {owner && !member.self_pc ? (
        <button
          type="button"
          onClick={() => onKick(member.id, member.name)}
          aria-label={t("spacesKick")}
          title={t("spacesKick")}
          className="grid size-7 shrink-0 place-items-center rounded-lg text-txt3 opacity-0 transition-opacity duration-200 hover:bg-[var(--red-bg)] hover:text-[var(--red)] focus-visible:opacity-100 group-hover:opacity-100"
        >
          <UserMinus className="size-3.5" aria-hidden />
        </button>
      ) : null}
    </div>
  )
}

export default function SpacesConsole(props: SpacesConsoleProps) {
  const { t } = useI18n()
  const [copied, setCopied] = useState("")

  const copy = async (value: string, key: string) => {
    await copyText(value)
    setCopied(key)
    window.setTimeout(() => setCopied((cur) => (cur === key ? "" : cur)), 1600)
  }

  const online = props.members.filter((m) => m.online).length
  const lanOn = props.lan?.phase === "connected"
  const lanDevice = props.lan?.node_name ?? ""
  const lanVip = props.lan?.snapshot?.vip ?? ""
  const tone: "live" | "ready" | "off" = props.live ? "live" : props.active ? "ready" : "off"
  const statusWord = props.live ? t("spacesLive") : props.active ? t("spacesReady") : t("spacesOff")

  return (
    <div className="flex w-full flex-col gap-6 pb-6">
      {/* Space switcher. The primary rail already carries the app; this row
          carries the spaces themselves, so the page below gets the width. */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="mr-1 text-xs font-semibold uppercase tracking-[0.16em] text-txt3">
            {t("spacesTitle")}
          </span>
          {props.spaces.map((s) => {
            const isSelected = s.id === props.selected
            const isActive = s.id === props.activeId
            const liveCount = s.members.filter((m) => m.online).length
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => props.onSelect(s.id)}
                aria-pressed={isSelected}
                className={cn(
                  "group flex h-9 max-w-[210px] items-center gap-2 rounded-full border px-3 text-sm transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)]",
                  isSelected
                    ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-txt"
                    : "border-line bg-transparent text-txt2 hover:bg-[var(--surface-2)] hover:text-txt",
                )}
              >
                <span
                  className={cn(
                    "size-1.5 shrink-0 rounded-full",
                    liveCount > 0 ? "bg-[var(--green)]" : "bg-txt3/40",
                  )}
                />
                <span className="truncate">{s.name}</span>
                {isActive ? (
                  <span className="shrink-0 text-xs font-medium uppercase tracking-[0.08em] text-brand-strong">
                    {t("spacesActive")}
                  </span>
                ) : null}
              </button>
            )
          })}
          {props.spaces.length > 0 ? (
            <>
              <button
                type="button"
                onClick={props.onJoin}
                className="flex h-9 items-center gap-1.5 rounded-full border border-line px-3 text-sm text-txt2 transition-colors duration-200 hover:bg-[var(--surface-2)] hover:text-txt focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)]"
              >
                <LogIn className="size-4" aria-hidden />
                {t("spacesJoin")}
              </button>
              <button
                type="button"
                onClick={props.onCreate}
                className="flex h-9 items-center gap-1.5 rounded-full bg-[var(--brand)] px-3 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)]"
              >
                <Plus className="size-4" aria-hidden />
                {t("spacesCreate")}
              </button>
            </>
          ) : null}
        </div>
        <IconButton label={t("spacesCoordinator")} onClick={props.onCoordinator}>
          <Settings2 className="size-4" aria-hidden />
        </IconButton>
      </div>

      {props.error ? (
        <div className="flex items-start gap-3 rounded-xl border border-[var(--red-line)] bg-[var(--red-bg)] px-4 py-3 text-sm leading-relaxed text-[var(--red)]">
          <span className="min-w-0 flex-1">{props.error}</span>
          <button
            type="button"
            onClick={props.onErrorClear}
            aria-label={t("cancel")}
            className="shrink-0 rounded-md p-0.5 hover:bg-[var(--red-line)]/20"
          >
            <X className="size-4" aria-hidden />
          </button>
        </div>
      ) : null}

      {!props.space ? (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
          <section className="grid place-items-center rounded-2xl border border-line bg-[var(--panel)] px-8 py-16 text-center shadow-[var(--sh-1)]">
            <span className="grid size-14 place-items-center rounded-2xl border border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong">
              <Users className="size-6" aria-hidden />
            </span>
            <h2 className="mt-4 font-display text-2xl font-semibold tracking-[-0.02em] text-txt">
              {t("spacesEmpty")}
            </h2>
            <p className="mt-2 max-w-[42ch] text-sm leading-relaxed text-txt2">{t("spacesEmptyBody")}</p>
            <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
              <button
                type="button"
                onClick={props.onJoin}
                className="flex h-10 items-center gap-2 rounded-xl border border-line px-4 text-sm text-txt2 transition-colors duration-200 hover:bg-[var(--surface-2)] hover:text-txt"
              >
                <LogIn className="size-4" aria-hidden />
                {t("spacesJoin")}
              </button>
              <button
                type="button"
                onClick={props.onCreate}
                className="flex h-10 items-center gap-2 rounded-xl bg-[var(--brand)] px-4 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)]"
              >
                <Plus className="size-4" aria-hidden />
                {t("spacesCreate")}
              </button>
            </div>
          </section>
          <aside className="grid content-start gap-6">
            <section className="rounded-2xl border border-line bg-[var(--panel)] p-5">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-semibold text-txt">{t("lanTitle")}</h2>
                <span
                  className={cn(
                    "size-1.5 rounded-full",
                    lanOn ? "bg-[var(--green)]" : props.lanBusy ? "bg-[var(--amber)]" : "bg-txt3/40",
                  )}
                />
              </div>
              {lanOn ? (
                <>
                  <p className="mt-2 text-sm leading-relaxed text-txt2">{t("lanEmptyOn")}</p>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <Chip>{lanDevice || t("spacesThisPc")}</Chip>
                    <Chip mono>{lanVip}</Chip>
                  </div>
                  <button
                    type="button"
                    onClick={props.onLanStop}
                    disabled={props.lanBusy}
                    className="mt-4 flex h-10 w-full items-center justify-center gap-2 rounded-xl border border-line bg-[var(--surface-2)] px-4 text-sm font-medium text-txt transition-colors duration-200 hover:border-[var(--red-line)] hover:text-[var(--red)] disabled:opacity-50"
                  >
                    {props.lanBusy ? (
                      <Loader2 className="size-4 animate-spin" aria-hidden />
                    ) : (
                      <Power className="size-4" aria-hidden />
                    )}
                    {t("lanStop")}
                  </button>
                </>
              ) : (
                <>
                  <p className="mt-2 text-sm leading-relaxed text-txt2">{t("lanOffHint")}</p>
                  <button
                    type="button"
                    onClick={props.onLanConnect}
                    disabled={props.lanBusy}
                    className="mt-4 flex h-10 w-full items-center justify-center gap-2 rounded-xl bg-[var(--brand)] px-4 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)] disabled:opacity-50"
                  >
                    {props.lanBusy ? (
                      <Loader2 className="size-4 animate-spin" aria-hidden />
                    ) : (
                      <Cable className="size-4" aria-hidden />
                    )}
                    {props.lanBusy ? t("lanConnecting") : t("lanConnect")}
                  </button>
                </>
              )}
            </section>
            <section className="rounded-2xl border border-line bg-[var(--panel)] p-5">
              <h2 className="text-sm font-semibold text-txt">{t("spacesCoordinator")}</h2>
              <p className="mt-2 truncate font-mono text-xs text-txt3" dir="ltr">
                {props.apiBase || t("spacesCoordinatorNone")}
              </p>
              <button
                type="button"
                onClick={props.onCoordinator}
                className="mt-3 text-xs font-medium text-brand-strong hover:underline"
              >
                {t("spacesCoordinatorChange")}
              </button>
            </section>
          </aside>
        </div>
      ) : (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
          <div className="grid min-w-0 content-start gap-6">
            <LanPanel
              lan={props.lan}
              peers={props.lanPeers}
              busy={props.lanBusy}
              error={props.lanError}
              onConnect={props.onLanConnect}
              onStop={props.onLanStop}
            />

            <section className="rounded-2xl border border-line bg-[var(--panel)] p-6 shadow-[var(--sh-1)]">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="font-display text-2xl font-semibold tracking-[-0.02em] text-txt">
                      {props.space.name}
                    </h2>
                    {props.space.owner ? (
                      <span className="inline-flex items-center gap-1 rounded-full border border-[var(--amber-line)] bg-[var(--amber-bg)] px-2 py-0.5 text-xs font-medium uppercase tracking-[0.08em] text-[var(--amber)]">
                        <Crown className="size-3" aria-hidden />
                        {t("spacesOwner")}
                      </span>
                    ) : null}
                    <StatusChip tone={tone}>{statusWord}</StatusChip>
                  </div>
                  <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-sm text-txt3">
                    <span className="flex items-center gap-1 font-mono text-txt2">
                      {props.space.address}
                      <button
                        type="button"
                        onClick={() => void copy(props.space!.address, "address")}
                        className="grid size-6 place-items-center rounded-md text-txt3 transition-colors hover:bg-[var(--surface-2)] hover:text-txt"
                        aria-label={t("copy")}
                      >
                        {copied === "address" ? <Check className="size-3" /> : <Copy className="size-3" />}
                      </button>
                    </span>
                    <span>
                      {online} {t("spacesOnline")}
                    </span>
                    <span className="font-mono">{props.space.subnet}</span>
                    {props.note ? <span className="text-[var(--amber)]">{props.note}</span> : null}
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {!props.active ? (
                    <button
                      type="button"
                      onClick={props.onActivate}
                      disabled={props.busy === "activate"}
                      className="flex h-9 items-center gap-2 rounded-xl bg-[var(--brand)] px-3.5 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)] disabled:opacity-50"
                    >
                      {props.busy === "activate" ? (
                        <Loader2 className="size-4 animate-spin" aria-hidden />
                      ) : null}
                      {t("spacesConnect")}
                    </button>
                  ) : null}
                  <IconButton
                    label={t("refresh")}
                    onClick={props.onRefresh}
                    disabled={props.busy === "refresh"}
                  >
                    {props.busy === "refresh" ? (
                      <Loader2 className="size-4 animate-spin" aria-hidden />
                    ) : (
                      <RefreshCw className="size-4" aria-hidden />
                    )}
                  </IconButton>
                </div>
              </div>

              <div className="mt-6 grid gap-4 lg:grid-cols-2">
                <div className="rounded-xl border border-line bg-[var(--surface-2)] p-4">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-medium text-txt3">{t("spacesInvite")}</span>
                    <button
                      type="button"
                      onClick={props.onRotate}
                      disabled={props.busy === "rotate"}
                      className="flex items-center gap-1.5 text-xs font-medium text-brand-strong transition-colors hover:text-txt disabled:opacity-50"
                    >
                      {props.busy === "rotate" ? (
                        <Loader2 className="size-3.5 animate-spin" aria-hidden />
                      ) : (
                        <RotateCw className="size-3.5" aria-hidden />
                      )}
                      {t("spacesRotate")}
                    </button>
                  </div>
                  <div className="mt-3 flex items-center gap-2">
                    <code className="min-w-0 flex-1 truncate font-mono text-xs text-txt2" dir="ltr">
                      {props.space.invite}
                    </code>
                    <IconButton
                      label={t("copy")}
                      onClick={() => void copy(props.space!.invite, "invite")}
                    >
                      {copied === "invite" ? (
                        <Check className="size-4" aria-hidden />
                      ) : (
                        <Copy className="size-4" aria-hidden />
                      )}
                    </IconButton>
                  </div>
                  <p className="mt-2 text-xs leading-relaxed text-txt3">{t("spacesInviteHint")}</p>
                </div>

                <div className="rounded-xl border border-line bg-[var(--surface-2)] p-4">
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-medium text-txt3">{t("spacesMembers")}</span>
                    <span className="font-mono text-xs text-txt3">{props.members.length}</span>
                  </div>
                  <div className="mt-2 grid gap-0.5">
                    {props.members.map((member) => (
                      <MemberRow
                        key={member.id}
                        member={member}
                        owner={props.space?.owner ?? false}
                        onKick={props.onKick}
                      />
                    ))}
                  </div>
                </div>
              </div>
            </section>
          </div>

          <aside className="grid content-start gap-6">
            <section className="rounded-2xl border border-line bg-[var(--panel)] p-5 shadow-[var(--sh-1)]">
              <div className="flex items-center gap-2">
                <Gamepad2 className="size-4 text-brand-strong" aria-hidden />
                <h2 className="text-sm font-semibold text-txt">{t("lanPlayTitle")}</h2>
              </div>
              <ol className="mt-4 grid gap-4">
                {[
                  { title: t("lanStepOne"), body: t("lanStepOneBody") },
                  { title: t("lanStepTwo"), body: t("lanStepTwoBody") },
                  { title: t("lanStepThree"), body: t("lanStepThreeBody") },
                ].map((step, index) => (
                  <li key={step.title} className="flex gap-3">
                    <span className="grid size-6 shrink-0 place-items-center rounded-full bg-[var(--brand-bg)] font-mono text-xs font-semibold text-brand-strong">
                      {index + 1}
                    </span>
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-txt">{step.title}</p>
                      <p className="mt-0.5 text-xs leading-relaxed text-txt3">{step.body}</p>
                    </div>
                  </li>
                ))}
              </ol>
              <p className="mt-4 border-t border-line pt-4 text-xs leading-relaxed text-txt3">
                {t("lanPlayGames")}
              </p>
            </section>

            <section className="rounded-2xl border border-line bg-[var(--panel)] p-5">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Server className="size-4 text-txt3" aria-hidden />
                  <h2 className="text-sm font-semibold text-txt">{t("spacesCoordinator")}</h2>
                </div>
                <button
                  type="button"
                  onClick={props.onCoordinator}
                  aria-label={t("spacesCoordinatorChange")}
                  title={t("spacesCoordinatorChange")}
                  className="rounded-md p-1 text-txt3 transition-colors hover:bg-[var(--surface-2)] hover:text-txt"
                >
                  <Settings2 className="size-4" aria-hidden />
                </button>
              </div>
              <p className="mt-2 truncate font-mono text-xs text-txt3" dir="ltr">
                {props.apiBase || t("spacesCoordinatorNone")}
              </p>
              <div className="mt-3 flex items-center gap-2 text-xs text-txt3">
                <span className={cn("size-1.5 rounded-full", tone === "off" ? "bg-txt3/40" : "bg-[var(--green)]")} />
                {props.live ? t("lanCoordinatorLive") : props.active ? t("spacesReady") : t("spacesOff")}
              </div>
            </section>

            <section className="rounded-2xl border border-line bg-[var(--panel)] p-5">
              <div className="flex items-center gap-2 text-xs leading-relaxed text-txt3">
                <WifiOff className="size-4 shrink-0" aria-hidden />
                {t("spacesLocalNote")}
              </div>
              <button
                type="button"
                onClick={props.onRemove}
                className="mt-4 flex w-full items-center justify-center gap-2 rounded-xl border border-line px-3 py-2.5 text-sm text-txt3 transition-colors duration-200 hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)]"
              >
                <Trash2 className="size-4" aria-hidden />
                {props.space.owner ? t("spacesDelete") : t("spacesLeave")}
              </button>
            </section>
          </aside>
        </div>
      )}
    </div>
  )
}
