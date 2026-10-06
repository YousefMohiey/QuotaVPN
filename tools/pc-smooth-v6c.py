import io

P = r"C:\Tools\QuotaCards\desktop\ui-next\src\components\Hero.tsx"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

old = '''    const w = 260
    const h = 44
    const step = w / Math.max(1, values.length - 1)
    const pts = values.map((v, i) => [i * step, h - (v / max) * (h - 8) - 3] as const)
    const d = pts
      .map(([x, y], i) => `${i ? "L" : "M"} ${x.toFixed(1)} ${y.toFixed(1)}`)
      .join(" ")
    const current = values[values.length - 1] || 0
    return {
      line: d,
      area: `${d} L ${w} ${h} L 0 ${h} Z`,'''
new = '''    const w = 260
    const h = 44
    // A fixed step keeps the trace anchored to the left as the window fills
    // instead of stretching a two point wedge across the whole width.
    const step = w / 59
    const pts = values.map((v, i) => [i * step, h - (v / max) * (h - 8) - 3] as const)
    const d = pts
      .map(([x, y], i) => `${i ? "L" : "M"} ${x.toFixed(1)} ${y.toFixed(1)}`)
      .join(" ")
    const current = values[values.length - 1] || 0
    const endX = (values.length - 1) * step
    return {
      line: d,
      area: `${d} L ${endX.toFixed(1)} ${h} L 0 ${h} Z`,'''
assert t.count(old) == 1, "count %d" % t.count(old)
t = t.replace(old, new, 1)

nl = "\r\n" if b"\r\n" in io.open(P, "rb").read()[:2000] else "\n"
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
print("graph step ok")
