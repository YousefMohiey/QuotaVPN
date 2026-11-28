import {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react"
import { getCurrentWebview } from "@tauri-apps/api/webview"

/** The interface scales that fit the desktop window without turning it into a
    phone layout. The webview zoom scales everything, including the fixed px
    sizes that a root font size would miss. */
export const SCALES = [0.9, 1, 1.1, 1.25, 1.5] as const
export type Scale = (typeof SCALES)[number]

const KEY = "qc-scale"

function isScale(value: number): value is Scale {
  return (SCALES as readonly number[]).includes(value)
}

function initialScale(): Scale {
  if (typeof window === "undefined") return 1
  try {
    const saved = Number(localStorage.getItem(KEY))
    if (isScale(saved)) return saved
  } catch {
    /* private mode */
  }
  return 1
}

function applyScale(scale: Scale) {
  const fallback = () => {
    document.documentElement.style.zoom = String(scale)
  }
  try {
    void getCurrentWebview()
      .setZoom(scale)
      .then(() => {
        // A preview session may have left a CSS zoom behind; the native zoom
        // owns the scale once it succeeds.
        document.documentElement.style.zoom = ""
      })
      .catch(fallback)
  } catch {
    fallback()
  }
}

type ScaleValue = {
  scale: Scale
  setScale: (value: number) => void
  scales: readonly Scale[]
}

const Ctx = createContext<ScaleValue | null>(null)

export function ScaleProvider({ children }: { children: ReactNode }) {
  const [scale, setScaleState] = useState<Scale>(initialScale)

  const setScale = useCallback((value: number) => {
    if (!isScale(value)) return
    setScaleState(value)
    try {
      localStorage.setItem(KEY, String(value))
    } catch {
      /* private mode */
    }
  }, [])

  useLayoutEffect(() => {
    applyScale(scale)
  }, [scale])

  const value = useMemo(() => ({ scale, setScale, scales: SCALES }), [scale, setScale])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useScale(): ScaleValue {
  const value = useContext(Ctx)
  if (!value) throw new Error("useScale must be used inside ScaleProvider")
  return value
}
