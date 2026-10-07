import io

P = r"C:\Tools\QuotaCards\desktop\ui-next\src\components\Hero.tsx"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# Measure the text block so the dial can drop by half of it when parked.
t = sub(t, '''    const id = window.setTimeout(() => setTextOpen(true), 560)
    return () => window.clearTimeout(id)
  }, [dialLeft])''',
'''    const id = window.setTimeout(() => setTextOpen(true), 560)
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
  const dialDrop = (18 + textH) / 2''')

# The dial moves down as it parks so it sits centered against the zone.
t = sub(t, '''            <Dial state={state} onClick={toggle} disabled={busy} />''',
'''            <motion.div
              animate={{ y: dialLeft ? dialDrop : 0 }}
              transition={SPRING}
              className="flex flex-col items-center"
            >
              <Dial state={state} onClick={toggle} disabled={busy} />
            </motion.div>''')

# The text only fades; its height never animates, so nothing reflows.
t = sub(t, '''            <motion.div
              initial={false}
              animate={{
                height: textOpen ? "auto" : 0,
                marginTop: textOpen ? 18 : 0,
                opacity: textOpen ? 1 : 0,
              }}
              transition={{ duration: 0.35, ease: EASE_OUT }}
              className={cn("overflow-hidden", !textOpen && "pointer-events-none")}
            >
              <div className="flex flex-col items-center">
                <button
                  type="button"
                  onClick={toggle}
                  className="mt-2 text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong"
                >
                  {t("connect")}
                </button>
                <p className="mt-1.5 text-[13px] text-txt3">{t("clickToConnect")}</p>
              </div>
            </motion.div>''',
'''            <motion.div
              initial={false}
              animate={{ opacity: textOpen ? 1 : 0 }}
              transition={{ duration: 0.3, ease: EASE_OUT }}
              className={cn(!textOpen && "pointer-events-none")}
            >
              <div ref={textRef} className="mt-[18px] flex flex-col items-center pt-2">
                <button
                  type="button"
                  onClick={toggle}
                  className="text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong"
                >
                  {t("connect")}
                </button>
                <p className="mt-1.5 text-[13px] text-txt3">{t("clickToConnect")}</p>
              </div>
            </motion.div>''')

nl = "\r\n" if b"\r\n" in io.open(P, "rb").read()[:2000] else "\n"
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
print("hero v8 ok")
