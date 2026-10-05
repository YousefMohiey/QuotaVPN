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

t = sub(t, '''/**
 * The connection card, one composition: the glass dial anchors the left;
 * the right zone carries the state line, the big action word and the host,
 * and once connected the live session opens under them - rate and graph
 * beside the dial, the timers in a slim column to the right. The three
 * connection facts close the card along its floor, so the reading order
 * is state, action, live data, reference data.
 */''',
'''/**
 * The connection card, one composition: the glass dial anchors the left,
 * and the right zone carries the state line, the action word and the host.
 * Under them the zone shows its reading: the three connection facts while
 * idle, and the live session (rate and graph, then the timers) once the
 * tunnel is up. One zone, two readings, nothing else on the card.
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

          <button
            type="button"
            onClick={toggle}
            disabled={busy}
            className="mt-2 block max-w-full truncate text-left text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong disabled:cursor-wait"
          >
            {connected ? t("disconnect") : state === "connecting" ? t("working") : t("connect")}
          </button>
          <p className="mt-1 truncate text-[13px] text-txt3" dir="auto">
            {connected ? hostLine : t("clickToConnect")}
          </p>
          {status ? (
            <div className="mt-2 max-w-[520px] text-[11.5px] leading-snug text-txt3">{status}</div>
          ) : null}

          {/* the zone's reading: facts while idle, the live session once up */}
          <AnimatePresence mode="wait" initial={false}>
            {connected ? (
              <motion.div
                key="session"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.3, ease: EASE_OUT, delay: 0.08 }}
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
                    <StatTile label={t("srvLocation")} value={placeText} sub={ipText} />
                  </div>
                </div>
              </motion.div>
            ) : (
              <motion.div
                key="facts"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.3, ease: EASE_OUT }}
              >
                <div className="mt-5 grid grid-cols-3 gap-4 border-t border-line pt-4">
                  <Fact icon={Globe} label={t("srvLocation")} value={placeText} />
                  <Fact icon={Network} label={t("yourIp")} value={ipText} />
                  <Fact
                    icon={Activity}
                    label={t("statusLbl")}
                    value={connected ? t("connected") : t("notConnected")}
                    dot={connected}
                  />
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </section>
  )'''

lines = lines[:start] + new_ret.split("\n") + lines[end + 1:]
t = "\n".join(lines)
save(P, t)
print("hero v3 ok")

# ============================================================ Dial =========
P = SRC + r"\components\Dial.tsx"
t = load(P)
# Stronger glass: lighter translucent fill, a radial sheen, a lit top edge
# and a real float, since there is no scene behind the card to refract.
t = sub(t, '''        /* the glass: a light translucent pane, frosted by the scene behind
           it, with a lit top edge and a soft floor shadow */
        "bg-[rgb(255_255_255/0.055)] backdrop-blur-[16px] backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.16),inset_0_-18px_36px_rgb(0_0_0/0.22),0_18px_44px_rgb(0_0_0/0.35)]",''',
'''        /* the glass: a light translucent pane with a soft sheen at the
           top, a lit edge, inner thickness and a real float underneath */
        "bg-[rgb(255_255_255/0.07)] backdrop-blur-[16px] backdrop-saturate-150",
        "bg-[radial-gradient(120%_120%_at_50%_0%,rgb(255_255_255/0.13),transparent_55%)]",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.25),inset_0_-22px_44px_rgb(0_0_0/0.28),0_22px_50px_rgb(0_0_0/0.45)]",''')
save(P, t)
print("dial ok")
