import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ============================================================ CSS ==========
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# 1. The desktop's glass tokens (desktop/ui-next/src/index.css).
sub("""  --card: #111826;
  --field: #1a2338;""",
"""  --card: #111826;
  --field: #1a2338;
  /* The one shared material, same tokens as the desktop: flat dark frosted
     fill, lit edge, inner thickness, outer float. */
  --glass-fill: rgb(16 20 28 / 0.55);
  --glass-line: rgb(255 255 255 / 0.12);
  --glass-blur: blur(20px) saturate(150%);""")

# 2. Cards take the material.
sub(""".sec, .hero, .card {
  background: rgb(21 29 46 / 0.62);
  border: 1px solid var(--line);
  border-radius: var(--r-card);
}""",
""".sec, .hero, .card {
  background: var(--glass-fill);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-line);
  border-radius: var(--r-card);
  box-shadow:
    inset 0 -12px 24px rgb(0 0 0 / 0.18),
    0 24px 60px rgb(0 0 0 / 0.35);
}""")

# 3. The tab bar is a floating pane: cooler, quieter, blurred under.
sub("""  background: var(--bg1);
  border: 1px solid var(--line);
  border-radius: var(--r-lg);
  box-shadow: var(--sh-panel);
  z-index: 5;""",
"""  background: rgb(16 20 28 / 0.72);
  backdrop-filter: blur(26px) saturate(150%);
  -webkit-backdrop-filter: blur(26px) saturate(150%);
  border: 1px solid var(--glass-line);
  border-radius: var(--r-lg);
  box-shadow: var(--sh-panel);
  z-index: 5;""")

# 4. The dialog: heavier blur, never heavier opacity than this.
sub("""  background: var(--panel);
  border: 1px solid var(--line-strong);
  border-radius: var(--r-lg);
  box-shadow: inset 0 -12px 24px rgb(0 0 0 / 0.18), 0 30px 70px rgb(0 0 0 / 0.5);""",
"""  background: rgb(18 24 36 / 0.85);
  backdrop-filter: blur(30px) saturate(150%);
  -webkit-backdrop-filter: blur(30px) saturate(150%);
  border: 1px solid var(--glass-line);
  border-radius: var(--r-lg);
  box-shadow: inset 0 -12px 24px rgb(0 0 0 / 0.18), 0 30px 70px rgb(0 0 0 / 0.5);""")

# 5. Picker checks: only the chosen row carries one, and it is white and bold
#    (the desktop's PickerDialog does exactly this).
sub("""/* Picker lists show ghost boxes so rows read as tappable. */
.apps-list .opt .check, .sheet-list .opt .check { visibility: visible; opacity: 0.30; }
.apps-list .opt[aria-selected="true"] .check, .sheet-list .opt[aria-selected="true"] .check { opacity: 1; }""",
""".opt .check { color: #fff; }
.opt .check svg { stroke-width: 2.8; }""")

# 6. The status line under the routing picker is information, not a link.
sub(".appsstatus { margin: 8px 2px 10px; font-size: 12.5px; font-weight: 600; color: var(--accent-strong); }",
    ".appsstatus { margin: 8px 2px 10px; font-size: 12px; font-weight: 500; color: var(--txt3); }")

# 7. Picker rows: the name is one line, smaller, ellipsised - a long
#    hostname must not wrap the row.
sub(".opt .meta { flex: 1; min-width: 0; }",
    ".opt .meta { flex: 1; min-width: 0; }\n.opt .t { display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }\n.sheet-list .opt .meta { font-size: 13px; }")

# 8. Search fields get the desktop's magnifier and its inset.
sub("#sheet-search { margin: 0 8px 8px; }", "#sheet-search { margin: 0; }")
sub("#apps-search { margin-bottom: 10px; }\n", "")
sub("""#sheet-search[hidden] { display: none; }""",
"""#sheet-search[hidden] { display: none; }
.searchwrap { position: relative; display: flex; align-items: center; margin: 0 8px 8px; }
.searchwrap svg { position: absolute; inset-inline-start: 10px; width: 14px; height: 14px; color: var(--txt3); pointer-events: none; }
.searchwrap input { width: 100%; margin: 0; padding-inline-start: 32px; }""")
sub(".apps-list { display: flex; flex-direction: column; gap: 6px;",
    ".searchwrap.apps { margin: 0 0 10px; }\n.apps-list { display: flex; flex-direction: column; gap: 6px;")

# 9. Quiet scrollbars in the lists and dialogs.
sub("""/* Settings: a header, then one card per setting""",
"""::-webkit-scrollbar { width: 4px; height: 4px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: rgb(255 255 255 / 0.12); border-radius: 2px; }

/* Fallback when the system asks for less transparency. */
@media (prefers-reduced-transparency: reduce) {
  .sec, .hero, .card, .tabbar, .sheet {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .sec, .hero, .card { background: rgb(16 20 28 / 0.96); }
  .tabbar { background: rgb(16 20 28 / 0.98); }
  .sheet { background: rgb(18 24 36 / 0.98); }
}

/* Settings: a header, then one card per setting""")

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")

# ============================================================ HTML =========
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub2(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

MAG = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg>'

sub2('''  <input id="sheet-search" data-i18n-ph="sheetSearch" placeholder="Search domains…" autocomplete="off" hidden />''',
'''  <div class="searchwrap"><span class="sw-ico">''' + MAG + '''</span><input id="sheet-search" data-i18n-ph="sheetSearch" placeholder="Search domains…" autocomplete="off" hidden /></div>''')

sub2('''      <input id="apps-search" data-i18n-ph="appsSearch" placeholder="Search apps…" autocomplete="off" />''',
'''      <div class="searchwrap apps"><span class="sw-ico">''' + MAG + '''</span><input id="apps-search" data-i18n-ph="appsSearch" placeholder="Search apps…" autocomplete="off" /></div>''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html ok")

# ============================================================ JS ===========
P = UI + r"\app.js"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub3(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

sub3('''  const m = document.createElement("span");
  m.className = "meta";
  m.textContent = main;''',
'''  const m = document.createElement("span");
  m.className = "meta";
  const tt = document.createElement("span");
  tt.className = "t";
  tt.textContent = main;
  m.append(tt);''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("js ok")
