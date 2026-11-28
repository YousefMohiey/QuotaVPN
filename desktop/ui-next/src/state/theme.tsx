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
  | "quota"
  | "catppuccin"
  | "tokyo-night"
  | "nord"
  | "dracula"
  | "gruvbox"
  | "rose-pine"
  | "one-dark"

export type ThemeMeta = {
  id: ThemeId
  labelKey: StrKey
  accent: string
  preview: { bg: string; panel: string; text: string }
}

/** The eight palettes. Quota is the branded flat dark; the rest are the
    palettes developers already live in, adapted to the same surface steps. */
export const THEMES: ThemeMeta[] = [
  {
    id: "quota",
    labelKey: "themeQuota",
    accent: "#1a72e8",
    preview: { bg: "#000000", panel: "#181818", text: "#fafafa" },
  },
  {
    id: "catppuccin",
    labelKey: "themeCatppuccin",
    accent: "#89b4fa",
    preview: { bg: "#11111b", panel: "#1e1e2e", text: "#cdd6f4" },
  },
  {
    id: "tokyo-night",
    labelKey: "themeTokyoNight",
    accent: "#7aa2f7",
    preview: { bg: "#16161e", panel: "#1f2335", text: "#c0caf5" },
  },
  {
    id: "nord",
    labelKey: "themeNord",
    accent: "#88c0d0",
    preview: { bg: "#2e3440", panel: "#3b4252", text: "#eceff4" },
  },
  {
    id: "dracula",
    labelKey: "themeDracula",
    accent: "#bd93f9",
    preview: { bg: "#1e1f29", panel: "#282a36", text: "#f8f8f2" },
  },
  {
    id: "gruvbox",
    labelKey: "themeGruvbox",
    accent: "#fe8019",
    preview: { bg: "#1d2021", panel: "#32302f", text: "#ebdbb2" },
  },
  {
    id: "rose-pine",
    labelKey: "themeRosePine",
    accent: "#ebbcba",
    preview: { bg: "#191724", panel: "#26233a", text: "#e0def4" },
  },
  {
    id: "one-dark",
    labelKey: "themeOneDark",
    accent: "#61afef",
    preview: { bg: "#1b1d23", panel: "#282c34", text: "#d7dae0" },
  },
]

export type FontMeta = {
  id: string
  label: string
  stack: string
  bundled?: boolean
}

/** The bundled faces are the ones the interface was designed against; the
    installed Windows faces remain for anyone who wants their own machine. */
const LATIN = '"Segoe UI", system-ui, "Cairo", "Twemoji Country Flags", sans-serif'
export const FONTS: FontMeta[] = [
  { id: "inter", label: "Inter", stack: `"Inter", ${LATIN}`, bundled: true },
  { id: "geist", label: "Geist", stack: `"Geist", "Inter", ${LATIN}`, bundled: true },
  { id: "manrope", label: "Manrope", stack: `"Manrope", ${LATIN}`, bundled: true },
  { id: "ibm-plex", label: "IBM Plex Sans", stack: `"IBM Plex Sans", "Inter", ${LATIN}`, bundled: true },
  { id: "archivo", label: "Archivo", stack: `"Archivo", ${LATIN}`, bundled: true },
  {
    id: "jetbrains",
    label: "JetBrains Mono",
    stack: '"JetBrains Mono", "Geist Mono", ui-monospace, monospace',
    bundled: true,
  },
  {
    id: "segoe-display",
    label: "Segoe UI Variable",
    stack: `"Segoe UI Variable Display", "Segoe UI Variable Text", "Segoe UI", ${LATIN}`,
  },
  { id: "segoe", label: "Segoe UI", stack: `"Segoe UI Variable Text", "Segoe UI", ${LATIN}` },
  { id: "calibri", label: "Calibri", stack: `"Calibri", ${LATIN}` },
  { id: "candara", label: "Candara", stack: `"Candara", ${LATIN}` },
  { id: "corbel", label: "Corbel", stack: `"Corbel", ${LATIN}` },
  { id: "tahoma", label: "Tahoma", stack: `"Tahoma", ${LATIN}` },
  { id: "trebuchet", label: "Trebuchet MS", stack: `"Trebuchet MS", ${LATIN}` },
  { id: "verdana", label: "Verdana", stack: `"Verdana", ${LATIN}` },
  { id: "arial", label: "Arial", stack: `"Arial", ${LATIN}` },
  { id: "georgia", label: "Georgia", stack: `"Georgia", ${LATIN}` },
  { id: "cambria", label: "Cambria", stack: `"Cambria", "Georgia", ${LATIN}` },
  { id: "palatino", label: "Palatino Linotype", stack: `"Palatino Linotype", "Georgia", ${LATIN}` },
  { id: "bahnschrift", label: "Bahnschrift", stack: `"Bahnschrift", ${LATIN}` },
  { id: "lucida", label: "Lucida Sans", stack: `"Lucida Sans Unicode", "Lucida Sans", ${LATIN}` },
  {
    id: "cascadia",
    label: "Cascadia Code",
    stack: '"Cascadia Code", "Geist Mono", ui-monospace, monospace',
  },
  { id: "consolas", label: "Consolas", stack: '"Consolas", "Geist Mono", ui-monospace, monospace' },
]

const KEY = "qc-theme"
const FONT_KEY = "qc-font"
const ACCENT_KEY = "qc-accent"

/** The old accent-only themes and the first full set migrate onto the new
    palettes instead of snapping back to the default. */
const LEGACY: Record<string, ThemeId> = {
  obsidian: "quota",
  serein: "quota",
  eclipse: "tokyo-night",
  slate: "nord",
  nightfall: "dracula",
  violet: "dracula",
  ember: "gruvbox",
  verdant: "catppuccin",
  mint: "catppuccin",
  afterglow: "rose-pine",
}

function isTheme(value: string | null | undefined): value is ThemeId {
  return THEMES.some((theme) => theme.id === value)
}

function resolveTheme(value: string | null | undefined): ThemeId | null {
  if (!value) return null
  if (isTheme(value)) return value
  return LEGACY[value] ?? null
}

function initialTheme(): ThemeId {
  if (typeof window === "undefined") return "quota"
  try {
    const query = new URLSearchParams(window.location.search).get("theme")
    const fromQuery = resolveTheme(query)
    if (fromQuery) return fromQuery
    const saved = localStorage.getItem(KEY)
    const fromSaved = resolveTheme(saved)
    if (fromSaved) return fromSaved
  } catch {
    /* private mode */
  }
  return "quota"
}

function initialFont(): string {
  if (typeof window === "undefined") return "inter"
  try {
    const saved = localStorage.getItem(FONT_KEY)
    if (saved && FONTS.some((font) => font.id === saved)) return saved
  } catch {
    /* private mode */
  }
  return "inter"
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

/** Black or white, whichever reads better on the accent. The deep brand blue
    and the red keep white; the pale presets take black. */
function readableOn(hex: string): string {
  const clean = hex.replace("#", "")
  const r = parseInt(clean.slice(0, 2), 16) / 255
  const g = parseInt(clean.slice(2, 4), 16) / 255
  const b = parseInt(clean.slice(4, 6), 16) / 255
  const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
  const luminance = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
  return luminance > 0.25 ? "#0b0d10" : "#ffffff"
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
