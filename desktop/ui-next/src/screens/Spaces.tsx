import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { api, type LanSpace, type OpenRadPeer, type OpenRadStatus } from "@/lib/ipc"
import SpacesConsole from "./SpacesConsole"

const SPACE_KEY = "qc-lan-space"

const messageOf = (e: unknown): string =>
  typeof e === "string" ? e : e instanceof Error ? e.message : String(e)

/**
 * Spaces is the one-button QuotaSpace LAN. Each space is its own OpenRad
 * network, so selecting a space switches the engine to that network and the
 * card shows only that network's peers. The old layer-3 space records and
 * their coordinator are gone.
 */
export function Spaces() {
  const [spaces, setSpaces] = useState<LanSpace[]>([])
  const [selected, setSelected] = useState(() => {
    try {
      return localStorage.getItem(SPACE_KEY) ?? ""
    } catch {
      return ""
    }
  })
  const [lan, setLan] = useState<OpenRadStatus | null>(null)
  const [lanPeers, setLanPeers] = useState<OpenRadPeer[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  const selectedSpace = useMemo(
    () => spaces.find((space) => space.id === selected) ?? null,
    [spaces, selected],
  )
  const selectedRef = useRef(selectedSpace)
  useEffect(() => {
    selectedRef.current = selectedSpace
  }, [selectedSpace])

  const loadSpaces = useCallback(async (): Promise<LanSpace[]> => {
    const list = await api.openradSpaces()
    setSpaces(list)
    setSelected((current) => {
      if (list.some((space) => space.id === current)) return current
      // Only a fresh profile picks a space to show. If the selected space was
      // removed, stay unselected instead of silently moving to another one.
      if (!current) {
        const first = list.find((space) => space.default) ?? list[0]
        return first?.id ?? ""
      }
      return ""
    })
    return list
  }, [])

  useEffect(() => {
    void loadSpaces().catch((e) => setError(messageOf(e)))
  }, [loadSpaces])

  useEffect(() => {
    try {
      if (selected) localStorage.setItem(SPACE_KEY, selected)
      else localStorage.removeItem(SPACE_KEY)
    } catch {
      /* private mode */
    }
  }, [selected])

  const refreshLan = useCallback(async () => {
    try {
      const status = await api.openradStatus()
      setLan(status)
      if (status.phase === "connected") {
        const peers = await api.openradPeers()
        setLanPeers(Array.isArray(peers) ? peers : [])
      } else {
        setLanPeers([])
      }
    } catch {
      setLan(null)
      setLanPeers([])
    }
  }, [])

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
          const joined = status.snapshot?.networks?.[0]?.name
          const current = selectedRef.current
          if (current && joined === current.network) {
            // The engine is on the space the user picked. Any banner left from
            // a previous switch is stale now.
            setError((value) => (value ? "" : value))
          }
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

  const selectSpace = (id: string) => {
    setSelected(id)
    setError("")
    setLanPeers([])
  }

  const connectLan = async () => {
    if (!selectedSpace) return
    setBusy(true)
    setError("")
    try {
      const joinedName = lan?.snapshot?.networks?.[0]?.name ?? ""
      // Switching while connected has its own command: it leaves the current
      // space first, then joins the selected one, without stopping the engine
      // or surfacing an error for a normal switch.
      const status =
        lan?.phase === "connected" && joinedName !== selectedSpace.network
          ? await api.openradSwitch(selectedSpace.id)
          : await api.openradConnect(selectedSpace.id)
      setLan(status)
      const peers = await api.openradPeers()
      setLanPeers(Array.isArray(peers) ? peers : [])
    } catch (e) {
      setError(messageOf(e))
    } finally {
      setBusy(false)
    }
  }

  const stopLan = async () => {
    setBusy(true)
    setError("")
    try {
      await api.openradDisconnect()
      setLan(null)
      setLanPeers([])
    } catch (e) {
      setError(messageOf(e))
    } finally {
      setBusy(false)
    }
  }

  const createSpace = async (name: string) => {
    setError("")
    const space = await api.openradCreateSpace(name)
    await loadSpaces()
    setSelected(space.id)
    setLanPeers([])
    await refreshLan()
  }

  const joinSpace = async (invite: string) => {
    setError("")
    const space = await api.openradJoinSpace(invite)
    await loadSpaces()
    setSelected(space.id)
    setLanPeers([])
    await refreshLan()
  }

  const removeSpace = async (space: LanSpace) => {
    setError("")
    await api.openradDeleteSpace(space.id)
    await loadSpaces()
    setLanPeers([])
    await refreshLan()
  }

  const leaveSpace = async (space: LanSpace) => {
    setError("")
    await api.openradLeaveSpace(space.id)
    await loadSpaces()
    setLanPeers([])
    await refreshLan()
  }

  return (
    <SpacesConsole
      spaces={spaces}
      selected={selectedSpace}
      lan={lan}
      peers={lanPeers}
      busy={busy}
      error={error}
      onSelect={(id) => void selectSpace(id)}
      onConnect={() => void connectLan()}
      onStop={() => void stopLan()}
      onCreate={createSpace}
      onJoin={joinSpace}
      onLeave={leaveSpace}
      onRemove={removeSpace}
      onErrorClear={() => setError("")}
    />
  )
}
