import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ---------------------------------------------------------------- html -----
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")
old = '      <div class="seg" id="transport-seg">'
new = '''      <label data-i18n="transport" id="transport-label">Connection</label>
      <div class="seg" id="transport-seg">'''
assert t.count(old) == 1
t = t.replace(old, new, 1)
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html ok")

# ---------------------------------------------------------------- css ------
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:80])
    t = t.replace(old, new, cnt)

# A quiet label keeps the protocol control from floating on its own.
sub("""#transport-seg { flex: none; align-self: stretch; min-width: 0; padding: 3px; border-radius: var(--r-ctl); }""",
"""#transport-label { align-self: stretch; text-align: start; font-size: 11.5px; font-weight: 500; letter-spacing: 0.01em; color: var(--txt3); }
#setup-card > #transport-seg { margin-top: 7px; }
#transport-seg { flex: none; align-self: stretch; min-width: 0; padding: 3px; border-radius: var(--r-ctl); }""")

# Fill the screen: bigger dial, roomier rows, a card that earns its height.
sub("  padding: 18px 14px 12px;", "  padding: 20px 14px 16px;")
sub("""  width: 168px;
  height: 168px;""", """  width: 180px;
  height: 180px;""")
sub(".hero-btn .power { width: 46px; height: 46px; position: relative; z-index: 1; }",
    ".hero-btn .power { width: 50px; height: 50px; position: relative; z-index: 1; }")
sub(".facts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin: 18px 0 0; border-top: 1px solid var(--line); padding-top: 13px; }",
    ".facts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin: 20px 0 0; border-top: 1px solid var(--line); padding-top: 15px; }")
sub(".card { padding: 18px 16px 16px; }", ".card { padding: 20px 16px 18px; }")
sub("#setup-card > * + * { margin-top: 14px; }", "#setup-card > * + * { margin-top: 16px; }")
sub(".preset{display:flex; flex-direction:row; align-items:center; gap:9px; min-height:70px;",
    ".preset{display:flex; flex-direction:row; align-items:center; gap:9px; min-height:76px;")
sub(".homenet-row { display: flex; align-items: center; gap: 8px; padding: 9px 2px;",
    ".homenet-row { display: flex; align-items: center; gap: 8px; padding: 13px 2px;")

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
