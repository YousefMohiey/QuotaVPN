import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# ---- 1. Settings back to the top, as the owner wants it -------------------
P = SRC + r"\screens\Settings.tsx"
t = load(P)
t = sub(t, '    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">',
        '    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-3">')
t = sub(t, '''      {/* the cards sit centered in the page instead of stranded at the top */}
      <div className="flex min-h-0 flex-1 flex-col justify-center gap-3">''',
        '''      {/* the cards sit right under the header, as the owner wants */}
      <div className="flex flex-col gap-3">''')
save(P, t); print("settings top ok")

# ---- 2. Connection type gets a real icon ----------------------------------
P = SRC + r"\screens\Home.tsx"
t = load(P)
t = sub(t, 'import { ChevronDown, ChevronRight, Gamepad2, LayoutGrid, Server as ServerIcon, TriangleAlert, Tv, type LucideIcon } from "lucide-react"',
        'import { Cable, ChevronDown, ChevronRight, Gamepad2, LayoutGrid, Server as ServerIcon, Tv, type LucideIcon } from "lucide-react"')
t = sub(t, '<ControlRow label={t("transport") + " \\u24d8"} align="start">',
        '<ControlRow label={t("transport") + " \\u24d8"} align="start" icon={Cable}>')
# the inline warning box goes: it is a notification now
t = sub(t, '''                {transport !== "vless" && (
                  <p
                    role="status"
                    className="mt-2 flex items-start gap-2 rounded-[10px] border border-warn-line bg-warn-bg px-2.5 py-1.5 text-[11.5px] leading-[1.45] text-warn"
                  >
                    <TriangleAlert className="mt-px size-3.5 shrink-0" aria-hidden />
                    <span>{t("quotaWarn")}</span>
                  </p>
                )}
''', '')
save(P, t); print("home icon + warning ok")

# ---- 3. the toast itself ---------------------------------------------------
P = SRC + r"\components\QuotaToast.tsx"
io.open(P, "w", encoding="utf-8", newline="").write('''import { useEffect, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { TriangleAlert, X } from "lucide-react"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const

/** The transport warning as a notification: it used to sit inline in the
    setup card and push the column around. It floats top right now and the
    user can close it; a new transport choice arms it again. */
export function QuotaToast() {
  const { transport } = useApp()
  const { t } = useI18n()
  const [closed, setClosed] = useState(false)
  useEffect(() => {
    setClosed(false)
  }, [transport])
  return (
    <AnimatePresence>
      {transport !== "vless" && !closed && (
        <motion.div
          role="status"
          initial={{ opacity: 0, x: 24 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 24 }}
          transition={{ duration: 0.28, ease: EASE_OUT }}
          className="fixed right-4 top-14 z-[60] flex w-[380px] max-w-[calc(100vw-2rem)] items-start gap-2.5 rounded-[12px] border border-warn-line bg-warn-bg px-3.5 py-3 text-[12.5px] leading-[1.5] text-warn shadow-[0_18px_50px_rgb(0_0_0/0.5)] backdrop-blur-[24px]"
        >
          <TriangleAlert className="mt-px size-4 shrink-0" aria-hidden />
          <span className="min-w-0 flex-1">{t("quotaWarn")}</span>
          <button
            type="button"
            onClick={() => setClosed(true)}
            aria-label={t("dismiss")}
            className="-me-1 -mt-0.5 grid size-6 shrink-0 place-items-center rounded-[8px] transition-colors hover:bg-white/[0.06]"
          >
            <X className="size-3.5" aria-hidden />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
''')
print("toast ok")

# ---- 4. mount it -----------------------------------------------------------
P = SRC + r"\App.tsx"
t = load(P)
t = sub(t, 'import { WindowControls } from "@/components/WindowControls"',
        'import { WindowControls } from "@/components/WindowControls"\nimport { QuotaToast } from "@/components/QuotaToast"')
t = sub(t, '''        </main>
      </div>''',
'''        </main>
        <QuotaToast />
      </div>''')
save(P, t); print("app mount ok")

# ---- 5. app routing selections take the history colours --------------------
P = SRC + r"\screens\Apps.tsx"
t = load(P)
t = sub(t, '''                    appsMode === "all" && "opacity-70",
                    focused ? "bg-white/[0.04]" : "hover:bg-white/[0.02]",''',
'''                    appsMode === "all" && "opacity-70",
                    on
                      ? "bg-[var(--brand-bg)]"
                      : focused
                        ? "bg-white/[0.04]"
                        : "hover:bg-white/[0.02]",''')
t = sub(t, '''                      "grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
                      on ? "border-[var(--brand)] bg-[var(--brand)]" : "border-line-strong",''',
'''                      "grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
                      on ? "border-[var(--brand-line)] bg-[var(--brand-bg)]" : "border-line-strong",''')
t = sub(t, '{on && <Check className="size-3 text-white" strokeWidth={3} aria-hidden />}',
        '{on && <Check className="size-3 text-brand-strong" strokeWidth={3} aria-hidden />}')
save(P, t); print("apps colours ok")

# ---- 6. the dismiss string -------------------------------------------------
for f, anchor, line in [("en.ts", '  "quotaWarn": "Spends from your general quota, not your package.",\n',
                         '  "quotaWarn": "Spends from your general quota, not your package.",\n  "dismiss": "Dismiss",\n'),
                        ("ar.ts", '  "quotaWarn": "يُحتسب من الباقة العامة، وليس من باقتك.",\n',
                         '  "quotaWarn": "يُحتسب من الباقة العامة، وليس من باقتك.",\n  "dismiss": "إغلاق",\n')]:
    P = SRC + r"\lib\i18n\\" + f
    t = load(P)
    t = sub(t, anchor, line)
    save(P, t); print(f, "dismiss ok")
