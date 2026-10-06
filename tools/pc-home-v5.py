import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

P = SRC + r"\components\Hero.tsx"
t = load(P)

# the spring comes back with the parking motion
t = sub(t, 'const EASE_OUT = [0.1, 0.9, 0.2, 1] as const',
        'const EASE_OUT = [0.1, 0.9, 0.2, 1] as const\nconst SPRING = { type: "spring", stiffness: 320, damping: 34 } as const')

# splice from the layout comment to the end of the component
lines = t.split("\n")
start = None
for i, l in enumerate(lines):
    if l.strip() == "// The dial sits left in every state, like the mockup: the word column":
        start = i
        break
assert start is not None, "start not found"
end = None
for j in range(start, len(lines)):
    if lines[j] == '  )' and lines[j + 1] == '}':
        end = j
        break
assert end is not None, "end not found"

new_body = '''  // The dial starts centered and parks left the moment Connect is pressed.
  // The reading zone fades in beside it only after the slide has had room
  // to travel; on the way back the zone exits first, then the dial returns.
  const state: DialState = phase === "on" ? "on" : phase === "connecting" ? "connecting" : "idle"
  const name = card?.name.split(" (")[0] ?? ""
  const hostLine = [name, displayHost(card?.sni)].filter(Boolean).join(" \\u00b7 ")
  const active = phase === "connecting" || phase === "on"

  const [showInfo, setShowInfo] = useState(false)
  useEffect(() => {
    if (!active) {
      setShowInfo(false)
      return
    }
    const id = window.setTimeout(() => setShowInfo(true), 250)
    return () => window.clearTimeout(id)
  }, [active])

  const [dialLeft, setDialLeft] = useState(false)
  useEffect(() => {
    if (active) {
      setDialLeft(true)
      return
    }
    if (phase === "stopping") {
      // Short hold only: the zone exit leads by a beat, then the dial
      // answers at once. A long hold here reads as a dead pause.
      const id = window.setTimeout(() => setDialLeft(false), 140)
      return () => window.clearTimeout(id)
    }
    setDialLeft(false)
  }, [active, phase])

  useTick(1000, connected)

  return (
    <section className="flex flex-1 flex-col rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] p-5">
      <div className="flex flex-1 items-center">
        <motion.div
          layout
          transition={SPRING}
          className={cn("flex w-full items-center", dialLeft ? "justify-start gap-6" : "justify-center")}
        >
          <motion.div layout transition={SPRING} className="flex shrink-0 flex-col items-center text-center">
            <Dial state={state} onClick={toggle} disabled={busy} />
            <AnimatePresence>
              {!active && phase !== "stopping" && (
                <motion.div
                  key="idle"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.25, ease: EASE_OUT }}
                  className="flex flex-col items-center"
                >
                  <button
                    type="button"
                    onClick={toggle}
                    className="mt-5 text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong"
                  >
                    {t("connect")}
                  </button>
                  <p className="mt-1.5 text-[13px] text-txt3">{t("clickToConnect")}</p>
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>

          <AnimatePresence mode="popLayout">
            {showInfo && (
              <motion.div
                key="info"
                initial={{ opacity: 0, x: 16 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 12 }}
                transition={{ duration: 0.32, ease: EASE_OUT, delay: 0.06 }}
                className="min-w-0 flex-1"
              >
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

                <h1 className="mt-2 truncate text-[24px] font-semibold tracking-[-0.01em] text-txt">
                  {connected ? t("disconnect") : t("working")}
                </h1>
                <p className="mt-1 truncate text-[13px] text-txt3" dir="auto">
                  {connected ? hostLine : t("talking")}
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
                          <StatTile label={t("srvLocation")} value={placeText} sub={serverAddr || ipText} />
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>
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

lines = lines[:start] + new_body.split("\n") + lines[end + 1:]
t = "\n".join(lines)
save(P, t)
print("hero motion ok")
