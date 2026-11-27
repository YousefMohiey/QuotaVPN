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
  ShieldCheck,
  Trash2,
  X,
} from "@/components/icons"
import { useI18n } from "@/lib/i18n"
import { copyText } from "@/lib/clipboard"
import type { LanSpace, OpenRadPeer, OpenRadStatus } from "@/lib/ipc"
import { cn } from "@/lib/utils"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"

type Props = {
  spaces: LanSpace[]
  selected: LanSpace | null
  lan: OpenRadStatus | null
  peers: OpenRadPeer[]
  busy: boolean
  error: string
  onSelect: (id: string) => void
  onConnect: () => void
  onStop: () => void
  onCreate: (name: string) => Promise<void>
  onJoin: (invite: string) => Promise<void>
  onRemove: (space: LanSpace) => Promise<void>
  onErrorClear: () => void
}

function initial(name: string): string {
  return (name.trim()[0] ?? "?").toUpperCase()
}

function Chip({ children, mono = false }: { children: React.ReactNode; mono?: boolean }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 rounded-lg border border-line bg-[var(--surface-2)] px-3 py-2 text-xs text-txt2",
        mono && "font-mono",
      )}
    >
      {children}
    </span>
  )
}

function DialogHead({ title, body }: { title: string; body: string }) {
  return (
    <div className="border-b border-line bg-[var(--surface-2)] px-6 py-5">
      <DialogTitle className="text-base font-semibold tracking-[-0.01em]">{title}</DialogTitle>
      <p className="mt-1 text-xs leading-relaxed text-txt2">{body}</p>
    </div>
  )
}

function PeerRow({ peer, self }: { peer: OpenRadPeer; self?: boolean }) {
  const { t } = useI18n()
  // The engine names the direct paths DirectUdp / DirectTcp and the fallback
  // Relay. Classify by the word, not by one exact string, so a new transport
  // name can never make a relayed peer wear the direct badge.
  const relayed = (peer.transport ?? "").toLowerCase().includes("relay")
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
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm text-txt">{peer.name}</span>
          {self ? (
            <span className="shrink-0 rounded bg-[var(--brand-bg)] px-2 py-0.5 text-xs font-medium text-brand-strong">
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
              "rounded-md px-2 py-0.5 text-xs font-medium uppercase tracking-[0.08em]",
              relayed
                ? "bg-[var(--amber-bg)] text-[var(--amber)]"
                : "bg-[var(--green-bg)] text-[var(--green)]",
            )}
          >
            {relayed ? t("lanRelay") : t("lanDirect")}
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
  space,
  lan,
  peers,
  busy,
  error,
  onConnect,
  onStop,
}: {
  space: LanSpace | null
  lan: OpenRadStatus | null
  peers: OpenRadPeer[]
  busy: boolean
  error: string
  onConnect: () => void
  onStop: () => void
}) {
  const { t } = useI18n()
  const calm = useReducedMotion()
  const joinedName = lan?.snapshot?.networks?.[0]?.name ?? ""
  const sameNetwork = !space || joinedName === space.network
  const on = lan?.phase === "connected" && sameNetwork
  const switching = lan?.phase === "connected" && !!space && joinedName !== space.network
  const ready = on && peers.length > 0
  const vip = lan?.snapshot?.vip ?? ""
  const device = lan?.node_name ?? ""
  const word = switching
    ? t("lanSpaceSwitching")
    : on
      ? peers.length > 0
        ? t("lanReady")
        : t("lanWaitPeer")
      : busy
        ? t("lanConnectingShort")
        : error
          ? t("lanFailed")
          : t("lanOff")
  const hint = switching
    ? t("lanSpaceSwitchingHint")
    : on
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
                  : busy || switching
                    ? "bg-[var(--amber)]"
                    : on
                      ? "bg-[var(--brand-strong)]"
                      : "bg-txt3/50",
              )}
            />
            {t("lanTitle")}
            {space ? (
              <span className="font-medium normal-case tracking-normal text-txt2">{space.name}</span>
            ) : null}
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
                  : on || switching
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

          <div className="mt-6 flex flex-wrap items-center gap-3">
            {lan?.phase === "connected" ? (
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
                disabled={busy || !space}
                className="press flex h-10 items-center gap-2 whitespace-nowrap rounded-xl bg-[var(--brand)] px-4 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)] disabled:opacity-50"
              >
                {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <Cable className="size-4" aria-hidden />}
                {busy ? t("lanConnecting") : t("lanConnect")}
              </button>
            )}
          </div>
          {on ? (
            <p className="mt-2 flex items-center gap-2 text-xs text-txt3">
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
                  <div className="m-1 grid place-items-center rounded-lg border border-dashed border-line px-4 py-6 text-center">
                    <Monitor className="size-5 text-txt3" aria-hidden />
                    <p className="mt-2 text-xs leading-relaxed text-txt3">{t("lanNoPeers")}</p>
                  </div>
                ) : null}
              </div>
            ) : (
              <div className="m-1 grid place-items-center rounded-lg border border-dashed border-line px-4 py-6 text-center">
                {switching ? (
                  <Loader2 className="size-5 animate-spin text-txt3" aria-hidden />
                ) : (
                  <Cable className="size-5 text-txt3" aria-hidden />
                )}
                <p className="mt-2 text-xs leading-relaxed text-txt3">{t("lanPeersOff")}</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  )
}

