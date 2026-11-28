import { useEffect, useState } from "react"
import { useTheme } from "@/state/theme"

/** The shipping icon, unchanged. Only the royal-blue tail is recoloured: each
    pixel keeps its shading and its anti-aliased edge while its hue and
    saturation move to the active accent. The black tile, the white Q and the
    tiny top highlight stay exactly as drawn. */
const ICON_SRC = "./icon.png"
const SOURCE_HUE_LOW = 195
const SOURCE_HUE_HIGH = 265
const SOURCE_SAT = 0.84

type Hsl = { h: number; s: number; l: number }

function hexToHsl(hex: string): Hsl {
  const value = hex.replace("#", "")
  const r = parseInt(value.slice(0, 2), 16) / 255
  const g = parseInt(value.slice(2, 4), 16) / 255
  const b = parseInt(value.slice(4, 6), 16) / 255
  return rgbToHsl(r, g, b)
}

function rgbToHsl(r: number, g: number, b: number): Hsl {
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const d = max - min
  const l = (max + min) / 2
  let h = 0
  if (d !== 0) {
    if (max === r) h = 60 * (((g - b) / d) % 6)
    else if (max === g) h = 60 * ((b - r) / d + 2)
    else h = 60 * ((r - g) / d + 4)
  }
  if (h < 0) h += 360
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
  return { h, s, l }
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const c = (1 - Math.abs(2 * l - 1)) * s
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = l - c / 2
  let r = 0
  let g = 0
  let b = 0
  if (h < 60) [r, g, b] = [c, x, 0]
  else if (h < 120) [r, g, b] = [x, c, 0]
  else if (h < 180) [r, g, b] = [0, c, x]
  else if (h < 240) [r, g, b] = [0, x, c]
  else if (h < 300) [r, g, b] = [x, 0, c]
  else [r, g, b] = [c, 0, x]
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)]
}

const tuned = new Map<string, Promise<string>>()
let source: Promise<HTMLImageElement> | null = null

function loadIcon(): Promise<HTMLImageElement> {
  if (!source) {
    source = new Promise((resolve, reject) => {
      const image = new Image()
      image.onload = () => resolve(image)
      image.onerror = () => reject(new Error("icon load failed"))
      image.src = ICON_SRC
    })
  }
  return source
}

function tint(accent: string): Promise<string> {
  const cached = tuned.get(accent)
  if (cached) return cached

  const job = loadIcon().then((image) => {
    const canvas = document.createElement("canvas")
    canvas.width = image.naturalWidth
    canvas.height = image.naturalHeight
    const context = canvas.getContext("2d")
    if (!context) throw new Error("no canvas")
    context.drawImage(image, 0, 0)
    const frame = context.getImageData(0, 0, canvas.width, canvas.height)
    const target = hexToHsl(accent)

    for (let i = 0; i < frame.data.length; i += 4) {
      const r = frame.data[i] / 255
      const g = frame.data[i + 1] / 255
      const b = frame.data[i + 2] / 255
      const pixel = rgbToHsl(r, g, b)
      if (
        pixel.s > 0.1 &&
        pixel.h >= SOURCE_HUE_LOW &&
        pixel.h <= SOURCE_HUE_HIGH &&
        b > r + 0.04
      ) {
        // A blended edge keeps its lower saturation, so the anti-aliased rim
        // fades into the tile exactly as it did in the original.
        const strength = target.s * Math.min(1, pixel.s / SOURCE_SAT)
        const [nr, ng, nb] = hslToRgb(target.h, strength, pixel.l)
        frame.data[i] = nr
        frame.data[i + 1] = ng
        frame.data[i + 2] = nb
      }
    }

    context.putImageData(frame, 0, 0)
    return canvas.toDataURL("image/png")
  })

  tuned.set(accent, job)
  return job
}

export function QuotaMark({ className }: { className?: string }) {
  const { accent } = useTheme()
  const [src, setSrc] = useState<string>()

  useEffect(() => {
    let alive = true
    tint(accent)
      .then((url) => {
        if (alive) setSrc(url)
      })
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [accent])

  return <img src={src ?? ICON_SRC} alt="" aria-hidden className={className} />
}
