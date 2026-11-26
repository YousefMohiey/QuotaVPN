import { useCallback, useEffect, useMemo, useState } from "react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import {
  Cable,
  Check,
  ChevronRight,
  Copy,
  Crown,
  Link2,
  Loader2,
  LockKeyhole,
  LogIn,
  LogOut,
  Monitor,
  Plus,
  Radar,
  RefreshCw,
  RotateCw,
  Trash2,
  UserMinus,
  Users,
  Wifi,
  WifiOff,
  X,
} from "lucide-react"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { api, type DirectStatus, type SpaceMember, type SpaceProbe, type SpacesState } from "@/lib/ipc"
import { copyText } from "@/lib/clipboard"
import { pushCloser } from "@/lib/navStack"
import { cn } from "@/lib/utils"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"

const DEVICE_KEY = "qc-space-device"

const messageOf = (e: unknown): string =>
  typeof e === "string" ? e : e instanceof Error ? e.message : String(e)

function readDevice(): string {
  try {
    return localStorage.getItem(DEVICE_KEY) ?? ""
  } catch {
    return ""
  }
}

function rememberDevice(name: string) {
  try {
    localStorage.setItem(DEVICE_KEY, name)
  } catch {
    /* private mode */
  }
}

function CopyButton({ value, label }: { value: string; label: string }) {
  const { t } = useI18n()
  const [done, setDone] = useState(false)
  if (!value) return null
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={async (e) => {
        e.stopPropagation()
        if (!(await copyText(value))) return
        setDone(true)
        window.setTimeout(() => setDone(false), 1200)
      }}
      className="press grid size-7 shrink-0 place-items-center rounded-[7px] text-txt3 transition-colors hover:bg-white/[0.06] hover:text-txt"
    >
      {done ? (
        <Check className="size-3.5 text-[var(--green)]" aria-hidden />
      ) : (
        <Copy className="size-3.5" aria-hidden />
      )}
      <span className="sr-only">{done ? t("copied") : label}</span>
    </button>
  )
}

function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <label className="grid gap-1.5">
      <span className="text-[11px] uppercase tracking-[0.12em] text-txt3">{label}</span>
      {children}
      {hint ? <span className="text-[12px] leading-snug text-txt3">{hint}</span> : null}
    </label>
  )
}

function DialogHead({
  icon: Icon,
  title,
  body,
}: {
  icon: React.ComponentType<{ className?: string; "aria-hidden"?: boolean }>
  title: string
  body: string
}) {
  return (
    <div className="relative border-b border-line bg-[linear-gradient(135deg,rgba(101,145,255,0.12),transparent_62%)] px-5 py-5">
      <div className="flex items-center gap-3">
        <span className="grid size-11 place-items-center rounded-[12px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
          <Icon className="size-5 text-brand-strong" aria-hidden />
        </span>
        <div className="min-w-0">
          <DialogTitle className="text-[17px] font-semibold tracking-[-0.01em]">{title}</DialogTitle>
          <p className="mt-1 text-[12.5px] leading-relaxed text-txt2">{body}</p>
        </div>
      </div>
    </div>
  )
}

