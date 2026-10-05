import io, re
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ============================================================ HTML =========
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# 1. The status pill duplicated the facts strip's Status column. Gone.
sub('''      <div class="hero-state"><span class="livedot"></span><span id="hero-state">Not Connected</span></div>\n''', "")

# 2. Fact chips: three little boxes in a small strip read as noise. The
#    label and value carry it.
t = re.sub(r'[ \t]*<span class="fact-ico"[^\n]*</span>\n', "", t)
assert "fact-ico" not in t, "fact-ico left"

# 3. Presets become two compact tiles; the radio dot goes with them.
t = re.sub(r'[ \t]*<span class="preset-radio"[^\n]*</span>\n', "", t)
assert "preset-radio" not in t, "preset-radio left"

# 4. The transport block: one segmented control, no label over it and no
#    note under it (the warning still arrives as a toast on switch).
i = t.index('      <div class="tblock">')
j = t.index("</div>", t.index('id="transport-note"')) + len("</div>")
seg = re.search(r'[ \t]*<div class="seg" id="transport-seg">[^\n]*</div>\n', t[i:j]).group(0)
t = t[:i] + "      " + seg.strip() + "\n" + t[j + 1:]
assert "transport-label" not in t and "transport-note" not in t, "transport left"

# 5. The apps hint duplicated what the status line already says.
sub('''      <p class="hint" data-i18n="appsHint">Changes apply next time you connect.</p>\n''', "")

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html ok")

# ============================================================ JS ===========
P = UI + r"\app.js"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub3(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# The pill is gone; these writes must not crash on a missing node.
sub3('''function paintFactStatus() {''',
'''function setTxt(id, v) {
  const el = $(id);
  if (el) el.textContent = v;
}
function paintFactStatus() {''')

sub3('    $("hero-state").textContent = t("working");', '    setTxt("hero-state", t("working"));')
sub3('    $("hero-state").textContent = t("vpnConnected");', '    setTxt("hero-state", t("vpnConnected"));')
sub3('    $("hero-state").textContent = t("ready");', '    setTxt("hero-state", t("ready"));')
sub3('    $("hero-state").textContent = t("notConnected");', '    setTxt("hero-state", t("notConnected"));')

# The "All traffic goes through X" line repeated what the tiles and the
# facts strip already say.
sub3('''    $("hero-sub").innerHTML = "";
    $("hero-sub").append(
      document.createTextNode(t("trafficThru")),
      (() => { const b = document.createElement("bdi"); b.textContent = vpnCardName || "your card"; return b; })(),
    );
    if (lang === "en") $("hero-sub").append(document.createTextNode("."));
''', '''    $("hero-sub").textContent = "";
''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("js ok")

# ============================================================ CSS ==========
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub4(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# The pill and its colours leave with the element.
sub4('''/* Live status pill: quiet idle, blue working, green on. */
.hero-state { display: inline-flex; align-items: center; gap: 8px; font-size: 12px; font-weight: 500; color: var(--txt2); background: rgb(255 255 255 / 0.03); border: 1px solid var(--line); border-radius: 999px; padding: 6px 13px; margin: 2px 0 8px; }
.hero-state .livedot { display: block; width: 7px; height: 7px; border-radius: 50%; background: var(--txt3); flex: none; }
.hero.connected .hero-state { color: var(--green); border-color: var(--green-line); background: var(--green-bg); }
.hero.connected .hero-state .livedot { background: var(--green); }
.hero.connecting .hero-state { color: var(--accent-strong); border-color: var(--accent-line); background: var(--accent-bg); }
.hero.connecting .hero-state .livedot { background: var(--accent-strong); animation: pulse 1.1s ease-in-out infinite; }
''', "")

# The chip boxes leave with the spans.
sub4('''.fact-ico { width: 22px; height: 22px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 7px; background: rgb(255 255 255 / 0.03); color: var(--accent-strong); }
.fact-ico svg { width: 12px; height: 12px; }
''', "")

# Presets: two compact tiles side by side instead of two tall rows.
sub4('''.presets{display:flex; flex-direction:column; gap:8px; margin:10px 0 2px; width:100%}
.preset{display:flex; flex-direction:row; align-items:center; gap:10px; min-height:52px;
  padding:8px 10px; border-radius:14px; border:1px solid var(--line); background:rgb(255 255 255 / 0.02); color:var(--txt2); text-align:start}
.preset.on{border-color:var(--accent-line); background:var(--accent-bg); color:var(--txt)}
.preset-ico{width:34px; height:34px; flex:none; display:grid; place-items:center; border:1px solid var(--line); border-radius:10px; background:rgb(255 255 255 / 0.03); color:var(--txt3)}
.preset-ico svg{width:16px; height:16px}
.preset.on .preset-ico{color:var(--accent-strong); border-color:var(--accent-line); background:var(--accent-bg)}
.preset-txt{flex:1; min-width:0}
.preset-name{display:flex; align-items:center; font-size:13.5px; font-weight:600; min-width:0; max-width:100%; line-height:1.35}
.preset-sni{display:block; width:100%; font-size:11px; line-height:1.35; color:var(--txt3); font-variant-numeric:tabular-nums; overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
.preset-radio{width:18px; height:18px; flex:none; display:grid; place-items:center; border:1.5px solid var(--line-strong); border-radius:50%}
.preset.on .preset-radio{border-color:var(--accent)}
.preset-dot{width:8px; height:8px; border-radius:50%; background:var(--accent); opacity:0}
.preset.on .preset-dot{opacity:1}
.preset:disabled{opacity:.6}
.preset:active{transform:scale(0.985)}''',
'''.presets{display:grid; grid-template-columns:1fr 1fr; gap:8px; width:100%}
.preset{display:flex; flex-direction:row; align-items:center; gap:9px; min-height:60px;
  padding:9px 11px; border-radius:13px; border:1px solid var(--line); background:rgb(255 255 255 / 0.02); color:var(--txt2); text-align:start}
.preset.on{border-color:var(--accent-line); background:var(--accent-bg); color:var(--txt)}
.preset-ico{width:30px; height:30px; flex:none; display:grid; place-items:center; border:1px solid var(--line); border-radius:9px; background:rgb(255 255 255 / 0.03); color:var(--txt3)}
.preset-ico svg{width:15px; height:15px}
.preset.on .preset-ico{color:var(--accent-strong); border-color:var(--accent-line); background:var(--accent-bg)}
.preset-txt{flex:1; min-width:0}
.preset-name{display:flex; align-items:center; font-size:13px; font-weight:600; min-width:0; max-width:100%; line-height:1.3}
.preset-sni{display:block; width:100%; font-size:10.5px; line-height:1.3; color:var(--txt3); font-variant-numeric:tabular-nums; overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
.preset:disabled{opacity:.6}
.preset:active{transform:scale(0.985)}''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
