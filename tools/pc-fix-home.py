import io

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

# ============================================================ index.css =====
P = SRC + r"\index.css"
t = load(P)
t = sub(t, '''.dial-arc {
  fill: none;
  stroke: var(--brand-vivid);
  stroke-width: 2.6;
  stroke-linecap: round;
  stroke-dasharray: 148 148;
  transform: rotate(-90deg) scale(-1, 1);
  transform-origin: center;
  transition: stroke 0.3s ease;
}''',
'''.dial-arc {
  fill: none;
  stroke: var(--brand-vivid);
  stroke-width: 3;
  stroke-linecap: round;
  stroke-dasharray: 148 148;
  /* rotate(90) puts the swept half on the LEFT, like the mockup: the
     dash starts at 3 o'clock, so a clockwise quarter lands it 12-to-6. */
  transform: rotate(90deg);
  transform-origin: center;
  transition: stroke 0.3s ease;
}''')
save(P, t); print("index.css ok")

# ============================================================ Dial =========
P = SRC + r"\components\Dial.tsx"
t = load(P)
t = sub(t, '''      className={cn(
        "relative grid size-[172px] place-items-center rounded-full border bg-white/[0.03] shadow-[inset_0_1px_0_rgb(255_255_255/0.07)] transition-colors disabled:opacity-70",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.18)] text-txt hover:border-[var(--brand-line)]",
      )}''',
'''      className={cn(
        "relative grid size-[172px] place-items-center rounded-full border transition-[background-color,border-color,box-shadow] duration-300 disabled:opacity-70",
        /* the glass: a light translucent pane, frosted by the scene behind
           it, with a lit top edge and a soft floor shadow */
        "bg-[rgb(255_255_255/0.055)] backdrop-blur-[16px] backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.16),inset_0_-18px_36px_rgb(0_0_0/0.22),0_18px_44px_rgb(0_0_0/0.35)]",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.16)] text-txt hover:border-[var(--brand-line)] hover:bg-[rgb(255_255_255/0.08)]",
      )}''')
t = sub(t, '<Power className="size-[34px]" strokeWidth={1.6} aria-hidden />',
        '<Power className="size-[38px]" strokeWidth={1.7} aria-hidden />')
save(P, t); print("dial ok")

# ============================================================ Hero =========
P = SRC + r"\components\Hero.tsx"
t = load(P)
lines = t.split("\n")

# imports + the doc comment
t = sub(t, 'import { ArrowUpDown, Globe, MapPin, type LucideIcon } from "lucide-react"',
        'import { Activity, Globe, Network, type LucideIcon } from "lucide-react"')
t = sub(t, '''/**
 * The connection card: the dial parks left the moment Connect is pressed
 * and the reading column takes its place behind the same hairline divider
 * the Valorant cards use. The footer strip carries the three connection
 * facts, so the page is two calm cards instead of a stack of boxes.
 */''',
'''/**
 * The connection card: the glass dial on the left, the state and the big
 * action word beside it. The live session (rate, graph, timers) opens a
 * full-width block under the row once connected, and the three connection
 * facts sit along the card's floor. Nothing here is sized to the window,
 * so no column ever gets crushed.
 */''')

# the return block: splice by line markers
lines = t.split("\n")
start = None
for i, l in enumerate(lines):
    if l == '  return (' and i > 90:
        start = i
        break
assert start is not None, "return not found"
end = None
for j in range(start, len(lines)):
    if lines[j] == '  )' and lines[j + 1] == '}':
        end = j
        break
assert end is not None, "end not found"

