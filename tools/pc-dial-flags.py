import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# ===================================================== flags helper ========
P = SRC + r"\lib\flags.ts"
io.open(P, "w", encoding="utf-8", newline="").write('''/** Regional-indicator pair for a two letter country code. */
const ri = (cc: string) => String.fromCodePoint(...[...cc].map((c) => 0x1f1e6 + c.charCodeAt(0) - 65))

/** The countries the speed endpoints and the exit lookup report. */
const CC: Record<string, string> = {
  italy: "IT", germany: "DE", netherlands: "NL", france: "FR", spain: "ES",
  portugal: "PT", switzerland: "CH", austria: "AT", belgium: "BE", ireland: "IE",
  "united kingdom": "GB", uk: "GB", england: "GB", "united states": "US",
  usa: "US", canada: "CA", mexico: "MX", brazil: "BR", argentina: "AR",
  chile: "CL", colombia: "CO", sweden: "SE", norway: "NO", denmark: "DK",
  finland: "FI", poland: "PL", czechia: "CZ", "czech republic": "CZ",
  romania: "RO", hungary: "HU", greece: "GR", ukraine: "UA", turkey: "TR",
  russia: "RU", egypt: "EG", "south africa": "ZA", nigeria: "NG", kenya: "KE",
  morocco: "MA", "saudi arabia": "SA", "united arab emirates": "AE",
  uae: "AE", qatar: "QA", israel: "IL", india: "IN", singapore: "SG",
  japan: "JP", "south korea": "KR", china: "CN", "hong kong": "HK",
  taiwan: "TW", indonesia: "ID", malaysia: "MY", thailand: "TH",
  vietnam: "VN", philippines: "PH", australia: "AU", "new zealand": "NZ",
}

/** "Milan, Italy" -> "flag Milan, Italy". Empty prefix when unknown. */
export function flagFor(place: string): string {
  const country = place.split(",").pop()?.trim().toLowerCase() ?? ""
  const cc = CC[country]
  return cc ? ri(cc) + " " : ""
}
''')
print("flags helper ok")

# ===================================================== dial word inside ====
P = SRC + r"\components\Dial.tsx"
t = load(P)
t = sub(t, '''      <span className="flex flex-col items-center gap-2.5">
        <Power className="size-[38px]" strokeWidth={1.7} aria-hidden />
      </span>''',
'''      <span className="flex flex-col items-center gap-2">
        <Power className="size-[38px]" strokeWidth={1.7} aria-hidden />
        <span className="text-[15px] font-semibold tracking-[-0.01em]">{label}</span>
      </span>''')
save(P, t); print("dial label inside ok")

# ===================================================== hero ================
P = SRC + r"\components\Hero.tsx"
t = load(P)

# the idle text block, its drop math and the text state all go: the dial
# carries its own word now, so nothing sits under it and nothing reflows.
t = sub(t, '''  const [textOpen, setTextOpen] = useState(true)
  useEffect(() => {
    if (dialLeft) {
      setTextOpen(false)
      return
    }
    // the text waits for the dial to land before it opens
    const id = window.setTimeout(() => setTextOpen(true), 560)
    return () => window.clearTimeout(id)
  }, [dialLeft])

  const textRef = useRef<HTMLDivElement>(null)
  const [textH, setTextH] = useState(0)
  useEffect(() => {
    const el = textRef.current
    if (!el) return
    const measure = () => setTextH(el.offsetHeight)
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  // The text keeps its space in flow while parked, so the card height never
  // moves and the glide never fights a reflow. The dial drops by half the
  // block it no longer shares with, landing centered on the zone.
  const dialDrop = (18 + textH) / 2

  useTick(1000, connected)''',
'''  useTick(1000, connected)''')

t = sub(t, '''          <motion.div layout transition={SPRING} className="flex shrink-0 flex-col items-center text-center">
            <motion.div
              animate={{ y: dialLeft ? dialDrop : 0 }}
              transition={SPRING}
              className="flex flex-col items-center"
            >
              <Dial state={state} onClick={toggle} disabled={busy} />
            </motion.div>
            <motion.div
              initial={false}
              animate={{ opacity: textOpen ? 1 : 0 }}
              transition={{ duration: 0.3, ease: EASE_OUT }}
              className={cn(!textOpen && "pointer-events-none")}
            >
              <div ref={textRef} className="mt-[18px] flex flex-col items-center">
                <button
                  type="button"
                  onClick={toggle}
                  className="text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong"
                >
                  {t("connect")}
                </button>
              </div>
            </motion.div>
          </motion.div>''',
'''          <motion.div layout transition={SPRING} className="flex shrink-0 items-center justify-center">
            <Dial state={state} onClick={toggle} disabled={busy} />
          </motion.div>''')

# the zone loses its word: the dial says the action, the zone reads the state.
t = sub(t, '''                <h1 className="mt-2 truncate text-[24px] font-semibold tracking-[-0.01em] text-txt">
                  {connected ? t("disconnect") : t("working")}
                </h1>
                <p className="mt-1 truncate text-[13px] text-txt3" dir="auto">
                  {connected ? hostLine : t("talking")}
                </p>''',
'''                <p className="mt-2 truncate text-[15px] font-medium text-txt" dir="auto">
                  {connected ? hostLine : t("talking")}
                </p>''')

# flags in front of the server location, both places it shows.
t = sub(t, '''  const placeText = connected ? exit?.place || srvPlace || displayHost(serverIp) : srvPlace || "-"''',
'''  const placeText = connected ? exit?.place || srvPlace || displayHost(serverIp) : srvPlace || "-"
  const placeShown = flagFor(placeText) + placeText''')
t = sub(t, '<StatTile label={t("srvLocation")} value={placeText} sub={serverAddr || ipText} />',
        '<StatTile label={t("srvLocation")} value={placeShown} sub={serverAddr || ipText} />')
t = sub(t, '<Fact icon={Globe} label={t("srvLocation")} value={placeText} />',
        '<Fact icon={Globe} label={t("srvLocation")} value={placeShown} />')
t = sub(t, 'import { cn } from "@/lib/utils"',
        'import { cn } from "@/lib/utils"\nimport { flagFor } from "@/lib/flags"')
save(P, t); print("hero ok")

# ===================================================== css =================
P = SRC + r"\index.css"
t = load(P)
t = sub(t, '@font-face {\n  font-family: "Cairo";\n  src: url("/fonts/cairo-ar.woff2")',
'''/* Twemoji Country Flags (CC-BY 4.0, github.com/twitter/twemoji): Windows
   ships no flag glyphs, so regional-indicator pairs render as bare letters
   without this face. The unicode-range keeps it to the flags alone. */
@font-face {
  font-family: "Twemoji Country Flags";
  src: url("/fonts/TwemojiCountryFlags.woff2") format("woff2");
  font-weight: 400;
  font-style: normal;
  font-display: swap;
  unicode-range: U+1F1E6-1F1FF;
}

@font-face {
  font-family: "Cairo";
  src: url("/fonts/cairo-ar.woff2")''')
t = sub(t, '--font-sans: "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, Roboto, "Cairo", sans-serif;',
        '--font-sans: "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, Roboto, "Cairo", "Twemoji Country Flags", sans-serif;')
t = sub(t, 'font-family: "Cairo", "Segoe UI", system-ui, sans-serif;',
        'font-family: "Cairo", "Twemoji Country Flags", "Segoe UI", system-ui, sans-serif;')
save(P, t); print("css ok")
