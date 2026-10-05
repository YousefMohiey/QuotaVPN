import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p):
    return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))

def sub(t, old, new, cnt=1):
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:100])
    return t.replace(old, new, cnt)

# ============================================================ Hero =========
P = SRC + r"\components\Hero.tsx"
t = load(P)
lines = t.split("\n")

old_ret = '''  // The dial parks left from the moment Connect is pressed, and starts its
  // way back the moment Disconnect is pressed, not when the engine answers.
  const active = phase === "connecting" || phase === "on"
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""

  // The info panel waits for the dial to land first. Mounting both at once
  // is what made the motion feel fast and rough, so the panel fades in
  // only after the slide has had room to travel.
  const [showInfo, setShowInfo] = useState(false)
  useEffect(() => {
    if (!active) {
      setShowInfo(false)
      return
    }
    const id = window.setTimeout(() => setShowInfo(true), 250)
    return () => window.clearTimeout(id)
  }, [active])

  // Disconnect plays in order too: the panel exits first and the dial only
  // starts home once the exit is nearly done, never both at once.
  const [dialLeft, setDialLeft] = useState(false)
  useEffect(() => {
    if (active) {
      setDialLeft(true)
      return
    }
    if (phase === "stopping") {
      // Short hold only: the panel exit leads by a beat, then the dial
      // answers at once. A long hold here reads as a dead pause.
      const id = window.setTimeout(() => setDialLeft(false), 140)
      return () => window.clearTimeout(id)
    }
    setDialLeft(false)
  }, [active, phase])'''
new_ret = '''  // The dial sits left in every state, like the mockup: the word column
  // beside it tells the state and carries the session once connected.
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""
  const hostLine = [name, displayHost(card?.sni)].filter(Boolean).join(" \u00b7 ")'''
t = sub(t, old_ret, new_ret)
lines = t.split("\n")

# The JSX return: line-spliced so the exact whitespace cannot bite.
start = None
end = None
for i, l in enumerate(lines):
    if l.strip() == 'return (' and i > 100:
        start = i + 1
    if start is not None and l == '    <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">':
        start = i
        break
assert start is not None, "section start not found"
for j in range(start, len(lines)):
    if lines[j] == '    </section>':
        end = j
        break
assert end is not None, "section end not found"

new_jsx = '''    <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
      <div className="flex min-h-[190px] items-center gap-6">
        <div className="flex shrink-0 items-center justify-center">
          <Dial state={state} onClick={toggle} disabled={busy} />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span
              aria-hidden
              className={cn(
                "size-1.5 rounded-full",
                connected
                  ? "bg-[var(--green)]"
                  : state === "connecting"
                    ? "pulse-dot bg-[var(--brand)]"
                    : "bg-[var(--red)]",
              )}
            />
            <span className="text-[12.5px] text-txt2">
              {connected ? t("connected") : state === "connecting" ? t("working") : t("notConnected")}
            </span>
          </div>

          <h1 className="mt-1.5 truncate text-[22px] font-semibold tracking-[-0.01em] text-txt">
            {connected ? t("disconnect") : state === "connecting" ? t("working") : t("connect")}
          </h1>
          <p className="mt-1 truncate text-[12.5px] text-txt3" dir="auto">
            {connected ? hostLine : t("clickToConnect")}
          </p>
          {status ? (
            <div className="mt-2 max-w-[430px] text-[11.5px] leading-snug text-txt3">{status}</div>
          ) : null}

          <AnimatePresence>
            {connected && (
              <motion.div
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.36, ease: EASE_OUT, delay: 0.12 }}
              >
                <div className="mt-3 flex items-stretch gap-3">
                  <div className="flex min-w-0 flex-1 flex-col justify-center">
                    <Traffic rx={rx} tx={tx} />
                  </div>
                  <div className="flex w-[200px] shrink-0 flex-col justify-center gap-2">
                    <StatTile
                      label={t("sessLabel")}
                      value={sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
                      sub={"\u2068\u2193 " + fmtBytes(rx) + "\u2069   \u2068\u2191 " + fmtBytes(tx) + "\u2069"}
                    />
                    <StatTile label={t("yourIp")} value={serverAddr || displayHost(serverIp)} sub={exit?.isp || undefined} />
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* the connection facts, one quiet row behind the same hairline */}
        <div className="flex shrink-0 items-center gap-5 border-l border-line pl-6">
          <Fact icon={Globe} label={t("srvLocation")} value={placeText} />
          <Fact icon={MapPin} label={t("yourIp")} value={ipText} />
          <Fact
            icon={ArrowUpDown}
            label={t("statusLbl")}
            value={connected ? t("connected") : t("notConnected")}
            dot={connected}
          />
        </div>
      </div>
    </section>'''

lines = lines[:start] + new_jsx.split("\n") + lines[end + 1:]
t = "\n".join(lines)

# The facts' idle dot goes red, like the mockup's, and each fact gets a
# fixed column so labels never crush.
t = sub(t, '''          {dot !== undefined && (
            <span className={cn("size-1.5 shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-txt3")} aria-hidden />
          )}''',
'''          {dot !== undefined && (
            <span className={cn("size-1.5 shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-[var(--red)]")} aria-hidden />
          )}''')
