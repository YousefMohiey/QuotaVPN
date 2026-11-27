import { useCallback, useEffect, useMemo, useState } from "react"
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
    () => spaces.find((space) => space.id === selected) ?? spaces[0] ?? null,
    [spaces, selected],
  )

  const loadSpaces = useCallback(async (): Promise<LanSpace[]> => {
    const list = await api.openradSpaces()
    setSpaces(list)
    setSelected((current) => {
      if (list.some((space) => space.id === current)) return current
      const fallback = list.find((space) => space.default) ?? list[0]
      return fallback?.id ?? ""
    })
    return list
  }, [])

  useEffect(() => {
    void loadSpaces().catch((e) => setError(messageOf(e)))
  }, [loadSpaces])

  useEffect(() => {
    if (!selected) return
    try {
      localStorage.setItem(SPACE_KEY, selected)
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
    setLanPeers([])
  }

  const connectLan = async () => {
    if (!selectedSpace) return
    setBusy(true)
    setError("")
    try {
      const status = await api.openradConnect(selectedSpace.id)
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
    const list = await loadSpaces()
    const fallback = list.find((row) => row.id !== space.id) ?? null
    setSelected(fallback?.id ?? "")
    setLanPeers([])
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
      onRemove={removeSpace}
      onErrorClear={() => setError("")}
    />
  )
}
