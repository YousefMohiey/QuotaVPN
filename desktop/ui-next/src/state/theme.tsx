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

export type ThemeId =
  | "serein"
  | "eclipse"
  | "slate"
  | "nightfall"
  | "ember"
  | "verdant"
  | "afterglow"

export type ThemeMeta = {
  id: ThemeId
  labelKey: StrKey
  accent: string
  surface: string
}

/** Seven complete palettes. Each one changes the surfaces, lines, text and
    accent together; the old four only moved the accent. */
export const THEMES: ThemeMeta[] = [
  { id: "serein", labelKey: "themeSerein", accent: "#1a72e8", surface: "#000000" },
  { id: "eclipse", labelKey: "themeEclipse", accent: "#3b82f6", surface: "#0a1220" },
  { id: "slate", labelKey: "themeSlate", accent: "#7d93b2", surface: "#12161c" },
  { id: "nightfall", labelKey: "themeNightfall", accent: "#8b70f5", surface: "#14121f" },
  { id: "ember", labelKey: "themeEmber", accent: "#e87a43", surface: "#1a1410" },
  { id: "verdant", labelKey: "themeVerdant", accent: "#17b3a6", surface: "#0f1a17" },
  { id: "afterglow", labelKey: "themeAfterglow", accent: "#f472b6", surface: "#1c1118" },
]

export type FontMeta = {
  id: string
  label: string
  stack: string
  bundled?: boolean
}

/** Fonts that ship with Windows plus the two bundled families. The stack
    keeps Cairo and the flag face as fallbacks so Arabic and flags never
    disappear. */
const LATIN = '"Segoe UI", system-ui, "Cairo", "Twemoji Country Flags", sans-serif'
export const FONTS: FontMeta[] = [
  { id: "manrope", label: "Manrope", stack: `"Manrope", ${LATIN}`, bundled: true },
  { id: "archivo", label: "Archivo", stack: `"Archivo", ${LATIN}`, bundled: true },
  { id: "segoe", label: "Segoe UI", stack: `"Segoe UI Variable Text", "Segoe UI", ${LATIN}` },
  { id: "segoe-display", label: "Segoe UI Variable", stack: `"Segoe UI Variable Display", "Segoe UI Variable Text", "Segoe UI", ${LATIN}` },
  { id: "arial", label: "Arial", stack: `"Arial", ${LATIN}` },
  { id: "arial-black", label: "Arial Black", stack: `"Arial Black", "Arial", ${LATIN}` },
  { id: "calibri", label: "Calibri", stack: `"Calibri", ${LATIN}` },
  { id: "cambria", label: "Cambria", stack: `"Cambria", "Georgia", ${LATIN}` },
  { id: "candara", label: "Candara", stack: `"Candara", ${LATIN}` },
  { id: "corbel", label: "Corbel", stack: `"Corbel", ${LATIN}` },
  { id: "constantia", label: "Constantia", stack: `"Constantia", "Cambria", "Georgia", ${LATIN}` },
  { id: "georgia", label: "Georgia", stack: `"Georgia", ${LATIN}` },
  { id: "palatino", label: "Palatino Linotype", stack: `"Palatino Linotype", "Georgia", ${LATIN}` },
  { id: "tahoma", label: "Tahoma", stack: `"Tahoma", ${LATIN}` },
  { id: "trebuchet", label: "Trebuchet MS", stack: `"Trebuchet MS", ${LATIN}` },
  { id: "verdana", label: "Verdana", stack: `"Verdana", ${LATIN}` },
  { id: "franklin", label: "Franklin Gothic", stack: `"Franklin Gothic Medium", "Arial", ${LATIN}` },
  { id: "bahnschrift", label: "Bahnschrift", stack: `"Bahnschrift", ${LATIN}` },
  { id: "lucida", label: "Lucida Sans", stack: `"Lucida Sans Unicode", "Lucida Sans", ${LATIN}` },
  { id: "ebrima", label: "Ebrima", stack: `"Ebrima", ${LATIN}` },
  { id: "inkfree", label: "Ink Free", stack: `"Ink Free", "Segoe Print", ${LATIN}` },
  { id: "segoe-print", label: "Segoe Print", stack: `"Segoe Print", "Ink Free", ${LATIN}` },
  { id: "segoe-script", label: "Segoe Script", stack: `"Segoe Script", "Segoe Print", ${LATIN}` },
  { id: "comic", label: "Comic Sans MS", stack: `"Comic Sans MS", ${LATIN}` },
  { id: "impact", label: "Impact", stack: `"Impact", "Arial Black", ${LATIN}` },
  { id: "cascadia", label: "Cascadia Code", stack: `"Cascadia Code", "Consolas", "Geist Mono", ui-monospace, monospace` },
  { id: "consolas", label: "Consolas", stack: `"Consolas", "Geist Mono", ui-monospace, monospace` },
]

