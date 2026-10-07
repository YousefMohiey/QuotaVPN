import { useEffect, useState } from "react"
import { appIcons } from "@/lib/ipc"

/** Per-app icons, fetched once from the backend and kept for the session.
    The backend extracts them from the executables themselves; anything it
    cannot read stays null and the row shows a letter tile instead. */
const cache = new Map<string, string | null>()
const inflight = new Set<string>()
const listeners = new Set<() => void>()

export async function requestIcons(pkgs: string[]) {
  const want = pkgs.filter((p) => !cache.has(p) && !inflight.has(p))
  if (want.length === 0) return
  want.forEach((p) => inflight.add(p))
  try {
    const out = await appIcons(want)
    for (const it of out) cache.set(it.pkg, it.png ? "data:image/png;base64," + it.png : null)
  } catch {
    /* no icons on this build: the tiles stand in */
  }
  want.forEach((p) => {
    inflight.delete(p)
    if (!cache.has(p)) cache.set(p, null)
  })
  listeners.forEach((fn) => fn())
}

/** The icons handed over so far, with a subscription so late arrivals repaint
    the rows that asked for them. */
export function useAppIcons(pkgs: string[]): (pkg: string) => string | undefined {
  const [, force] = useState(0)
  const key = pkgs.join("|")
  useEffect(() => {
    const fn = () => force((n) => n + 1)
    listeners.add(fn)
    return () => {
      listeners.delete(fn)
    }
  }, [])
  useEffect(() => {
    void requestIcons(pkgs)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])
  return (pkg: string) => cache.get(pkg) ?? undefined
}