function CreateDialog({
  open,
  onOpenChange,
  onDone,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onDone: (next: SpacesState) => void
}) {
  const { t } = useI18n()
  const [name, setName] = useState("")
  const [device, setDevice] = useState(readDevice)
  const [password, setPassword] = useState("")
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState("")

  useEffect(() => {
    if (!open) return
    setErr("")
    setPassword("")
    const remembered = readDevice()
    setDevice(remembered)
    if (!remembered) {
      void api
        .spaceDeviceName()
        .then((n) => {
          if (n) setDevice((current) => current || n)
        })
        .catch(() => {})
    }
    return pushCloser(() => onOpenChange(false))
  }, [open, onOpenChange])

  const submit = async () => {
    const missingName = !name.trim()
    const missingDevice = !device.trim()
    if (missingName || missingDevice) {
      setErr(
        missingName && missingDevice
          ? t("spacesNeedBoth")
          : missingName
            ? t("spacesNeedName")
            : t("spacesNeedDevice"),
      )
      return
    }
    setBusy(true)
    setErr("")
    try {
      const next = await api.spaceCreate(name.trim(), password, device.trim())
      rememberDevice(device.trim())
      onDone(next)
      setName("")
      setPassword("")
      onOpenChange(false)
    } catch (e) {
      setErr(messageOf(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[440px] overflow-hidden p-0">
        <DialogHead icon={Users} title={t("spacesNew")} body={t("spacesCreateBody")} />
        <div className="grid gap-4 px-5 py-4">
          <Field label={t("spacesName")}>
            <Input
              value={name}
              onChange={(e) => {
                setErr("")
                setName(e.target.value)
              }}
              autoFocus
              maxLength={40}
              dir="auto"
              className="h-9"
            />
          </Field>
          <Field label={t("spacesDevice")}>
            <Input
              value={device}
              onChange={(e) => {
                setErr("")
                setDevice(e.target.value)
              }}
              maxLength={40}
              dir="auto"
              placeholder={t("spacesDevicePlaceholder")}
              className="h-9"
            />
          </Field>
          <Field label={t("spacesPassword")} hint={t("spacesPasswordHint")}>
            <Input
              value={password}
              onChange={(e) => {
                setErr("")
                setPassword(e.target.value)
              }}
              type="password"
              maxLength={64}
              dir="ltr"
              className="h-9"
            />
          </Field>
          {err ? <p className="text-[12.5px] leading-snug text-[var(--red)]">{err}</p> : null}
        </div>
        <div className="flex justify-end gap-2 border-t border-line px-5 py-3">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-[8px] border border-line px-4 text-[13px] text-txt2 hover:text-txt"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void submit()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-4 text-[13px] font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {t("spacesCreateAction")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function JoinDialog({
  open,
  onOpenChange,
  onDone,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onDone: (next: SpacesState) => void
}) {
  const { t } = useI18n()
  const [code, setCode] = useState("")
  const [device, setDevice] = useState(readDevice)
  const [password, setPassword] = useState("")
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState("")

  useEffect(() => {
    if (!open) return
    setErr("")
    setPassword("")
    const remembered = readDevice()
    setDevice(remembered)
    if (!remembered) {
      void api
        .spaceDeviceName()
        .then((n) => {
          if (n) setDevice((current) => current || n)
        })
        .catch(() => {})
    }
    return pushCloser(() => onOpenChange(false))
  }, [open, onOpenChange])

  const submit = async () => {
    if (!/^qvs1:[0-9a-z]{8}:[0-9a-f]{32}$/i.test(code.trim())) {
      setErr(t("spacesNeedCode"))
      return
    }
    if (!device.trim()) {
      setErr(t("spacesNeedDevice"))
      return
    }
    setBusy(true)
    setErr("")
    try {
      const next = await api.spaceJoin(code.trim(), password, device.trim())
      rememberDevice(device.trim())
      onDone(next)
      setCode("")
      setPassword("")
      onOpenChange(false)
    } catch (e) {
      setErr(messageOf(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[460px] overflow-hidden p-0">
        <DialogHead icon={LogIn} title={t("spacesJoinTitle")} body={t("spacesJoinBody")} />
        <div className="grid gap-4 px-5 py-4">
          <Field label={t("spacesCode")} hint={t("spacesCodeHint")}>
            <Input
              value={code}
              onChange={(e) => {
                setErr("")
                setCode(e.target.value)
              }}
              autoFocus
              dir="ltr"
              spellCheck={false}
              className="h-9 font-mono text-[12px]"
            />
          </Field>
          <Field label={t("spacesPassword")}>
            <Input
              value={password}
              onChange={(e) => {
                setErr("")
                setPassword(e.target.value)
              }}
              type="password"
              maxLength={64}
              dir="ltr"
              className="h-9"
            />
          </Field>
          <Field label={t("spacesDevice")}>
            <Input
              value={device}
              onChange={(e) => {
                setErr("")
                setDevice(e.target.value)
              }}
              maxLength={40}
              dir="auto"
              placeholder={t("spacesDevicePlaceholder")}
              className="h-9"
            />
          </Field>
          {err ? <p className="text-[12.5px] leading-snug text-[var(--red)]">{err}</p> : null}
        </div>
        <div className="flex justify-end gap-2 border-t border-line px-5 py-3">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="press h-9 rounded-[8px] border border-line px-4 text-[13px] text-txt2 hover:text-txt"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void submit()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-4 text-[13px] font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {t("spacesJoinAction")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

type Confirm =
  | { kind: "leave" }
  | { kind: "delete" }
  | { kind: "kick"; memberId: string; name: string }

function ConfirmDialog({
  confirm,
  onClose,
  onDone,
  spaceId,
}: {
  confirm: Confirm | null
  onClose: () => void
  onDone: (next: SpacesState) => void
  spaceId: string
}) {
  const { t } = useI18n()
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState("")

  useEffect(() => {
    if (!confirm) return
    setErr("")
    return pushCloser(onClose)
  }, [confirm, onClose])

  const title =
    confirm?.kind === "delete"
      ? t("spacesDelete")
      : confirm?.kind === "kick"
        ? t("spacesKick")
        : t("spacesLeave")
  const body =
    confirm?.kind === "delete"
      ? t("spacesDeleteBody")
      : confirm?.kind === "kick"
        ? t("spacesKickBody")
        : t("spacesLeaveBody")

  const go = async () => {
    if (!confirm) return
    setBusy(true)
    setErr("")
    try {
      const next =
        confirm.kind === "delete"
          ? await api.spaceDelete(spaceId)
          : confirm.kind === "kick"
            ? await api.spaceKick(spaceId, confirm.memberId)
            : await api.spaceLeave(spaceId)
      onDone(next)
      onClose()
    } catch (e) {
      setErr(messageOf(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={!!confirm} onOpenChange={(o) => (!o ? onClose() : undefined)}>
      <DialogContent className="max-w-[400px] p-5">
        <DialogTitle className="text-[17px] font-semibold tracking-[-0.01em]">{title}</DialogTitle>
        <p className="text-[13px] leading-relaxed text-txt2">{body}</p>
        {err ? <p className="text-[12.5px] leading-snug text-[var(--red)]">{err}</p> : null}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="press h-9 rounded-[8px] border border-line px-4 text-[13px] text-txt2 hover:text-txt"
          >
            {t("cancel")}
          </button>
          <button
            type="button"
            onClick={() => void go()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--red)] px-4 text-[13px] font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {t("spacesConfirm")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function PersonRow({
  member,
  canKick,
  onKick,
}: {
  member: SpaceMember
  canKick: boolean
  onKick: (m: SpaceMember) => void
}) {
  const { t } = useI18n()
  const initial = (member.name || "?").trim().slice(0, 1).toUpperCase()
  return (
    <div className="flex items-center gap-3 px-3.5 py-3">
      <span
        className={cn(
          "grid size-9 shrink-0 place-items-center rounded-full border text-[13px] font-semibold",
          member.self_pc
            ? "border-[var(--brand-line)] bg-[var(--brand)]/12 text-brand-strong"
            : "border-line bg-white/[0.035] text-txt2",
        )}
        aria-hidden
      >
        {initial}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-1.5">
          <span className="truncate text-[14.5px] font-medium text-txt">{member.name || "-"}</span>
          {member.self_pc ? (
            <span className="shrink-0 rounded-[4px] bg-white/[0.06] px-1.5 py-0.5 text-[10px] uppercase tracking-[0.08em] text-txt3">
              {t("spacesThisPc")}
            </span>
          ) : null}
          {member.owner ? <Crown className="size-3.5 shrink-0 text-[var(--amber)]" aria-hidden /> : null}
        </div>
        <div className="mt-0.5 flex items-center gap-1 font-mono text-[12px] text-txt3">
          {member.address}
          <CopyButton value={member.address} label={t("copy")} />
        </div>
      </div>
      <span
        className={cn(
          "flex shrink-0 items-center gap-1.5 text-[12px]",
          member.online ? "text-txt2" : "text-txt3",
        )}
      >
        <span
          className={cn("size-1.5 rounded-full", member.online ? "bg-[var(--green)]" : "bg-txt3/45")}
          aria-hidden
        />
        {member.online ? t("netStatusOnline") : t("netOffline")}
      </span>
      {canKick ? (
        <button
          type="button"
          onClick={() => onKick(member)}
          aria-label={t("spacesKick")}
          title={t("spacesKick")}
          className="press grid size-7 shrink-0 place-items-center rounded-[7px] text-txt3 hover:bg-white/[0.06] hover:text-[var(--red)]"
        >
          <UserMinus className="size-3.5" aria-hidden />
        </button>
      ) : null}
    </div>
  )
}

export function Spaces() {
  const { t } = useI18n()
  const calm = useReducedMotion()
  const { vpnOn, toggle } = useApp()
  const [state, setState] = useState<SpacesState | null>(null)
  const [selected, setSelected] = useState("")
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState("")
  const [note, setNote] = useState("")
  const [createOpen, setCreateOpen] = useState(false)
  const [joinOpen, setJoinOpen] = useState(false)
  const [confirm, setConfirm] = useState<Confirm | null>(null)
  const [probe, setProbe] = useState<SpaceProbe | null>(null)
  const [probeBusy, setProbeBusy] = useState(false)
  const [probeError, setProbeError] = useState("")
  const [direct, setDirect] = useState<DirectStatus | null>(null)
  const [directBusy, setDirectBusy] = useState(false)
  const [directError, setDirectError] = useState("")

  const adopt = useCallback((next: SpacesState) => {
    setState(next)
    setSelected((cur) => {
      if (next.spaces.some((s) => s.id === cur)) return cur
      if (next.spaces.some((s) => s.id === next.active)) return next.active
      return next.spaces[0]?.id ?? ""
    })
  }, [])

  /** After create or join, open the new space instead of leaving the old
      selection on screen. The server made it active; the screen should show
      the one the user just built. */
  const adoptNew = useCallback((next: SpacesState) => {
    setState(next)
    setSelected(next.active || next.spaces[0]?.id || "")
  }, [])

  useEffect(() => {
    let alive = true
    void api
      .spacesState()
      .then((next) => {
        if (!alive) return
        adopt(next)
      })
      .catch((e) => {
        if (alive) setError(messageOf(e))
      })
    return () => {
      alive = false
    }
  }, [adopt])

  const space = useMemo(
    () => state?.spaces.find((s) => s.id === selected) ?? null,
    [state, selected],
  )

  const refresh = useCallback(async (quiet: boolean) => {
    if (!selected) return
    if (!quiet) setBusy("refresh")
    try {
      const next = await api.spaceRefresh(selected)
      adopt(next)
      if (!quiet) setError("")
    } catch (e) {
      if (!quiet) setError(messageOf(e))
    } finally {
      if (!quiet) setBusy(null)
    }
  }, [selected, adopt])

  // The roster's online dots are the only live fact here. A slow poll keeps
  // them honest without turning the screen into a spinner; an offline
  // coordinator keeps the last known roster on screen.
  useEffect(() => {
    if (!selected) return
    let alive = true
    const tick = async () => {
      try {
        const next = await api.spaceRefresh(selected)
        if (alive) adopt(next)
      } catch {
        /* keep the cached roster */
      }
    }
    void tick()
    const id = window.setInterval(tick, 25000)
    return () => {
      alive = false
      window.clearInterval(id)
    }
  }, [selected, adopt])

  useEffect(() => {
    setNote("")
    setProbe(null)
    setProbeError("")
  }, [selected])

  const activate = async () => {
    if (!space) return
    setBusy("activate")
    setError("")
    try {
      const next = await api.spaceActivate(space.id)
      adopt(next)
      setNote("")
    } catch (e) {
      setError(messageOf(e))
    } finally {
      setBusy(null)
    }
  }

  const rotate = async () => {
    if (!space) return
    setBusy("rotate")
    setError("")
    try {
      adopt(await api.spaceRotate(space.id))
    } catch (e) {
      setError(messageOf(e))
    } finally {
      setBusy(null)
    }
  }

  const members = useMemo(() => {
    if (!space) return []
    return [...space.members].sort((a, b) => {
      if (a.self_pc !== b.self_pc) return a.self_pc ? -1 : 1
      if (a.owner !== b.owner) return a.owner ? -1 : 1
      if (a.online !== b.online) return a.online ? -1 : 1
      return a.name.localeCompare(b.name)
    })
  }, [space])

  const active = !!state && !!space && state.active === space.id
  const applied = !!state && state.tunnel_space === space?.id
  const live = active && applied && (vpnOn || !!state?.tunnel_on)
  const online = space?.members.filter((m) => m.online).length ?? 0

  useEffect(() => {
    if (live) setNote("")
  }, [live])

  // Removing the space the live session carries has to end that session;
  // otherwise the old endpoint keeps running with no row to control it.
  const onConfirmDone = (next: SpacesState) => {
    const wasLive = live
    adopt(next)
    if (wasLive && confirm && confirm.kind !== "kick") toggle()
  }

  const probePeer = useMemo(
    () =>
      space?.members.find((m) => !m.self_pc && m.online) ??
      space?.members.find((m) => !m.self_pc) ??
      null,
    [space],
  )

  const runProbe = async () => {
    if (!space || !probePeer) return
    setProbeBusy(true)
    setProbe(null)
    setProbeError("")
    try {
      setProbe(await api.spaceProbe(space.id, probePeer.id))
    } catch (e) {
      setProbeError(messageOf(e))
    } finally {
      setProbeBusy(false)
    }
  }

  const loadDirect = useCallback(async () => {
    try {
      setDirect(await api.spaceDirectStatus())
    } catch {
      /* the command is absent outside a Windows build */
    }
  }, [])

  useEffect(() => {
    void loadDirect()
  }, [loadDirect])

  useEffect(() => {
    if (!direct?.running) return
    const id = window.setInterval(() => void loadDirect(), 2000)
    return () => window.clearInterval(id)
  }, [direct?.running, loadDirect])

  const startDirect = async () => {
    if (!space || !probePeer) return
    setDirectBusy(true)
    setDirectError("")
    try {
      setDirect(await api.spaceDirectStart(space.id, probePeer.id))
    } catch (e) {
      setDirectError(messageOf(e))
    } finally {
      setDirectBusy(false)
    }
  }

  const stopDirect = async () => {
    setDirectBusy(true)
    setDirectError("")
    try {
      setDirect(await api.spaceDirectStop())
    } catch (e) {
      setDirectError(messageOf(e))
    } finally {
      setDirectBusy(false)
    }
  }

  return (
    <div
      className="flex min-h-full flex-col gap-4 lg:h-full lg:min-h-0"
      data-space-live={live ? "1" : "0"}
      data-space-vpn={vpnOn ? "1" : "0"}
      data-space-tunnel={state?.tunnel_on ? "1" : "0"}
    >
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="min-w-0">
          <div className="text-[11.5px] uppercase tracking-[0.16em] text-txt3">{t("spacesKicker")}</div>
          <h1 className="mt-1 font-display text-[30px] font-semibold leading-none tracking-[-0.03em] text-txt">
            {t("tabSpaces")}
          </h1>
          <p className="mt-2 text-[13px] leading-relaxed text-txt2">{t("spacesTag")}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            onClick={() => setJoinOpen(true)}
            className="press flex h-10 items-center gap-2 rounded-[9px] border border-line bg-white/[0.03] px-4 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)]"
          >
            <LogIn className="size-4" aria-hidden />
            {t("spacesJoin")}
          </button>
          <button
            type="button"
            onClick={() => setCreateOpen(true)}
            className="press flex h-10 items-center gap-2 rounded-[9px] bg-[var(--brand)] px-4 text-[13px] font-medium text-white transition-[filter] hover:brightness-110"
          >
            <Plus className="size-4" aria-hidden />
            {t("spacesCreate")}
          </button>
        </div>
      </header>

      <AnimatePresence initial={false}>
        {error ? (
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            className="flex items-start gap-2 rounded-[10px] border border-[var(--red-line)] bg-[var(--red)]/[0.06] px-3.5 py-2.5 text-[12.5px] leading-snug text-[var(--red)]"
          >
            <span className="min-w-0 flex-1">{error}</span>
            <button
              type="button"
              onClick={() => setError("")}
              aria-label={t("cancel")}
              className="press grid size-5 shrink-0 place-items-center rounded-[5px] hover:bg-white/[0.06]"
            >
              <X className="size-3.5" aria-hidden />
            </button>
          </motion.div>
        ) : null}
      </AnimatePresence>

      {!state ? (
        <div className="grid min-h-[360px] place-items-center rounded-[16px] border border-line bg-[var(--panel)]">
          <Loader2 className="size-6 animate-spin text-txt3" aria-hidden />
        </div>
      ) : state.spaces.length === 0 ? (
        <div className="flex min-h-[360px] flex-col items-center justify-center gap-3 rounded-[16px] border border-line bg-[var(--panel)] px-6 text-center">
          <span className="relative grid size-16 place-items-center rounded-[18px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
            <Users className="size-8 text-brand-strong" aria-hidden />
          </span>
          <h2 className="mt-1 font-display text-[22px] font-semibold text-txt">{t("spacesEmpty")}</h2>
          <p className="max-w-[380px] text-[13px] leading-relaxed text-txt2">{t("spacesEmptyBody")}</p>
          <div className="mt-1 flex gap-2">
            <button
              type="button"
              onClick={() => setJoinOpen(true)}
              className="press flex h-9 items-center gap-2 rounded-[8px] border border-line px-4 text-[13px] text-txt2 hover:text-txt"
            >
              <LogIn className="size-4" aria-hidden />
              {t("spacesJoin")}
            </button>
            <button
              type="button"
              onClick={() => setCreateOpen(true)}
              className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-4 text-[13px] font-medium text-white hover:brightness-110"
            >
              <Plus className="size-4" aria-hidden />
              {t("spacesCreate")}
            </button>
          </div>
        </div>
      ) : (
        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
          <section className="flex min-h-0 flex-col overflow-hidden rounded-[16px] border border-line bg-[var(--panel)]">
            <div className="flex shrink-0 items-center justify-between border-b border-line px-4 py-3">
              <span className="text-[11.5px] uppercase tracking-[0.12em] text-txt3">{t("spacesTitle")}</span>
              <span className="font-mono text-[12px] text-txt3">{state.spaces.length}</span>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto">
              {state.spaces.map((s) => {
                const on = s.members.filter((m) => m.online).length
                const isSel = s.id === selected
                return (
                  <button
                    key={s.id}
                    type="button"
                    onClick={() => setSelected(s.id)}
                    aria-current={isSel ? "true" : undefined}
                    className={cn(
                      "relative flex w-full items-center gap-3 border-b border-line/60 px-3.5 py-3 text-start transition-colors last:border-b-0",
                      isSel ? "text-txt" : "hover:bg-white/[0.025]",
                    )}
                  >
                    {isSel ? (
                      <motion.span
                        layoutId="spaces-selection"
                        transition={calm ? { duration: 0 } : { type: "spring", stiffness: 430, damping: 38 }}
                        className="pointer-events-none absolute inset-x-1.5 inset-y-1.5 rounded-[11px] bg-[var(--brand)]/[0.10] ring-1 ring-inset ring-[var(--brand-line)]/40"
                        aria-hidden
                      />
                    ) : null}
                    <span
                      className={cn(
                        "relative z-10 grid size-9 shrink-0 place-items-center rounded-[10px] border",
                        isSel
                          ? "border-[var(--brand-line)] bg-[var(--brand)]/10"
                          : "border-line bg-white/[0.025]",
                      )}
                    >
                      <Users
                        className={cn("size-[18px]", isSel ? "text-brand-strong" : "text-txt2")}
                        aria-hidden
                      />
                    </span>
                    <span className="relative z-10 min-w-0 flex-1">
                      <span className="block truncate text-[14.5px] font-medium text-txt">{s.name}</span>
                      <span className="mt-0.5 flex items-center gap-1.5 font-mono text-[11.5px] text-txt3">
                        {s.address}
                        <span className="text-txt3/50">·</span>
                        <span className="font-sans">{on}</span>
                        <span className="font-sans">{t("spacesOnline")}</span>
                      </span>
                    </span>
                    {s.id === state.tunnel_space && state.tunnel_on ? (
                      <span
                        className="relative z-10 size-1.5 shrink-0 rounded-full bg-[var(--green)]"
                        aria-label={t("spacesLive")}
                        title={t("spacesLive")}
                      />
                    ) : s.id === state.active ? (
                      <span
                        className="relative z-10 size-1.5 shrink-0 rounded-full bg-[var(--brand)]"
                        aria-label={t("spacesReady")}
                        title={t("spacesReady")}
                      />
                    ) : null}
                    <ChevronRight className="relative z-10 size-4 shrink-0 text-txt3" aria-hidden />
                  </button>
                )
              })}
            </div>
            <div className="flex shrink-0 items-start gap-1.5 border-t border-line px-4 pb-1 pt-3 text-[11.5px] leading-snug text-txt3">
              <LockKeyhole className="mt-0.5 size-3 shrink-0" aria-hidden />
              {t("spacesOnlyInvite")}
            </div>
            <div className="grid shrink-0 grid-cols-2 gap-2 p-3">
              <button
                type="button"
                onClick={() => setJoinOpen(true)}
                className="press flex h-9 items-center justify-center gap-1.5 rounded-[8px] border border-line text-[12.5px] text-txt2 hover:text-txt"
              >
                <LogIn className="size-3.5" aria-hidden />
                {t("spacesJoin")}
              </button>
              <button
                type="button"
                onClick={() => setCreateOpen(true)}
                className="press flex h-9 items-center justify-center gap-1.5 rounded-[8px] border border-line text-[12.5px] text-txt2 hover:text-txt"
              >
                <Plus className="size-3.5" aria-hidden />
                {t("spacesCreate")}
              </button>
            </div>
          </section>

          <section className="flex min-h-0 flex-col overflow-hidden rounded-[16px] border border-line bg-[var(--panel)]">
            {!space ? (
              <div className="flex h-full min-h-[360px] flex-col items-center justify-center gap-3 px-6 text-center">
                <span className="grid size-12 place-items-center rounded-[14px] border border-line bg-white/[0.025]">
                  <Users className="size-6 text-txt3" aria-hidden />
                </span>
                <h2 className="font-display text-[20px] font-semibold text-txt">{t("spacesSelect")}</h2>
                <p className="max-w-[320px] text-[13px] text-txt2">{t("spacesSelectBody")}</p>
              </div>
            ) : (
              <>
                <div className="shrink-0 border-b border-line px-5 py-4">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-3">
                      <span className="grid size-12 place-items-center rounded-[13px] border border-[var(--brand-line)] bg-[var(--brand)]/10">
                        <Users className="size-6 text-brand-strong" aria-hidden />
                      </span>
                      <div className="min-w-0">
                        <div className="flex min-w-0 items-center gap-2">
                          <h2 className="truncate font-display text-[22px] font-semibold leading-tight text-txt">
                            {space.name}
                          </h2>
                          {space.owner ? (
                            <span className="flex shrink-0 items-center gap-1 rounded-[5px] border border-[var(--amber-line)] bg-[var(--amber)]/[0.08] px-1.5 py-0.5 text-[10px] uppercase tracking-[0.08em] text-[var(--amber)]">
                              <Crown className="size-3" aria-hidden />
                              {t("spacesOwner")}
                            </span>
                          ) : null}
                        </div>
                        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12.5px] text-txt3">
                          <span className="flex items-center gap-1 font-mono text-[12px] text-txt2">
                            {space.address}
                            <CopyButton value={space.address} label={t("copy")} />
                          </span>
                          <span className="text-txt3/50">·</span>
                          <span>
                            {online} {t("spacesOnline")}
                          </span>
                          <span className="text-txt3/50">·</span>
                          <span className="font-mono text-[11.5px]">{space.subnet}</span>
                        </div>
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => void refresh(false)}
                      disabled={busy === "refresh"}
                      aria-label={t("refresh")}
                      title={t("refresh")}
                      className="press grid size-8 place-items-center rounded-[7px] text-txt3 hover:bg-white/[0.06] hover:text-txt disabled:opacity-50"
                    >
                      {busy === "refresh" ? (
                        <Loader2 className="size-3.5 animate-spin" aria-hidden />
                      ) : (
                        <RefreshCw className="size-3.5" aria-hidden />
                      )}
                    </button>
                  </div>
                </div>

                <div className="min-h-0 flex-1 overflow-y-auto p-5">
                  <div className="grid gap-5">
                    <div
                      className={cn(
                        "rounded-[12px] border px-4 py-3.5",
                        live
                          ? "border-[var(--brand-line)] bg-[var(--brand)]/[0.05]"
                          : "border-line bg-white/[0.02]",
                      )}
                    >
                      <div className="flex flex-wrap items-center justify-between gap-3">
                        <div className="flex items-center gap-2 text-[13.5px] font-medium text-txt">
                          {live || direct?.running ? (
                            <Wifi className="size-4 text-brand-strong" aria-hidden />
                          ) : (
                            <WifiOff className="size-4 text-txt3" aria-hidden />
                          )}
                          {live
                            ? t("spacesLive")
                            : direct?.running
                              ? t("spacesDirectConnected")
                              : active
                                ? t("spacesReady")
                                : t("spacesOff")}
                        </div>
                        {live ? (
                          <span className="flex items-center gap-1.5 text-[12px] text-txt2">
                            <span className="size-1.5 rounded-full bg-[var(--green)]" aria-hidden />
                            {t("spacesLive")}
                          </span>
                        ) : direct?.running ? (
                          <span className="flex items-center gap-1.5 font-mono text-[12px] text-txt2">
                            <span className="size-1.5 rounded-full bg-[var(--green)]" aria-hidden />
                            {direct.address.split("/")[0]}
                          </span>
                        ) : active ? (
                          <button
                            type="button"
                            onClick={() => void startDirect()}
                            disabled={!probePeer || directBusy}
                            className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-4 text-[12.5px] font-medium text-white hover:brightness-110 disabled:opacity-50"
                          >
                            {directBusy ? (
                              <Loader2 className="size-3.5 animate-spin" aria-hidden />
                            ) : (
                              <Cable className="size-3.5" aria-hidden />
                            )}
                            {probePeer ? t("spacesConnectDirect") : t("spacesWaitMember")}
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => void activate()}
                            disabled={busy === "activate"}
                            className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-4 text-[12.5px] font-medium text-white hover:brightness-110 disabled:opacity-60"
                          >
                            {busy === "activate" ? (
                              <Loader2 className="size-3.5 animate-spin" aria-hidden />
                            ) : (
                              <Link2 className="size-3.5" aria-hidden />
                            )}
                            {t("spacesConnect")}
                          </button>
                        )}
                      </div>
                      <p className="mt-1.5 text-[12.5px] leading-relaxed text-txt2">
                        {note ||
                          (live
                            ? t("spacesLiveHint")
                            : direct?.running
                              ? t("spacesDirectHint")
                              : active
                                ? probePeer
                                  ? t("spacesConnectDirectHint")
                                  : t("spacesWaitHint")
                                : t("spacesSelectBody"))}
                      </p>
                    </div>

                    <div className="rounded-[12px] border border-[var(--brand-line)] bg-[var(--brand)]/[0.04] px-4 py-3.5">
                      <div className="flex flex-wrap items-center justify-between gap-3">
                        <div className="flex items-center gap-2 text-[13.5px] font-medium text-txt">
                          <Cable className="size-4 text-brand-strong" aria-hidden />
                          {t("spacesDirectLan")}
                        </div>
                        {direct?.running ? (
                          <button
                            type="button"
                            onClick={() => void stopDirect()}
                            disabled={directBusy}
                            className="press flex h-9 items-center gap-2 rounded-[8px] border border-line bg-white/[0.03] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--red-line)] disabled:opacity-60"
                          >
                            {directBusy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
                            {t("spacesDirectStop")}
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => void startDirect()}
                            disabled={!probePeer || directBusy}
                            className="press flex h-9 items-center gap-2 rounded-[8px] bg-[var(--brand)] px-3.5 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-110 disabled:opacity-50"
                          >
                            {directBusy ? (
                              <Loader2 className="size-3.5 animate-spin" aria-hidden />
                            ) : (
                              <Cable className="size-3.5" aria-hidden />
                            )}
                            {directBusy ? t("spacesDirectStarting") : t("spacesDirectStart")}
                          </button>
                        )}
                      </div>
                      <p className="mt-1.5 text-[12.5px] leading-relaxed text-txt2">{t("spacesDirectBody")}</p>
                      {probePeer ? (
                        <p className="mt-1 text-[12px] text-txt3">
                          {t("spacesProbeWith")} {probePeer.name}
                        </p>
                      ) : null}
                      {direct?.running ? (
                        <div className="mt-3 rounded-[10px] border border-line/70 bg-black/10 px-3 py-2.5">
                          <div className="flex items-center gap-2 text-[12.5px] font-medium text-[var(--green)]">
                            <span className="size-1.5 rounded-full bg-[var(--green)]" aria-hidden />
                            {t("spacesDirectOn")}
                          </div>
                          <div className="mt-1.5 grid gap-1 text-[12px] text-txt2">
                            <span>
                              {t("spacesDirectAddress")}:{" "}
                              <span className="font-mono text-txt">{direct.address.split("/")[0]}</span>
                            </span>
                            <span>
                              {t("spacesDirectAdapter")}:{" "}
                              <span className="font-mono text-txt3">{direct.adapter || "-"}</span>
                            </span>
                          </div>
                          {direct.error ? (
                            <p className="mt-1.5 text-[11.5px] leading-snug text-[var(--red)]">{direct.error}</p>
                          ) : null}
                        </div>
                      ) : direct && direct.error ? (
                        <p className="mt-2 text-[12px] leading-snug text-[var(--red)]">
                          {t("spacesDirectFailed")}: {direct.error}
                        </p>
                      ) : null}
                      {directError ? (
                        <p className="mt-2 text-[12px] leading-snug text-[var(--red)]">{directError}</p>
                      ) : null}
                      <p className="mt-2 text-[11.5px] leading-snug text-txt3">{t("spacesDirectHint")}</p>
                    </div>

                    <div className="rounded-[12px] border border-line bg-white/[0.02] px-4 py-3.5">
                      <div className="flex flex-wrap items-center justify-between gap-3">
                        <div className="flex items-center gap-2 text-[13.5px] font-medium text-txt">
                          <Radar className="size-4 text-brand-strong" aria-hidden />
                          {t("spacesProbeTitle")}
                        </div>
                        <button
                          type="button"
                          onClick={() => void runProbe()}
                          disabled={!probePeer || probeBusy}
                          className="press flex h-9 items-center gap-2 rounded-[8px] border border-line bg-white/[0.03] px-3.5 text-[12.5px] text-txt transition-colors hover:border-[var(--brand-line)] disabled:opacity-50"
                        >
                          {probeBusy ? (
                            <Loader2 className="size-3.5 animate-spin" aria-hidden />
                          ) : (
                            <Radar className="size-3.5" aria-hidden />
                          )}
                          {probeBusy ? t("spacesProbeBusy") : t("spacesProbeAction")}
                        </button>
                      </div>
                      <p className="mt-1.5 text-[12.5px] leading-relaxed text-txt2">{t("spacesProbeBody")}</p>
                      {probePeer ? (
                        <p className="mt-1 text-[12px] text-txt3">
                          {t("spacesProbeWith")} {probePeer.name}
                        </p>
                      ) : (
                        <p className="mt-1 text-[12px] text-txt3">{t("spacesProbeNoPeer")}</p>
                      )}
                      {probe ? (
                        <div className="mt-3 rounded-[10px] border border-line/70 bg-black/10 px-3 py-2.5">
                          <div className="flex flex-wrap items-center gap-2 text-[12.5px]">
                            {probe.direct ? (
                              <Wifi className="size-4 text-[var(--green)]" aria-hidden />
                            ) : (
                              <WifiOff className="size-4 text-[var(--amber)]" aria-hidden />
                            )}
                            <span
                              className={
                                probe.direct ? "font-medium text-[var(--green)]" : "font-medium text-[var(--amber)]"
                              }
                            >
                              {probe.direct ? t("spacesProbeDirect") : t("spacesProbeRelay")}
                            </span>
                            {probe.direct && probe.rtt_ms != null ? (
                              <span className="font-mono text-[12px] text-txt">{probe.rtt_ms.toFixed(1)} ms</span>
                            ) : null}
                          </div>
                          <p className="mt-1 break-all font-mono text-[10.5px] leading-relaxed text-txt3">
                            {probe.self_endpoint || "-"} - {probe.peer_endpoint || "-"}
                          </p>
                          {probe.note ? (
                            <p className="mt-1 text-[11.5px] leading-snug text-txt3">{probe.note}</p>
                          ) : null}
                        </div>
                      ) : null}
                      {probeError ? (
                        <p className="mt-2 text-[12px] leading-snug text-[var(--red)]">
                          {t("spacesProbeError")}: {probeError}
                        </p>
                      ) : null}
                      <p className="mt-2 text-[11.5px] leading-snug text-txt3">
                        {t("spacesProbeBoth")} {t("spacesProbeHint")}
                      </p>
                    </div>

                    {space.owner ? (
                      <div>
                        <div className="flex items-center justify-between gap-2">
                          <span className="flex items-center gap-2 text-[11.5px] uppercase tracking-[0.12em] text-txt3">
                            <Link2 className="size-3.5" aria-hidden />
                            {t("spacesInvite")}
                          </span>
                          <button
                            type="button"
                            onClick={() => void rotate()}
                            disabled={busy === "rotate"}
                            className="press flex items-center gap-1.5 text-[12px] text-brand-strong hover:underline disabled:opacity-50"
                          >
                            {busy === "rotate" ? (
                              <Loader2 className="size-3 animate-spin" aria-hidden />
                            ) : (
                              <RotateCw className="size-3" aria-hidden />
                            )}
                            {t("spacesRotate")}
                          </button>
                        </div>
                        <div className="mt-2 flex items-center gap-2 rounded-[10px] border border-line bg-white/[0.025] px-3 py-2">
                          <code className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-txt" dir="ltr">
                            {space.invite}
                          </code>
                          <CopyButton value={space.invite} label={t("copy")} />
                        </div>
                        <p className="mt-1.5 text-[12px] leading-snug text-txt3">{t("spacesInviteHint")}</p>
                      </div>
                    ) : null}

                    <div>
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-[11.5px] uppercase tracking-[0.12em] text-txt3">
                          {t("spacesMembers")}
                        </span>
                        <span className="font-mono text-[11.5px] text-txt3">{members.length}</span>
                      </div>
                      <div className="mt-2 divide-y divide-line/60 overflow-hidden rounded-[12px] border border-line">
                        {members.length === 0 ? (
                          <p className="px-4 py-4 text-[12.5px] text-txt3">{t("spacesMembersEmpty")}</p>
                        ) : (
                          members.map((m) => (
                            <PersonRow
                              key={m.id}
                              member={m}
                              canKick={space.owner && !m.self_pc}
                              onKick={(member) =>
                                setConfirm({ kind: "kick", memberId: member.id, name: member.name })
                              }
                            />
                          ))
                        )}
                      </div>
                    </div>

                    <div className="grid gap-2">
                      <p className="flex items-start gap-2 text-[12px] leading-relaxed text-txt3">
                        <Monitor className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                        {t("spacesLocalNote")}
                      </p>
                      <p className="flex items-start gap-2 text-[12px] leading-relaxed text-txt3">
                        <Crown className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                        {t("spacesTrust")}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="flex shrink-0 items-center justify-between gap-3 border-t border-line px-5 py-3">
                  <span className="truncate text-[12px] text-txt3">
                    {t("spacesMembers")}: {members.length}
                  </span>
                  <button
                    type="button"
                    onClick={() => setConfirm({ kind: space.owner ? "delete" : "leave" })}
                    className={cn(
                      "press flex h-9 items-center gap-2 rounded-[8px] border px-4 text-[12.5px]",
                      space.owner
                        ? "border-[var(--red-line)] text-[var(--red)] hover:bg-[var(--red)]/[0.06]"
                        : "border-line text-txt2 hover:text-txt",
                    )}
                  >
                    {space.owner ? <Trash2 className="size-3.5" aria-hidden /> : <LogOut className="size-3.5" aria-hidden />}
                    {space.owner ? t("spacesDelete") : t("spacesLeave")}
                  </button>
                </div>
              </>
            )}
          </section>
        </div>
      )}

      <CreateDialog open={createOpen} onOpenChange={setCreateOpen} onDone={adoptNew} />
      <JoinDialog open={joinOpen} onOpenChange={setJoinOpen} onDone={adoptNew} />
      <ConfirmDialog
        confirm={confirm}
        onClose={() => setConfirm(null)}
        onDone={onConfirmDone}
        spaceId={space?.id ?? ""}
      />
    </div>
  )
}