new_ret = '''  return (
    <section className="flex flex-1 flex-col justify-center rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
      <div className="flex min-h-[180px] items-center gap-6">
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
            <div className="mt-2 max-w-[520px] text-[11.5px] leading-snug text-txt3">{status}</div>
          ) : null}
        </div>
      </div>

      <AnimatePresence>
        {connected && (
          <motion.div
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.36, ease: EASE_OUT, delay: 0.12 }}
          >
            {/* the live session, full width so the graph gets real room */}
            <div className="mt-4 flex items-stretch gap-4 border-t border-line pt-4">
              <div className="flex min-w-0 flex-1 flex-col justify-center">
                <Traffic rx={rx} tx={tx} />
              </div>
              <div className="flex w-[220px] shrink-0 flex-col justify-center gap-2">
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

      {/* the connection facts: a calm strip along the card's floor */}
      <div className="mt-4 grid grid-cols-3 gap-4 border-t border-line pt-4">
        <Fact icon={Globe} label={t("srvLocation")} value={placeText} />
        <Fact icon={Network} label={t("yourIp")} value={ipText} />
        <Fact
          icon={Activity}
          label={t("statusLbl")}
          value={connected ? t("connected") : t("notConnected")}
          dot={connected}
        />
      </div>
    </section>
  )'''

lines = lines[:start] + new_ret.split("\n") + lines[end + 1:]
t = "\n".join(lines)

# the Fact tile: no fixed width now that it lives in a spread row
t = sub(t, '''  return (
    <div className="flex w-[114px] min-w-0 items-center gap-1.5">
      <span className="grid size-[24px] shrink-0 place-items-center rounded-[8px] border border-line bg-white/[0.03] text-brand-strong">
        <Icon className="size-[12px]" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="whitespace-nowrap text-[10.5px] text-txt3">{label}</div>
        <div className="mt-0.5 flex items-center gap-1 text-[11px] text-txt">
          {dot !== undefined && (
            <span className={cn("size-[5px] shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-[var(--red)]")} aria-hidden />
          )}''',
'''  return (
    <div className="flex min-w-0 items-center gap-2.5">
      <span className="grid size-[30px] shrink-0 place-items-center rounded-[9px] border border-line bg-white/[0.03] text-brand-strong">
        <Icon className="size-[15px]" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="whitespace-nowrap text-[11px] text-txt3">{label}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[12.5px] text-txt">
          {dot !== undefined && (
            <span className={cn("size-[6px] shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-[var(--red)]")} aria-hidden />
          )}''')
save(P, t); print("hero ok")

# ============================================================ Home =========
P = SRC + r"\screens\Home.tsx"
t = load(P)
t = sub(t, '    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-3">',
        '    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">')
t = sub(t, '      <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">',
        '      <section className="flex min-h-[200px] flex-1 flex-col justify-center rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">')
t = sub(t, '          <div className="flex min-w-0 flex-1 flex-col">',
        '          <div className="flex min-w-0 flex-1 flex-col justify-center">')
save(P, t); print("home ok")

# ============================================================ App ==========
P = SRC + r"\App.tsx"
t = load(P)
t = sub(t, '<div className="relative mx-auto w-full max-w-[900px] px-8 pb-6 pt-14">',
        '<div className="relative mx-auto h-full w-full max-w-[900px] px-8 pb-6 pt-14">')
t = sub(t, '''              <motion.div
                key={tab}
                initial={{ opacity: 0, y: 8 }}''',
'''              <motion.div
                key={tab}
                className="h-full"
                initial={{ opacity: 0, y: 8 }}''')
save(P, t); print("app ok")

# ============================================================ i18n =========
P = SRC + r"\lib\i18n\en.ts"
t = load(P)
t = sub(t, '  "kindGamerz": "Gamerz / PS",', '  "kindGamerz": "Gamerz",')
save(P, t); print("en ok")
P = SRC + r"\lib\i18n\ar.ts"
t = load(P)
t = sub(t, '  "kindGamerz": "جيمرز / PS",', '  "kindGamerz": "جيمرز",')
save(P, t); print("ar ok")

# ============================================================ phone arc ====
P = r"C:\Tools\QuotaCards\android\tauri-app\ui\style.css"
t = load(P)
t = sub(t, '.dial-arc circle { fill: none; stroke: var(--accent-strong); stroke-width: 3; stroke-linecap: round; stroke-dasharray: 148 148; transform: rotate(-90deg) scale(-1, 1); transform-origin: center; }',
        '.dial-arc circle { fill: none; stroke: var(--accent-strong); stroke-width: 3; stroke-linecap: round; stroke-dasharray: 148 148; transform: rotate(90deg); transform-origin: center; }')
save(P, t); print("phone arc ok")
