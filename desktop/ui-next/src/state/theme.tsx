import {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react"
import type { StrKey } from "@/lib/i18n"

export type ThemeId = "obsidian" | "ember" | "mint" | "violet"

export type ThemeMeta = {
  id: ThemeId
  labelKey: StrKey
  accent: string
  surface: string
}

export const THEMES: ThemeMeta[] = [
  { id: "obsidian", labelKey: "themeObsidian", accent: "#6C8BFF", surface: "#111318" },
  { id: "ember", labelKey: "themeEmber", accent: "#E87A43", surface: "#17120F" },
  { id: "mint", labelKey: "themeMint", accent: "#17B3A6", surface: "#0F1715" },
  { id: "violet", labelKey: "themeViolet", accent: "#8B70F5", surface: "#120F1B" },
]

const KEY = "qc-theme"

function isTheme(value: string | null | undefined): value is ThemeId {
  return value === "obsidian" || value === "ember" || value === "mint" || value === "violet"
}

function initialTheme(): ThemeId {
  if (typeof window === "undefined") return "obsidian"
  try {
    const query = new URLSearchParams(window.location.search).get("theme")
    if (isTheme(query)) return query
    const saved = localStorage.getItem(KEY)
    if (isTheme(saved)) return saved
  } catch {
    /* private mode */
  }
  return "obsidian"
}

type ThemeValue = {
  theme: ThemeId
  setTheme: (id: ThemeId) => void
  themes: ThemeMeta[]
}

const Ctx = createContext<ThemeValue | null>(null)

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeId>(initialTheme)

  const setTheme = useCallback((id: ThemeId) => {
    setThemeState(id)
    try {
      localStorage.setItem(KEY, id)
    } catch {
      /* private mode */
    }
  }, [])

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme
    document.documentElement.style.colorScheme = "dark"
  }, [theme])

  const value = useMemo(() => ({ theme, setTheme, themes: THEMES }), [theme, setTheme])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useTheme(): ThemeValue {
  const value = useContext(Ctx)
  if (!value) throw new Error("useTheme must be used inside ThemeProvider")
  return value
}
