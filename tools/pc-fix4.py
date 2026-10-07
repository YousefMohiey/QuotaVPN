# Owner pass 4:
#  - The connected reading zone follows his edit: the Session and Server location
#    tiles move up beside the Connected line, Live traffic keeps the left, and the
#    sparkline stretches the full width and stands taller.
#  - The graph strokes stay crisp at any width (non-scaling-stroke).
#  - The info glyph leaves the Connection type label.
import io, sys

ROOT = r"C:\Tools\QuotaCards"
p = ROOT + r"\desktop\ui-next\src\components\Hero.tsx"
s = io.open(p, "r", encoding="utf8", newline="").read()

START = '''                <div className="flex items-center gap-2">
                  <span
                    aria-hidden
                    className={cn(
                      "size-1.5 rounded-full",
                      connected ? "bg-[var(--green)]" : "pulse-dot bg-[var(--brand)]",
                    )}
                  />
                  <span className="text-[12.5px] text-txt2">
                    {connected ? t("connected") : t("working")}
                  </span>
                </div>

                <p className="mt-2 truncate text-[15px] font-medium text-txt" dir="auto">
                  {connected ? hostLine : t("talking")}
                </p>
                {status ? (
                  <div className="mt-2 max-w-[520px] select-text text-[11.5px] leading-snug text-txt3">{status}</div>
                ) : null}

                <AnimatePresence>
                  {connected && (
                    <motion.div
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.36, ease: EASE_OUT, delay: 0.12 }}
                    >
                      <div className="mt-5 flex items-stretch gap-4">
                        <div className="flex min-w-0 flex-1 flex-col justify-center">
                          <Traffic rx={rx} tx={tx} />
                        </div>
                        <div className="flex w-[212px] shrink-0 flex-col justify-center gap-2">
                          <StatTile
                            label={t("sessLabel")}
                            value={sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
                            sub={"\u2068\u2193 " + fmtBytes(rx) + "\u2069   \u2068\u2191 " + fmtBytes(tx) + "\u2069"}
                          />
                          <StatTile label={t("srvLocation")} value={placeShown} sub={serverAddr || ipText} />
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>'''

NEW = '''                {/* the owner's own arrangement: the reading tiles ride the
                    top right, and Live traffic keeps the left for itself */}
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        aria-hidden
                        className={cn(
                          "size-1.5 rounded-full",
                          connected ? "bg-[var(--green)]" : "pulse-dot bg-[var(--brand)]",
                        )}
                      />
                      <span className="text-[12.5px] text-txt2">
                        {connected ? t("connected") : t("working")}
                      </span>
                    </div>
                    <p className="mt-2 truncate text-[15px] font-medium text-txt" dir="auto">
                      {connected ? hostLine : t("talking")}
                    </p>
                  </div>
                  {connected && (
                    <div className="flex shrink-0 items-stretch gap-2.5">
                      <StatTile
                        label={t("sessLabel")}
                        value={sessionStart ? fmtDuration((Date.now() - sessionStart) / 1000) : "-"}
                        sub={"\u2068\u2193 " + fmtBytes(rx) + "\u2069   \u2068\u2191 " + fmtBytes(tx) + "\u2069"}
                      />
                      <StatTile label={t("srvLocation")} value={placeShown} sub={serverAddr || ipText} />
                    </div>
                  )}
                </div>
                {status ? (
                  <div className="mt-2 max-w-[520px] select-text text-[11.5px] leading-snug text-txt3">{status}</div>
                ) : null}

                <AnimatePresence>
                  {connected && (
                    <motion.div
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.36, ease: EASE_OUT, delay: 0.12 }}
                    >
                      <div className="mt-4">
                        <Traffic rx={rx} tx={tx} />
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>'''

if START not in s:
    print("FAIL zone block anchor")
    sys.exit(1)
s = s.replace(START, NEW, 1)

# taller graph, crisp at full width
old_g = '''  const { line, area, rate, peak } = useMemo(() => {
    const values = hist.current.length ? hist.current : [0, 0]
    const max = Math.max(1, ...values)
    const w = 260
    const h = 44'''
new_g = '''  const { line, area, rate, peak } = useMemo(() => {
    const values = hist.current.length ? hist.current : [0, 0]
    const max = Math.max(1, ...values)
    const w = 260
    const h = 56'''
if old_g not in s:
    print("FAIL graph height anchor")
    sys.exit(1)
s = s.replace(old_g, new_g, 1)

old_svg = '''      <svg viewBox="0 0 260 44" preserveAspectRatio="none" className="mt-2 h-[48px] w-full" aria-hidden>
        <path d={area} fill="var(--brand-bg)" />
        <path
          d={line}
          fill="none"
          stroke="var(--brand)"
          strokeWidth="2"
          strokeLinejoin="round"
          strokeLinecap="round"
        />'''
new_svg = '''      <svg viewBox="0 0 260 56" preserveAspectRatio="none" className="mt-2 h-[56px] w-full" aria-hidden>
        <path d={area} fill="var(--brand-bg)" />
        <path
          d={line}
          fill="none"
          stroke="var(--brand)"
          strokeWidth="2"
          vectorEffect="non-scaling-stroke"
          strokeLinejoin="round"
          strokeLinecap="round"
        />'''
if old_svg not in s:
    print("FAIL svg anchor")
    sys.exit(1)
s = s.replace(old_svg, new_svg, 1)

io.open(p, "w", encoding="utf8", newline="").write(s)
print("hero zone restructured, graph taller")

# the info glyph leaves the Connection type label
p2 = ROOT + r"\desktop\ui-next\src\screens\Home.tsx"
t = io.open(p2, "r", encoding="utf8", newline="").read()
old_lbl = '<ControlRow label={t("transport") + " \\u24d8"} align="start" icon={Cable}>'
if old_lbl not in t:
    print("FAIL transport label anchor")
    sys.exit(1)
t = t.replace(old_lbl, '<ControlRow label={t("transport")} align="start" icon={Cable}>', 1)
io.open(p2, "w", encoding="utf8", newline="").write(t)
print("info glyph removed")
