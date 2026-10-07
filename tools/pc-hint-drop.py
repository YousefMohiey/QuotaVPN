import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# The idle hint goes: pressing Connect is the obvious move, the word says it.
P = SRC + r"\components\Hero.tsx"
t = load(P)
t = sub(t, '''              <div ref={textRef} className="mt-[18px] flex flex-col items-center pt-2">
                <button
                  type="button"
                  onClick={toggle}
                  className="text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong"
                >
                  {t("connect")}
                </button>
                <p className="mt-1.5 text-[13px] text-txt3">{t("clickToConnect")}</p>
              </div>''',
'''              <div ref={textRef} className="mt-[18px] flex flex-col items-center">
                <button
                  type="button"
                  onClick={toggle}
                  className="text-[24px] font-semibold tracking-[-0.01em] text-txt transition-colors duration-200 hover:text-brand-strong"
                >
                  {t("connect")}
                </button>
              </div>''')
save(P, t); print("hint removed")

# The string itself is now unused.
for f, line in [("en.ts", '  "clickToConnect": "Click to connect to QuotaVPN",\n'),
                ("ar.ts", '  "clickToConnect": "اضغط للاتصال بـQuotaVPN",\n')]:
    P = SRC + r"\lib\i18n\\" + f
    t = load(P)
    assert t.count(line) == 1, "count %d in %s" % (t.count(line), f)
    t = t.replace(line, "", 1)
    save(P, t); print(f, "string removed")