function NewSpaceDialog({
  open,
  onOpenChange,
  onCreate,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreate: (name: string) => Promise<void>
}) {
  const { t } = useI18n()
  const [name, setName] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  const submit = async () => {
    if (!name.trim()) {
      setError(t("lanSpaceNeedName"))
      return
    }
    setBusy(true)
    setError("")
    try {
      await onCreate(name.trim())
      setName("")
      onOpenChange(false)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[440px] overflow-hidden p-0">
        <DialogHead title={t("lanNewSpace")} body={t("lanSpaceCreateBody")} />
        <div className="px-6 py-5">
          <label className="grid gap-2">
            <span className="text-xs uppercase tracking-[0.12em] text-txt3">{t("lanSpaceName")}</span>
            <Input
              autoFocus
              value={name}
              onChange={(e) => {
                setError("")
                setName(e.target.value)
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") void submit()
              }}
              maxLength={40}
              dir="auto"
              placeholder={t("lanSpacePlaceholder")}
              className="h-9"
            />
          </label>
          <p className="mt-2 text-xs leading-relaxed text-txt3">{t("lanSpaceNameHint")}</p>
          {error ? <p className="mt-2 text-xs leading-snug text-[var(--red)]">{error}</p> : null}
        </div>
        <div className="flex justify-end gap-2 border-t border-line px-6 py-3">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-lg border border-line px-4 text-sm text-txt2 hover:text-txt"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void submit()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-lg bg-[var(--brand)] px-4 text-sm font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <Plus className="size-3.5" aria-hidden />}
            {busy ? t("lanSpaceCreating") : t("lanSpaceCreate")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function JoinSpaceDialog({
  open,
  onOpenChange,
  onJoin,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onJoin: (invite: string) => Promise<void>
}) {
  const { t } = useI18n()
  const [invite, setInvite] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  const submit = async () => {
    if (!invite.trim()) {
      setError(t("lanSpaceBadInvite"))
      return
    }
    setBusy(true)
    setError("")
    try {
      await onJoin(invite.trim())
      setInvite("")
      onOpenChange(false)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[520px] overflow-hidden p-0">
        <DialogHead title={t("lanJoinSpace")} body={t("lanSpaceJoinBody")} />
        <div className="px-6 py-5">
          <label className="grid gap-2">
            <span className="text-xs uppercase tracking-[0.12em] text-txt3">{t("lanSpaceInvite")}</span>
            <Input
              autoFocus
              value={invite}
              onChange={(e) => {
                setError("")
                setInvite(e.target.value)
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") void submit()
              }}
              maxLength={512}
              dir="ltr"
              spellCheck={false}
              className="h-9 font-mono text-xs"
            />
          </label>
          {error ? <p className="mt-2 text-xs leading-snug text-[var(--red)]">{error}</p> : null}
        </div>
        <div className="flex justify-end gap-2 border-t border-line px-6 py-3">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-lg border border-line px-4 text-sm text-txt2 hover:text-txt"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void submit()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-lg bg-[var(--brand)] px-4 text-sm font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <LogIn className="size-3.5" aria-hidden />}
            {busy ? t("lanSpaceJoining") : t("lanSpaceJoinAction")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function RemoveSpaceDialog({
  space,
  onClose,
  onConfirm,
}: {
  space: LanSpace | null
  onClose: () => void
  onConfirm: (space: LanSpace) => Promise<void>
}) {
  const { t } = useI18n()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  const title = t("lanSpaceRemoveTitle")
  const body = space?.owner ? t("lanSpaceDeleteBody") : t("lanSpaceLeaveBody")

  const go = async () => {
    if (!space) return
    setBusy(true)
    setError("")
    try {
      await onConfirm(space)
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={!!space} onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent className="max-w-[400px] p-6">
        <DialogTitle className="text-base font-semibold tracking-[-0.01em]">{title}</DialogTitle>
        <p className="text-sm leading-relaxed text-txt2">{body}</p>
        {space ? (
          <p className="truncate font-mono text-xs text-txt3">{space.name}</p>
        ) : null}
        {error ? <p className="text-xs leading-snug text-[var(--red)]">{error}</p> : null}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="press h-9 rounded-lg border border-line px-4 text-sm text-txt2 hover:text-txt"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void go()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-lg bg-[var(--red)] px-4 text-sm font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <Trash2 className="size-3.5" aria-hidden />}
            {busy ? t("lanSpaceDeleting") : space?.owner ? t("lanSpaceDelete") : t("lanSpaceLeave")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export default function SpacesConsole(props: Props) {
  const { t } = useI18n()
  const [newOpen, setNewOpen] = useState(false)
  const [joinOpen, setJoinOpen] = useState(false)
  const [remove, setRemove] = useState<LanSpace | null>(null)
  const [copied, setCopied] = useState(false)

  const selected = props.selected
  const joinedName = props.lan?.snapshot?.networks?.[0]?.name ?? ""

  const copyInvite = async () => {
    if (!selected) return
    if (!(await copyText(selected.invite))) return
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  return (
    <div className="flex w-full flex-col gap-6 pb-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="mr-1 text-xs font-semibold uppercase tracking-[0.16em] text-txt3">
            {t("spacesTitle")}
          </span>
          {props.spaces.map((space) => {
            const isSelected = space.id === selected?.id
            const isLive = props.lan?.phase === "connected" && joinedName === space.network
            return (
              <button
                key={space.id}
                type="button"
                onClick={() => props.onSelect(space.id)}
                aria-pressed={isSelected}
                className={cn(
                  "group flex h-9 max-w-[220px] items-center gap-2 rounded-full border px-3 text-sm transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)]",
                  isSelected
                    ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-txt"
                    : "border-line bg-transparent text-txt2 hover:bg-[var(--surface-2)] hover:text-txt",
                )}
              >
                <span
                  className={cn(
                    "size-1.5 shrink-0 rounded-full",
                    isLive ? "bg-[var(--green)]" : "bg-txt3/40",
                  )}
                />
                <span className="truncate">{space.name}</span>
                {space.owner ? <Crown className="size-3 shrink-0 text-[var(--amber)]" aria-hidden /> : null}
              </button>
            )
          })}
          <button
            type="button"
            onClick={() => setNewOpen(true)}
            className="flex h-9 items-center gap-2 rounded-full border border-line px-3 text-sm text-txt2 transition-colors duration-200 hover:bg-[var(--surface-2)] hover:text-txt focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)]"
          >
            <Plus className="size-4" aria-hidden />
            {t("lanNewSpace")}
          </button>
          <button
            type="button"
            onClick={() => setJoinOpen(true)}
            className="flex h-9 items-center gap-2 rounded-full bg-[var(--brand)] px-3 text-sm font-semibold text-[var(--on-brand)] transition-colors duration-200 hover:bg-[var(--brand-vivid)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-line)]"
          >
            <LogIn className="size-4" aria-hidden />
            {t("lanJoinSpace")}
          </button>
        </div>
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

      {!selected ? (
        <section className="grid place-items-center rounded-2xl border border-line bg-[var(--panel)] px-8 py-16 text-center">
          <p className="text-sm text-txt2">{t("lanSpaceNoSpaces")}</p>
          <button
            type="button"
            onClick={() => setNewOpen(true)}
            className="mt-4 flex h-10 items-center gap-2 rounded-xl bg-[var(--brand)] px-4 text-sm font-semibold text-[var(--on-brand)]"
          >
            <Plus className="size-4" aria-hidden />
            {t("lanNewSpace")}
          </button>
        </section>
      ) : (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
          <LanPanel
            space={selected}
            lan={props.lan}
            peers={joinedName === selected.network ? props.peers : []}
            busy={props.busy}
            error={props.error}
            onConnect={props.onConnect}
            onStop={props.onStop}
          />
          <aside className="grid content-start gap-4">
            <section className="rounded-2xl border border-line bg-[var(--panel)] p-4 shadow-[var(--sh-1)]">
              <div className="flex items-center justify-between gap-2">
                <h2 className="text-sm font-semibold text-txt">{t("lanSpaceInvite")}</h2>
                <button
                  type="button"
                  onClick={() => void copyInvite()}
                  aria-label={copied ? t("copied") : t("copy")}
                  className="grid size-7 place-items-center rounded-md text-txt3 transition-colors hover:bg-[var(--surface-2)] hover:text-txt"
                >
                  {copied ? <Check className="size-4" aria-hidden /> : <Copy className="size-4" aria-hidden />}
                </button>
              </div>
              <code className="mt-2 block max-h-20 overflow-y-auto break-all rounded-lg border border-line bg-[var(--surface-2)] px-3 py-2 font-mono text-xs leading-relaxed text-txt2" dir="ltr">
                {selected.invite}
              </code>
              <p className="mt-2 text-xs leading-relaxed text-txt3">{t("lanSpaceInviteHint")}</p>
              <div className="mt-3 flex items-center gap-2 text-xs text-txt3">
                <span className="size-1.5 rounded-full bg-[var(--brand-strong)]" />
                {selected.owner ? t("spacesOwner") : t("lanSpaceMember")}
              </div>
            </section>

            <section className="rounded-2xl border border-line bg-[var(--panel)] p-4 shadow-[var(--sh-1)]">
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

            <section className="rounded-2xl border border-line bg-[var(--panel)] p-4">
              <p className="text-xs leading-relaxed text-txt3">{t("lanSpaceOnly")}</p>
              {!selected.default ? (
                <button
                  type="button"
                  onClick={() => setRemove(selected)}
                  className="mt-4 flex w-full items-center justify-center gap-2 rounded-xl border border-line px-3 py-3 text-sm text-txt3 transition-colors duration-200 hover:border-[var(--red-line)] hover:bg-[var(--red-bg)] hover:text-[var(--red)]"
                >
                  <Trash2 className="size-4" aria-hidden />
                  {selected.owner ? t("lanSpaceDelete") : t("lanSpaceLeave")}
                </button>
              ) : null}
            </section>
          </aside>
        </div>
      )}

      <NewSpaceDialog open={newOpen} onOpenChange={setNewOpen} onCreate={props.onCreate} />
      <JoinSpaceDialog open={joinOpen} onOpenChange={setJoinOpen} onJoin={props.onJoin} />
      <RemoveSpaceDialog space={remove} onClose={() => setRemove(null)} onConfirm={props.onRemove} />
    </div>
  )
}