const KEY = "qc-theme"
const FONT_KEY = "qc-font"
const ACCENT_KEY = "qc-accent"

const LEGACY: Record<string, ThemeId> = {
  obsidian: "serein",
  ember: "ember",
  mint: "verdant",
  violet: "nightfall",
}

function isTheme(value: string | null | undefined): value is ThemeId {
  return THEMES.some((theme) => theme.id === value)
}

function initialTheme(): ThemeId {
  if (typeof window === "undefined") return "serein"
  try {
    const query = new URLSearchParams(window.location.search).get("theme")
    if (query && isTheme(query)) return query
    const saved = localStorage.getItem(KEY)
    if (saved && isTheme(saved)) return saved
    if (saved && LEGACY[saved]) return LEGACY[saved]
  } catch {
    /* private mode */
  }
  return "serein"
}

function initialFont(): string {
  if (typeof window === "undefined") return "manrope"
  try {
    const saved = localStorage.getItem(FONT_KEY)
    if (saved && FONTS.some((font) => font.id === saved)) return saved
  } catch {
    /* private mode */
  }
  return "manrope"
}

function normalizeAccent(value: string): string | null {
  const clean = value.trim().replace(/^#/, "")
  if (!/^[0-9a-f]{6}$/i.test(clean)) return null
  return `#${clean.toLowerCase()}`
}

function initialAccent(): string | null {
  if (typeof window === "undefined") return null
  try {
    const saved = localStorage.getItem(ACCENT_KEY)
    return saved ? normalizeAccent(saved) : null
  } catch {
    return null
  }
}

/** Black on a light accent, white on a dark one. */
function readableOn(hex: string): string {
  const clean = hex.replace("#", "")
  const r = parseInt(clean.slice(0, 2), 16) / 255
  const g = parseInt(clean.slice(2, 4), 16) / 255
  const b = parseInt(clean.slice(4, 6), 16) / 255
  const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
  const luminance = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
  return luminance > 0.52 ? "#0b0d10" : "#ffffff"
}

type ThemeValue = {
  theme: ThemeId
  setTheme: (id: ThemeId) => void
  themes: ThemeMeta[]
  font: string
  setFont: (id: string) => void
  fonts: FontMeta[]
  accent: string
  setAccent: (hex: string) => void
}

const Ctx = createContext<ThemeValue | null>(null)

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeId>(initialTheme)
  const [font, setFontState] = useState<string>(initialFont)
  const [accent, setAccentState] = useState<string | null>(initialAccent)

  const setTheme = useCallback((id: ThemeId) => {
    setThemeState(id)
    // A preset means its own accent. A custom colour is a tweak, not a second
    // theme, so it is cleared when the user picks a different preset.
    setAccentState(null)
    try {
      localStorage.setItem(KEY, id)
      localStorage.removeItem(ACCENT_KEY)
    } catch {
      /* private mode */
    }
  }, [])

  const setFont = useCallback((id: string) => {
    if (!FONTS.some((font) => font.id === id)) return
    setFontState(id)
    try {
      localStorage.setItem(FONT_KEY, id)
    } catch {
      /* private mode */
    }
  }, [])

  const setAccent = useCallback((hex: string) => {
    const next = normalizeAccent(hex)
    if (!next) return
    setAccentState(next)
    try {
      localStorage.setItem(ACCENT_KEY, next)
    } catch {
      /* private mode */
    }
  }, [])

  const activeTheme = THEMES.find((entry) => entry.id === theme) ?? THEMES[0]
  const activeAccent = accent ?? activeTheme.accent
  const activeFont = FONTS.find((entry) => entry.id === font) ?? FONTS[0]

  useLayoutEffect(() => {
    const root = document.documentElement
    root.dataset.theme = theme
    root.style.colorScheme = "dark"
    root.style.setProperty("--brand-base", activeAccent)
    root.style.setProperty("--on-brand", readableOn(activeAccent))
    root.style.setProperty("--font-ui", activeFont.stack)
    root.style.setProperty("--font-display", activeFont.stack)
  }, [theme, activeAccent, activeFont.stack])

  const value = useMemo(
    () => ({
      theme,
      setTheme,
      themes: THEMES,
      font,
      setFont,
      fonts: FONTS,
      accent: activeAccent,
      setAccent,
    }),
    [theme, setTheme, font, setFont, activeAccent, setAccent],
  )
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useTheme(): ThemeValue {
  const value = useContext(Ctx)
  if (!value) throw new Error("useTheme must be used inside ThemeProvider")
  return value
}
