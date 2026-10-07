import io

P = r"C:\Tools\QuotaCards\desktop\ui-next\src\components\Hero.tsx"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# Graph back to the way it was: the window spreads across the full width.
t = sub(t, '''    // A fixed step keeps the trace anchored to the left as the window fills
    // instead of stretching a two point wedge across the whole width.
    const step = w / 59''',
'''    const step = w / Math.max(1, values.length - 1)''')
t = sub(t, '''    const current = values[values.length - 1] || 0
    const endX = (values.length - 1) * step
    return {
      line: d,
      area: `${d} L ${endX.toFixed(1)} ${h} L 0 ${h} Z`,''',
'''    const current = values[values.length - 1] || 0
    return {
      line: d,
      area: `${d} L ${w} ${h} L 0 ${h} Z`,''')

# The text opens only after the dial has landed, so the glide never fights
# a height change and the return reads as one clean move.
t = sub(t, '''    setDialLeft(false)
  }, [active, phase])''',
'''    setDialLeft(false)
  }, [active, phase])

  const [textOpen, setTextOpen] = useState(true)
  useEffect(() => {
    if (dialLeft) {
      setTextOpen(false)
      return
    }
    // the text waits for the dial to land before it opens
    const id = window.setTimeout(() => setTextOpen(true), 560)
    return () => window.clearTimeout(id)
  }, [dialLeft])''')

t = sub(t, '''              animate={{
                height: dialLeft ? 0 : "auto",
                marginTop: dialLeft ? 0 : 18,
                opacity: dialLeft ? 0 : 1,
              }}
              transition={{ duration: 0.4, ease: EASE_OUT }}
              className={cn("overflow-hidden", dialLeft && "pointer-events-none")}''',
'''              animate={{
                height: textOpen ? "auto" : 0,
                marginTop: textOpen ? 18 : 0,
                opacity: textOpen ? 1 : 0,
              }}
              transition={{ duration: 0.35, ease: EASE_OUT }}
              className={cn("overflow-hidden", !textOpen && "pointer-events-none")}''')

nl = "\r\n" if b"\r\n" in io.open(P, "rb").read()[:2000] else "\n"
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
print("hero v7 ok")
