import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ---------------------------------------------------------------- html -----
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# The session folds into the one status line: the dot says the state, the
# line carries the live numbers. Saves a whole row on the busiest screen.
sub('''      <div class="hero-line" id="hero-line"><span class="hl-dot" id="hl-dot"></span><span id="hl-txt">Not connected</span></div>
      <div class="session" id="session-line" hidden>
        <span id="sess-time">00:00</span>
        <span class="sep"></span>
        <span>↓ <span id="sess-rx">0 B</span></span>
        <span class="sep"></span>
        <span>↑ <span id="sess-tx">0 B</span></span>
      </div>''',
'''      <div class="hero-line" id="hero-line">
        <span class="hl-dot" id="hl-dot"></span>
        <span id="hl-txt">Not connected</span>
        <span class="session" id="session-line" hidden>
          <span id="sess-time">00:00</span>
          <span class="sep"></span>
          <span>↓ <span id="sess-rx">0 B</span></span>
          <span class="sep"></span>
          <span>↑ <span id="sess-tx">0 B</span></span>
        </span>
      </div>''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html ok")

# ---------------------------------------------------------------- js -------
P = UI + r"\app.js"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub3(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# Connected: the dot plus the live numbers say it; the word would only
# repeat the facts strip. Idle and working keep their word.
sub3('''function paintFactStatus() {
  const txt = busy ? t("working") : vpnOn ? t("vpnConnected") : t("notConnected");
  const el = $("fact-status");
  if (el) el.textContent = txt;
  const line = $("hl-txt");
  if (line) line.textContent = txt;''',
'''function paintFactStatus() {
  const txt = busy ? t("working") : vpnOn ? t("vpnConnected") : t("notConnected");
  const el = $("fact-status");
  if (el) el.textContent = txt;
  const line = $("hl-txt");
  if (line) line.textContent = vpnOn && !busy ? "" : txt;''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("js ok")

# ---------------------------------------------------------------- css ------
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub4(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# Fit the screen: the header costs a row, so every block tightens a notch.
sub4("  padding: 20px 14px 14px;", "  padding: 14px 14px 12px;")
sub4("""  width: 152px;
  height: 152px;""", """  width: 128px;
  height: 128px;""")
sub4(".hero-btn .power { width: 42px; height: 42px; position: relative; z-index: 1; }",
    ".hero-btn .power { width: 36px; height: 36px; position: relative; z-index: 1; }")
sub4(".btn-action { font-size: 19px; line-height: 1.2; font-weight: 700; letter-spacing: -0.01em; color: var(--txt); margin: 8px 0 2px; }",
    ".btn-action { font-size: 18px; line-height: 1.2; font-weight: 700; letter-spacing: -0.01em; color: var(--txt); margin: 7px 0 0; }")
sub4(".hero-line { display: inline-flex; align-items: center; gap: 7px; margin: 5px 0 0; font-size: 12.5px; color: var(--txt2); }",
    ".hero-line { display: inline-flex; align-items: center; justify-content: center; flex-wrap: wrap; gap: 6px; margin: 4px 0 0; font-size: 12.5px; color: var(--txt2); }")
sub4(".facts-panel { display: grid; grid-template-columns: repeat(3, 1fr); padding: 12px 12px; }",
    ".facts-panel { display: grid; grid-template-columns: repeat(3, 1fr); padding: 9px 11px; }")
sub4(".card { padding: 20px 16px 16px; }", ".card { padding: 14px 16px 12px; }")
sub4("#setup-card > * + * { margin-top: 16px; }", "#setup-card > * + * { margin-top: 12px; }")
sub4(".preset{display:flex; flex-direction:row; align-items:center; gap:9px; min-height:76px;",
    ".preset{display:flex; flex-direction:row; align-items:center; gap:9px; min-height:64px;")
sub4(".homenet-row { display: flex; align-items: center; gap: 10px; padding: 13px 2px;",
    ".homenet-row { display: flex; align-items: center; gap: 10px; padding: 9px 2px;")

# The session rides inside the status line now.
sub4(".session {", ".hero-line .session { display: inline-flex; align-items: center; gap: 6px; }\n.session {")

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
