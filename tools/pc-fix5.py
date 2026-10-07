# The live graph moves from SVG to canvas: WebView2 skips stroking the path
# on its first raster and only paints it after unrelated pokes, which is why
# the line kept vanishing. Canvas paints when asked, every time.
import io, sys

p = r"C:\Tools\QuotaCards\desktop\ui-next\src\components\Hero.tsx"
s = io.open(p, "r", encoding="utf8", newline="").read()

START = "function Traffic({ rx, tx }: { rx: number; tx: number }) {"
END = "/** Bytes per second as the live number"

i = s.find(START)
j = s.find(END)
if i < 0 or j < 0:
    print("FAIL anchors", i, j)
    sys.exit(1)

NEW = '''function Traffic({ rx, tx }: { rx: number; tx: number }) {
  const { t } = useI18n()
  const hist = useRef<number[]>([])
  const last = useRef({ rx: 0, tx: 0, t: Date.now() })

  useEffect(() => {
    const now = Date.now()
    const dt = Math.max(0.2, (now - last.current.t) / 1000)
    const delta = Math.max(0, rx - last.current.rx) + Math.max(0, tx - last.current.tx)
    last.current = { rx, tx, t: now }
    if (last.current.rx || last.current.tx) {
      hist.current = [...hist.current.slice(-59), delta / dt]
    }
  }, [rx, tx])

  const values = hist.current.length ? hist.current : [0, 0]
  const { rate, peak } = useMemo(() => {
    const current = values[values.length - 1] || 0
    return { rate: rateText(current), peak: rateText(Math.max(0, ...values)) }
  }, [rx, tx])

  return (
    <div>
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-[11.5px] tracking-[0.04em] text-txt3">{t("liveLabel")}</span>
        <span className="text-[11.5px] tabular-nums text-txt3">
          {t("peakLbl")} {peak.num} {peak.unit}
        </span>
      </div>
      <div className="mt-1 flex items-baseline gap-1.5">
        <span className="text-[20px] font-semibold leading-none tabular-nums text-txt">{rate.num}</span>
        <span className="text-[12px] text-txt2">{rate.unit}</span>
      </div>
      <Spark values={values} />
    </div>
  )
}

/** The theme colour a var would give, resolved to a plain value. */
function cssVar(name: string, fallback: string): string {
  const probe = document.createElement("span")
  probe.style.color = `var(${name})`
  probe.style.display = "none"
  document.body.appendChild(probe)
  const v = getComputedStyle(probe).color
  probe.remove()
  return v && v !== "rgba(0, 0, 0, 0)" ? v : fallback
}

/** The rolling rate on a canvas. WebView2 never strokes the equivalent SVG
    path on its first raster (it only paints after unrelated DOM pokes, which
    is why the line kept disappearing), so the pixels are drawn here instead:
    a canvas paints exactly when it is asked to. */
function Spark({ values }: { values: number[] }) {
  const ref = useRef<HTMLCanvasElement | null>(null)

  const paint = useCallback(() => {
    const cv = ref.current
    if (!cv) return
    const dpr = window.devicePixelRatio || 1
    const w = cv.clientWidth
    const h = cv.clientHeight
    if (!w || !h) return
    cv.width = Math.round(w * dpr)
    cv.height = Math.round(h * dpr)
    const ctx = cv.getContext("2d")
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, w, h)
    const vals = values.length ? values : [0, 0]
    const max = Math.max(1, ...vals)
    const step = w / Math.max(1, vals.length - 1)
    const yOf = (v: number) => h - (v / max) * (h - 8) - 3
    const trace = () => {
      ctx.beginPath()
      vals.forEach((v, ix) => (ix ? ctx.lineTo(ix * step, yOf(v)) : ctx.moveTo(0, yOf(v))))
    }
    trace()
    ctx.lineTo(w, h)
    ctx.lineTo(0, h)
    ctx.closePath()
    ctx.fillStyle = cssVar("--brand-bg", "rgba(31, 89, 182, 0.14)")
    ctx.fill()
    trace()
    ctx.strokeStyle = cssVar("--brand", "#1f59b6")
    ctx.lineWidth = 2
    ctx.lineJoin = "round"
    ctx.lineCap = "round"
    ctx.stroke()
  }, [values])

  useEffect(() => {
    paint()
    const cv = ref.current
    if (!cv) return
    const ro = new ResizeObserver(() => paint())
    ro.observe(cv)
    return () => ro.disconnect()
  }, [paint])

  return <canvas ref={ref} className="mt-2 h-[56px] w-full" aria-hidden />
}

'''
s = s[:i] + NEW + s[j:]

# make sure useCallback is imported
import re as _re
m = _re.search(r'import \{([^}]*)\} from "react"', s)
if m and "useCallback" not in m.group(1):
    names = sorted([n.strip() for n in m.group(1).split(",") if n.strip()] + ["useCallback"])
    s = s[:m.start()] + "import { " + ", ".join(names) + ' } from "react"' + s[m.end():]
    print("useCallback imported")

io.open(p, "w", encoding="utf8", newline="").write(s)
print("traffic graph moved to canvas")
