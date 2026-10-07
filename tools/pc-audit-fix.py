import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# 1. The JS springs now honour the OS reduced-motion preference.
P = SRC + r"\main.tsx"
t = load(P)
t = sub(t, 'import { TooltipProvider } from "@/components/ui/tooltip"',
        'import { MotionConfig } from "motion/react"\nimport { TooltipProvider } from "@/components/ui/tooltip"')
t = sub(t, '''  <StrictMode>
    <I18nProvider>
      <AppStateProvider>
        <TooltipProvider>
          <App />
        </TooltipProvider>
      </AppStateProvider>
    </I18nProvider>
  </StrictMode>,''',
'''  <StrictMode>
    {/* Windows' animation-effects preference stands the springs down: motion
        collapses transforms to instant and keeps the opacity fades. */}
    <MotionConfig reducedMotion="user">
      <I18nProvider>
        <AppStateProvider>
          <TooltipProvider>
            <App />
          </TooltipProvider>
        </AppStateProvider>
      </I18nProvider>
    </MotionConfig>
  </StrictMode>,''')
save(P, t); print("motion config ok")

# 2. Corner radius strays folded into the scale: 9 -> 8, 14 -> 12.
P = SRC + r"\components\Hero.tsx"
t = load(P)
t = sub(t, 'rounded-[9px] border border-line bg-white/[0.03] text-brand-strong',
        'rounded-[8px] border border-line bg-white/[0.03] text-brand-strong')
save(P, t); print("hero radius ok")

P = SRC + r"\screens\Home.tsx"
t = load(P)
t = sub(t, 'flex min-h-[56px] items-center gap-3 rounded-[14px] border',
        'flex min-h-[56px] items-center gap-3 rounded-[12px] border')
save(P, t); print("home radius ok")

P = SRC + r"\screens\SpeedHistory.tsx"
t = load(P)
t = sub(t, '"group flex items-center rounded-[14px] border border-line bg-[rgb(21_29_46/0.62)] transition-colors hover:border-line-strong"',
        '"group flex items-center rounded-[12px] border border-line bg-[rgb(21_29_46/0.62)] transition-colors hover:border-line-strong"')
t = sub(t, 'className="sticky bottom-0 flex items-center justify-between gap-3 rounded-[14px] border border-line bg-[rgb(21_29_46/0.9)] px-4 py-2.5 backdrop-blur-xl"',
        'className="sticky bottom-0 flex items-center justify-between gap-3 rounded-[12px] border border-line bg-[rgb(21_29_46/0.9)] px-4 py-2.5 backdrop-blur-xl"')
save(P, t); print("history radius ok")

# 3. Document the radius scale where the tokens live.
P = SRC + r"\index.css"
t = load(P)
t = sub(t, '''  :focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }''',
'''  :focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  /* Radius scale, kept to five steps: cards 16, rows and tiles 12, controls
     10, chips and icon boxes 8, dialogs 20. Micro parts (checkbox 5, tooltip
     arrow 2) keep their own. */
''')
save(P, t); print("scale documented")
