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

# Softer spring: the glide should read as a settle, not a snap.
t = sub(t, 'const SPRING = { type: "spring", stiffness: 320, damping: 34 } as const',
        'const SPRING = { type: "spring", stiffness: 170, damping: 26 } as const')

# The idle text collapses with an animated height instead of unmounting in
# one frame, so the dial glides down as it parks left instead of jumping.
t = sub(t, '''            <AnimatePresence>
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
            </AnimatePresence>''',
'''            <motion.div
              initial={false}
              animate={{
                height: dialLeft ? 0 : "auto",
                marginTop: dialLeft ? 0 : 18,
                opacity: dialLeft ? 0 : 1,
              }}
              transition={{ duration: 0.4, ease: EASE_OUT }}
              className={cn("overflow-hidden", dialLeft && "pointer-events-none")}
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
            </motion.div>''')

# Slower, ordered beats: the zone waits for the glide to land, and on the
# way back it leaves before the dial starts home.
t = sub(t, 'const id = window.setTimeout(() => setShowInfo(true), 250)',
        'const id = window.setTimeout(() => setShowInfo(true), 420)')
t = sub(t, 'const id = window.setTimeout(() => setDialLeft(false), 140)',
        'const id = window.setTimeout(() => setDialLeft(false), 340)')
t = sub(t, 'exit={{ opacity: 0, x: 12 }}\n                transition={{ duration: 0.32, ease: EASE_OUT, delay: 0.06 }}',
        'exit={{ opacity: 0, x: 12 }}\n                transition={{ duration: 0.28, ease: EASE_OUT, delay: 0.05 }}')
save(P, t); print("hero motion ok")

# ============================================================ Home =========
P = SRC + r"\screens\Home.tsx"
t = load(P)
# The setup card reads top down now: title and body span the card, then
# presets left, controls right. No more squeezed two-line heading column.
t = sub(t, '''        <div className="flex flex-col gap-5 lg:flex-row lg:items-stretch">
          {/* the story, and the one choice that shapes it */}
          <div className="flex min-w-0 flex-1 flex-col justify-center">
            <div>
              <h2 className="text-[16.5px] font-semibold text-txt">{t("setupTitle")}</h2>
              <p className="mt-1 max-w-[430px] text-[13.5px] leading-relaxed text-txt2">{t("setupBody")}</p>
            </div>
            <div className="mt-4 flex flex-col gap-2.5">''',
'''        <div>
          <h2 className="text-[16.5px] font-semibold text-txt">{t("setupTitle")}</h2>
          <p className="mt-1 max-w-[430px] text-[13.5px] leading-relaxed text-txt2">{t("setupBody")}</p>
        </div>
        <div className="mt-4 flex flex-col gap-5 lg:flex-row lg:items-stretch">
          {/* the presets */}
          <div className="flex min-w-0 flex-1 flex-col gap-2.5">''')
t = sub(t, '''              })}
            </div>
          </div>

          {/* the controls, one continuous column */}''',
'''              })}
          </div>

          {/* the controls, one continuous column */}''')
save(P, t); print("home setup ok")

# ============================================================ App ==========
P = SRC + r"\App.tsx"
t = load(P)
t = sub(t, 'transition={{ duration: 0.15, ease: EASE_OUT }}',
        'transition={{ duration: 0.22, ease: EASE_OUT }}')
save(P, t); print("app ok")
