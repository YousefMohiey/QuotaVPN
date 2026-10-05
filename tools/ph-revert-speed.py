import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ============================================================ HTML =========
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

import re
# The fact icon boxes leave again.
n = len(re.findall(r'[ \t]*<span class="fact-ico"[^\n]*</span>\n', t))
assert n == 3, "fact-ico count %d" % n
t = re.sub(r'[ \t]*<span class="fact-ico"[^\n]*</span>\n', "", t)

# Metric cells and net cells lose their icon boxes.
t = re.sub(r'<span class="sico"[^>]*>[^\n]*?</span>', "", t)
assert "sico" not in t, "sico left"

# The run control goes back to the bar with its own stop button.
sub('''        <button type="button" id="sp-run" aria-label="Start test"><svg class="ico-play" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg><svg class="ico-stop" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="7" y="7" width="10" height="10" rx="2"/></svg><span id="sp-run-label" data-i18n="startTest">Start test</span></button>''',
'''        <button type="button" id="sp-run"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg><span id="sp-run-label" data-i18n="startTest">Start test</span></button>
        <button type="button" id="sp-stop" hidden data-i18n="stop">Stop</button>''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html ok")

# ============================================================ JS ===========
P = UI + r"\app.js"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub3(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

sub3('''function spPaintRunState() {
  // One disc does both: it starts the run, and while the run is live it
  // stops it. The icon and the word swap together.
  const btn = $("sp-run");
  if (btn) {
    btn.disabled = false;
    btn.classList.toggle("stopping", !!spCtl);
  }
  const lbl = $("sp-run-label");
  if (lbl) lbl.textContent = spCtl ? t("stop") : t("startTest");
  document.querySelectorAll(".sstat").forEach((b) => { b.disabled = !!spCtl; });
}''',
'''function spPaintRunState() {
  const btn = $("sp-run");
  if (btn) btn.disabled = !!spCtl;
  const lbl = $("sp-run-label");
  if (lbl) lbl.textContent = spCtl ? t("measuring") : t("startTest");
  const stop = $("sp-stop");
  if (stop) stop.hidden = !spCtl;
  document.querySelectorAll(".sstat").forEach((b) => { b.disabled = !!spCtl; });
}''')

sub3('$("sp-run").onclick = () => { if (spCtl) spStop(); else void spRun("all"); };',
     '$("sp-run").onclick = () => { void spRun("all"); };\n$("sp-stop").onclick = spStop;')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("js ok")

# ============================================================ CSS ==========
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub4(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# Fact rows: plain label over value again.
sub4('''.fact { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; min-width: 0; padding: 0; }
.fact-ico { width: 30px; height: 30px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 10px; background: rgb(255 255 255 / 0.03); color: var(--accent-strong); }
.fact-ico svg { width: 15px; height: 15px; }''',
'''.fact { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; min-width: 0; padding: 0; }''')
sub4(".fact-txt i { font-style: normal; font-size: 10.5px; font-weight: 500; letter-spacing: 0.08em; text-transform: uppercase; line-height: 1.25; color: var(--txt3); }",
     ".fact-txt i { font-style: normal; font-size: 11px; line-height: 1.25; color: var(--txt3); }")

# Metric row: one hairline on top, four plain cells.
sub4(".speed-stats{display:grid; grid-template-columns:repeat(4,1fr); margin-top:14px; border:1px solid var(--line); border-radius:14px; background:rgb(255 255 255 / 0.02); overflow:hidden}",
     ".speed-stats{display:grid; grid-template-columns:repeat(4,1fr); margin-top:14px; border-top:1px solid var(--line)}")
sub4('''.sstat{appearance:none; background:none; border:0; padding:11px 5px 11px 9px; display:flex; flex-direction:column; align-items:flex-start; gap:3px; color:inherit; text-align:start; min-width:0}
.sico{width:26px; height:26px; flex:none; display:grid; place-items:center; border:1px solid var(--line); border-radius:9px; background:rgb(255 255 255 / 0.03); color:var(--accent-strong)}
.sico svg{width:14px; height:14px}
.sstat .sico{margin-bottom:5px}''',
'''.sstat{appearance:none; background:none; border:0; padding:8px 2px; display:flex; flex-direction:column; align-items:flex-start; gap:3px; color:inherit; text-align:start; min-width:0}''')
sub4(".sstat + .sstat{border-left:1px solid var(--line)}", ".sstat + .sstat{border-left:1px solid var(--line); padding-left:10px}")

# Net card: back to the plain two columns.
sub4(".speed-netcol{min-width:0; padding:11px 12px 11px 50px; text-align:start; font:inherit; color:inherit; background:none; border-radius:0; position:relative}\n.speed-netcol .sico{position:absolute; inset-inline-start:12px; top:11px; width:28px; height:28px}",
     ".speed-netcol{min-width:0; padding:9px 13px; text-align:start; font:inherit; color:inherit; background:none; border-radius:0}")
sub4(".speed-lab{font-size:10px; font-weight:500; letter-spacing:0.05em; text-transform:uppercase; color:var(--txt3); white-space:nowrap; overflow:hidden; text-overflow:ellipsis}",
     ".speed-lab{font-size:10.5px; font-weight:500; letter-spacing:0.08em; text-transform:uppercase; color:var(--txt3)}")

# Run control: the full-width bar again.
sub4('''.speed-runrow{display:flex; align-items:center; justify-content:center; margin-top:16px}
#sp-run{width:112px; height:112px; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:6px; border-radius:50%; border:1px solid rgb(255 255 255 / 0.10); background:rgb(255 255 255 / 0.05); box-shadow:inset 0 1px 0 rgb(255 255 255 / 0.09); color:var(--txt); font-size:12.5px; font-weight:600}
#sp-run svg{width:19px; height:19px; color:var(--accent-strong)}
#sp-run .ico-stop{display:none}
#sp-run.stopping .ico-play{display:none}
#sp-run.stopping .ico-stop{display:block}
#sp-run:disabled{opacity:1}
#sp-run:active{transform:scale(0.98)}''',
'''.speed-runrow{display:flex; align-items:center; gap:10px; margin-top:16px}
#sp-run{flex:1; height:44px; display:flex; align-items:center; justify-content:center; gap:8px; border-radius:14px; border:1px solid transparent; background:var(--accent); color:#fff; font-size:13.5px; font-weight:600}
#sp-run svg{width:14px; height:14px}''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
