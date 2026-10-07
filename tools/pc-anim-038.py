import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# The button animation goes back to the v0.3.8 timing, which the owner knows
# and likes: the snappier spring and the shorter beats. The old jump it used
# to fight is gone now that the word lives inside the dial.
P = SRC + r"\components\Hero.tsx"
t = load(P)
t = sub(t, 'const SPRING = { type: "spring", stiffness: 170, damping: 26 } as const',
        'const SPRING = { type: "spring", stiffness: 320, damping: 34 } as const')
t = sub(t, 'const id = window.setTimeout(() => setShowInfo(true), 420)',
        'const id = window.setTimeout(() => setShowInfo(true), 250)')
t = sub(t, 'const id = window.setTimeout(() => setDialLeft(false), 340)',
        'const id = window.setTimeout(() => setDialLeft(false), 140)')
t = sub(t, 'exit={{ opacity: 0, x: 12 }}\n                transition={{ duration: 0.28, ease: EASE_OUT, delay: 0.05 }}',
        'exit={{ opacity: 0, x: 12 }}\n                transition={{ duration: 0.32, ease: EASE_OUT, delay: 0.06 }}')
save(P, t); print("hero timing ok")

P = SRC + r"\App.tsx"
t = load(P)
t = sub(t, 'transition={{ duration: 0.22, ease: EASE_OUT }}',
        'transition={{ duration: 0.2, ease: EASE_OUT }}')
save(P, t); print("app transition ok")