t = sub(t, '''  return (
    <div className="flex min-w-0 items-center gap-3">
      <span className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-line bg-white/[0.03] text-brand-strong">''',
'''  return (
    <div className="flex w-[118px] min-w-0 items-center gap-2.5">
      <span className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-line bg-white/[0.03] text-brand-strong">''')
# SPRING is no longer used.
t = sub(t, 'const EASE_OUT = [0.1, 0.9, 0.2, 1] as const\nconst SPRING = { type: "spring", stiffness: 320, damping: 34 } as const',
        'const EASE_OUT = [0.1, 0.9, 0.2, 1] as const')
save(P, t)
print("hero ok")

# ============================================================ Segmented ====
P = SRC + r"\components\Segmented.tsx"
t = load(P)
t = sub(t, '''  onChange,
  className,
}: {
  id: string
  value: T
  options: Array<{ value: T; label: string }>
  onChange: (v: T) => void
  className?: string
}) {
  return (
    <div className={cn("inline-flex rounded-[13px] border border-line bg-white/[0.02] p-[3px]", className)}>''',
'''  onChange,
  className,
  fill,
}: {
  id: string
  value: T
  options: Array<{ value: T; label: string }>
  onChange: (v: T) => void
  className?: string
  fill?: boolean
}) {
  return (
    <div className={cn("rounded-[13px] border border-line bg-white/[0.02] p-[3px]", fill ? "flex w-full" : "inline-flex", className)}>''')
t = sub(t, '''            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",''',
'''            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",
              fill && "flex-1",''')
save(P, t)
print("segmented fill ok")

# ============================================================ Home =========
P = SRC + r"\screens\Home.tsx"
t = load(P)
t = sub(t, '''                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
                        : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",''',
'''                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)] shadow-[0_0_0_1px_var(--brand-line),0_8px_22px_rgb(31_89_182/0.28)]"
                        : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",''')
t = sub(t, '            <ControlRow label={t("domainSni")}>',
        '            <ControlRow label={t("domainSni")} icon={ServerIcon}>')
t = sub(t, '            <ControlRow label={t("routing")}>',
        '            <ControlRow label={t("routing")} icon={LayoutGrid}>')
t = sub(t, '''                  id="transport"
                  value={transport}
                  onChange={setTransport}''',
'''                  id="transport"
                  value={transport}
                  onChange={setTransport}
                  fill''')
t = sub(t, '            <ControlRow label={t("transport")} align="start">',
        '            <ControlRow label={t("transport") + " \\u24d8"} align="start">')
t = sub(t, '''/** One control row: a quiet label, then the control that fills the rest. */
function ControlRow({
  label,
  children,
  align = "center",
}: {
  label: string
  children: ReactNode
  align?: "center" | "start"
}) {
  return (
    <div className={cn("flex gap-3", align === "center" ? "items-center" : "items-start")}>
      <div className="w-[96px] shrink-0 pt-[2px] text-[12.5px] text-txt2">{label}</div>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  )
}''',
'''/** One control row: a quiet label with its glyph, then the control. */
function ControlRow({
  label,
  children,
  align = "center",
  icon: Icon,
}: {
  label: string
  children: ReactNode
  align?: "center" | "start"
  icon?: LucideIcon
}) {
  return (
    <div className={cn("flex gap-3", align === "center" ? "items-center" : "items-start")}>
      <div className="flex w-[104px] shrink-0 items-center gap-2 pt-[2px] text-[12.5px] text-txt2">
        {Icon && (
          <span className="grid size-[26px] shrink-0 place-items-center rounded-[8px] border border-line bg-white/[0.03] text-txt2">
            <Icon className="size-[13px]" aria-hidden />
          </span>
        )}
        <span className="min-w-0 truncate">{label}</span>
      </div>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  )
}''')
t = sub(t, 'import { ChevronDown, ChevronRight, Gamepad2, TriangleAlert, Tv } from "lucide-react"',
        'import { ChevronDown, ChevronRight, Gamepad2, LayoutGrid, Server as ServerIcon, TriangleAlert, Tv, type LucideIcon } from "lucide-react"')
save(P, t)
print("home ok")

# ============================================================ i18n =========
P = SRC + r"\lib\i18n\en.ts"
t = load(P)
t = sub(t, '  "transport": "Connection",', '  "transport": "Connection type",')
t = sub(t, '  "setupBody": "Pick what this VPN is for, the server it rides, and how it carries your traffic.",',
        '  "setupBody": "Choose a preset or configure your connection.",')
t = sub(t, '  "talking": "Talking to the server.",',
        '  "talking": "Talking to the server.",\n  "clickToConnect": "Click to connect to QuotaVPN",')
save(P, t)

P = SRC + r"\lib\i18n\ar.ts"
t = load(P)
t = sub(t, '  "transport": "الاتصال",', '  "transport": "نوع الاتصال",')
t = sub(t, '  "setupBody": "اختر استخدام الـVPN، والخادم الذي يمر عليه، وطريقة نقل الترافيك.",',
        '  "setupBody": "اختر حزمة أو اضبط اتصالك.",')
t = sub(t, '  "talking": "جارٍ التواصل مع الخادم…",',
        '  "talking": "جارٍ التواصل مع الخادم…",\n  "clickToConnect": "اضغط للاتصال بـQuotaVPN",')
save(P, t)
print("i18n ok")
