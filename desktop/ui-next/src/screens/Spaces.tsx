import { useCallback, useEffect, useMemo, useState } from "react"
import { Loader2, LogIn, Server, Users } from "@/components/icons"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"
import { api, type OpenRadPeer, type OpenRadStatus, type SpacesState } from "@/lib/ipc"
import { pushCloser } from "@/lib/navStack"
import SpacesConsole from "./SpacesConsole"
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
    <label className="grid gap-2">
      <span className="text-xs uppercase tracking-[0.12em] text-txt3">{label}</span>
      {children}
      {hint ? <span className="text-xs leading-snug text-txt3">{hint}</span> : null}
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
    <div className="relative border-b border-line bg-[var(--surface-2)] px-6 py-6">
      <div className="flex items-center gap-3">
        <span className="grid size-11 place-items-center rounded-xl border border-[var(--brand-line)] bg-[var(--brand)]/10">
          <Icon className="size-5 text-brand-strong" aria-hidden />
        </span>
        <div className="min-w-0">
          <DialogTitle className="text-base font-semibold tracking-[-0.01em]">{title}</DialogTitle>
          <p className="mt-1 text-xs leading-relaxed text-txt2">{body}</p>
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
        <div className="grid gap-4 px-6 py-4">
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
          {err ? <p className="text-xs leading-snug text-[var(--red)]">{err}</p> : null}
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
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {t("spacesCreateAction")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function CoordinatorDialog({
  open,
  onOpenChange,
  onSaved,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onSaved: (url: string) => void
}) {
  const { t } = useI18n()
  const [url, setUrl] = useState("")
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState("")

  useEffect(() => {
    if (!open) return
    setErr("")
    void api
      .spacesApi()
      .then((value) => setUrl(value))
      .catch(() => {})
    return pushCloser(() => onOpenChange(false))
  }, [open, onOpenChange])

  const save = async () => {
    setBusy(true)
    setErr("")
    try {
      const clean = await api.setSpacesApi(url.trim())
      onSaved(clean)
      onOpenChange(false)
    } catch (e) {
      setErr(messageOf(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[480px] overflow-hidden p-0">
        <DialogHead icon={Server} title={t("spacesCoordinator")} body={t("spacesCoordinatorBody")} />
        <div className="grid gap-4 px-6 py-4">
          <Field label={t("spacesCoordinatorAddress")} hint={t("spacesCoordinatorHint")}>
            <Input
              value={url}
              onChange={(e) => {
                setErr("")
                setUrl(e.target.value)
              }}
              autoFocus
              dir="ltr"
              spellCheck={false}
              className="h-9 font-mono text-xs"
            />
          </Field>
          {err ? <p className="text-xs leading-snug text-[var(--red)]">{err}</p> : null}
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
            onClick={() => void save()}
            disabled={busy}
            className="press flex h-9 items-center gap-2 rounded-lg bg-[var(--brand)] px-4 text-sm font-medium text-white hover:brightness-110 disabled:opacity-60"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {t("spacesCoordinatorSave")}
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
    if (!/^(qvs2:[A-Za-z0-9_-]+:)?qvs1:[0-9a-z]{8}:[0-9a-f]{32}$/i.test(code.trim())) {
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
        <div className="grid gap-4 px-6 py-4">
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
              className="h-9 font-mono text-xs"
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
          {err ? <p className="text-xs leading-snug text-[var(--red)]">{err}</p> : null}
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
      <DialogContent className="max-w-[400px] p-6">
        <DialogTitle className="text-base font-semibold tracking-[-0.01em]">{title}</DialogTitle>
        <p className="text-sm leading-relaxed text-txt2">{body}</p>
        {err ? <p className="text-xs leading-snug text-[var(--red)]">{err}</p> : null}
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
            {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : null}
            {t("spacesConfirm")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export function Spaces() {
  const { vpnOn, toggle } = useApp()
  const [state, setState] = useState<SpacesState | null>(null)
  const [selected, setSelected] = useState("")
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState("")
  const [note, setNote] = useState("")
  const [createOpen, setCreateOpen] = useState(false)
  const [joinOpen, setJoinOpen] = useState(false)
  const [coordOpen, setCoordOpen] = useState(false)
  const [apiBase, setApiBase] = useState("")
  const [confirm, setConfirm] = useState<Confirm | null>(null)
  const [lan, setLan] = useState<OpenRadStatus | null>(null)
  const [lanPeers, setLanPeers] = useState<OpenRadPeer[]>([])
  const [lanBusy, setLanBusy] = useState(false)
  const [lanError, setLanError] = useState("")

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

  useEffect(() => {
    void api
      .spacesApi()
      .then(setApiBase)
      .catch(() => {})
  }, [])

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

  const connectLan = async () => {
    setLanBusy(true)
    setLanError("")
    try {
      const status = await api.openradConnect()
      setLan(status)
      const peers = await api.openradPeers()
      setLanPeers(Array.isArray(peers) ? peers : [])
    } catch (e) {
      setLanError(messageOf(e))
    } finally {
      setLanBusy(false)
    }
  }

  const stopLan = async () => {
    setLanBusy(true)
    setLanError("")
    try {
      await api.openradDisconnect()
      setLan(null)
      setLanPeers([])
    } catch (e) {
      setLanError(messageOf(e))
    } finally {
      setLanBusy(false)
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
    const self = space.members.find((m) => m.self_pc)
    const seenAddress = new Set<string>()
    return space.members
      .filter((m) => {
        // A reset identity can leave the old device name behind as an offline
        // member. It is this same PC, not another machine on the network.
        if (!m.self_pc && self && !m.online && m.name === self.name) return false
        if (seenAddress.has(m.address)) return false
        seenAddress.add(m.address)
        return true
      })
      .sort((a, b) => {
        if (a.self_pc !== b.self_pc) return a.self_pc ? -1 : 1
        if (a.owner !== b.owner) return a.owner ? -1 : 1
        if (a.online !== b.online) return a.online ? -1 : 1
        return a.name.localeCompare(b.name)
      })
  }, [space])

  const active = !!state && !!space && state.active === space.id
  const applied = !!state && state.tunnel_space === space?.id
  const live = active && applied && (vpnOn || !!state?.tunnel_on)

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

  // The LAN engine lives outside the space roster. Poll it while this screen
  // is open, and only ask for peers once the engine is actually connected.
  useEffect(() => {
    let alive = true
    let busy = false
    const tick = async () => {
      if (busy) return
      busy = true
      try {
        const status = await api.openradStatus()
        if (!alive) return
        setLan(status)
        if (status.phase === "connected") {
          const peers = await api.openradPeers()
          if (alive) setLanPeers(Array.isArray(peers) ? peers : [])
        } else if (alive) {
          setLanPeers([])
        }
      } catch {
        if (!alive) return
        setLan(null)
        setLanPeers([])
      } finally {
        busy = false
      }
    }
    void tick()
    const id = window.setInterval(() => void tick(), 6000)
    return () => {
      alive = false
      window.clearInterval(id)
    }
  }, [])

  return (
    <>
      <SpacesConsole
        spaces={state?.spaces ?? []}
        selected={selected}
        activeId={state?.active ?? ""}
        space={space}
        members={members}
        active={active}
        live={live}
        lan={lan}
        lanPeers={lanPeers}
        lanBusy={lanBusy}
        lanError={lanError}
        busy={busy}
        error={error}
        note={note}
        apiBase={apiBase}
        onSelect={setSelected}
        onRefresh={() => void refresh(false)}
        onActivate={() => void activate()}
        onRotate={() => void rotate()}
        onLanConnect={() => void connectLan()}
        onLanStop={() => void stopLan()}
        onRemove={() => setConfirm({ kind: space?.owner ? "delete" : "leave" })}
        onKick={(memberId, name) => setConfirm({ kind: "kick", memberId, name })}
        onCreate={() => setCreateOpen(true)}
        onJoin={() => setJoinOpen(true)}
        onCoordinator={() => setCoordOpen(true)}
        onErrorClear={() => setError("")}
      />
      <CreateDialog open={createOpen} onOpenChange={setCreateOpen} onDone={adoptNew} />
      <JoinDialog open={joinOpen} onOpenChange={setJoinOpen} onDone={adoptNew} />
      <CoordinatorDialog open={coordOpen} onOpenChange={setCoordOpen} onSaved={setApiBase} />
      <ConfirmDialog
        confirm={confirm}
        onClose={() => setConfirm(null)}
        onDone={onConfirmDone}
        spaceId={space?.id ?? ""}
      />
    </>
  )
}

