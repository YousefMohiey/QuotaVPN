import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# ============================================================ Hero =========
P = SRC + r"\components\Hero.tsx"
t = load(P)

# doc comment
t = sub(t, '''/**
 * The connection card: the glass dial on the left, the state and the big
 * action word beside it. The live session (rate, graph, timers) opens a
 * full-width block under the row once connected, and the three connection
 * facts sit along the card's floor. Nothing here is sized to the window,
 * so no column ever gets crushed.
 */''',
'''/**
 * The connection card, one composition: the glass dial anchors the left;
 * the right zone carries the state line, the big action word and the host,
 * and once connected the live session opens under them - rate and graph
 * beside the dial, the timers in a slim column to the right. The three
 * connection facts close the card along its floor, so the reading order
 * is state, action, live data, reference data.
 */''')

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
    <section className="flex flex-1 flex-col rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
      <div className="flex flex-1 items-center gap-6">
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

          <h1 className="mt-2 truncate text-[24px] font-semibold tracking-[-0.01em] text-txt">
            {connected ? t("disconnect") : state === "connecting" ? t("working") : t("connect")}
          </h1>
          <p className="mt-1 truncate text-[13px] text-txt3" dir="auto">
            {connected ? hostLine : t("clickToConnect")}
          </p>
          {status ? (
            <div className="mt-2 max-w-[520px] text-[11.5px] leading-snug text-txt3">{status}</div>
          ) : null}

          <AnimatePresence>
            {connected && (
              <motion.div
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.36, ease: EASE_OUT, delay: 0.12 }}
              >
                {/* the live session rides beside the dial: rate and graph on
                    the left of the zone, the timers in a slim column right */}
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
                    <StatTile label={t("yourIp")} value={serverAddr || displayHost(serverIp)} sub={exit?.isp || undefined} />
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>

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
save(P, t)
print("hero ok")
