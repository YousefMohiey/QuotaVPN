import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

GAUGE = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4.5 18.5a9 9 0 1 1 15 0"/><path d="M12 14.5l4.4-4.4"/><circle cx="12" cy="14.5" r="1.3"/></svg>'
ACT = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 12h4l2.5-6.5 4 13L16 12h5"/></svg>'
DWN = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4.5v14"/><path d="M6.5 13l5.5 5.5L17.5 13"/></svg>'
UP = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19.5v-14"/><path d="M6.5 11l5.5-5.5L17.5 11"/></svg>'
WIFI = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M2.5 9.2a15 15 0 0 1 19 0"/><path d="M6 12.6a10 10 0 0 1 12 0"/><path d="M9.4 16a5 5 0 0 1 5.2 0"/><circle cx="12" cy="19.3" r="0.9"/></svg>'
SRV = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="7" rx="2"/><rect x="3" y="13" width="18" height="7" rx="2"/><path d="M7 7.5h.01"/><path d="M7 16.5h.01"/></svg>'
GLOBE = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.5 2.6 3.8 5.6 3.8 9S14.5 18.4 12 21c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z"/></svg>'
PIN = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 21s7-5.6 7-11a7 7 0 1 0-14 0c0 5.4 7 11 7 11z"/><circle cx="12" cy="10" r="2.5"/></svg>'
ARR = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 4v12"/><path d="M3 8l4-4 4 4"/><path d="M17 20V8"/><path d="M21 16l-4 4-4-4"/></svg>'

# ============================================================ HTML =========
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# Fact rows carry the desktop's icon box again.
sub('''        <div class="fact">
          <span class="fact-txt"><i data-i18n="srvLocation">Server location</i><b id="fact-place" dir="auto">-</b></span>
        </div>''',
'''        <div class="fact">
          <span class="fact-ico" aria-hidden="true">''' + GLOBE + '''</span>
          <span class="fact-txt"><i data-i18n="srvLocation">Server location</i><b id="fact-place" dir="auto">-</b></span>
        </div>''')
sub('''        <div class="fact" id="ip-row" role="button" tabindex="0">
          <span class="fact-txt"><i data-i18n="yourIp">Your IP</i><b id="home-ip" dir="ltr">-</b></span>
        </div>''',
'''        <div class="fact" id="ip-row" role="button" tabindex="0">
          <span class="fact-ico" aria-hidden="true">''' + PIN + '''</span>
          <span class="fact-txt"><i data-i18n="yourIp">Your IP</i><b id="home-ip" dir="ltr">-</b></span>
        </div>''')
sub('''        <div class="fact">
          <span class="fact-txt"><i data-i18n="statusLbl">Status</i><b class="fact-status"><span class="fact-dot" id="fact-dot"></span><span id="fact-status">Not connected</span></b></span>
        </div>''',
'''        <div class="fact">
          <span class="fact-ico" aria-hidden="true">''' + ARR + '''</span>
          <span class="fact-txt"><i data-i18n="statusLbl">Status</i><b class="fact-status"><span class="fact-dot" id="fact-dot"></span><span id="fact-status">Not connected</span></b></span>
        </div>''')

# Speed page: the metric cells and the fact cells take icon boxes too.
sub('<button type="button" class="sstat" data-which="ping"><span data-i18n="pingTitle">Ping</span>',
    '<button type="button" class="sstat" data-which="ping"><span class="sico">' + GAUGE + '</span><span data-i18n="pingTitle">Ping</span>')
sub('<button type="button" class="sstat" data-which="ping2"><span data-i18n="jitter">Jitter</span>',
    '<button type="button" class="sstat" data-which="ping2"><span class="sico">' + ACT + '</span><span data-i18n="jitter">Jitter</span>')
sub('<button type="button" class="sstat" data-which="down"><span data-i18n="chDown">Down</span>',
    '<button type="button" class="sstat" data-which="down"><span class="sico">' + DWN + '</span><span data-i18n="chDown">Down</span>')
sub('<button type="button" class="sstat" data-which="up"><span data-i18n="chUp">Up</span>',
    '<button type="button" class="sstat" data-which="up"><span class="sico">' + UP + '</span><span data-i18n="chUp">Up</span>')

sub('''        <div class="speed-netcol">
          <div class="speed-lab" data-i18n="yourConn">Your connection</div>''',
'''        <div class="speed-netcol">
          <span class="sico" aria-hidden="true">''' + WIFI + '''</span>
          <div class="speed-lab" data-i18n="yourConn">Your connection</div>''')
sub('''        <button type="button" class="speed-netcol tap" id="sp-server-panel" aria-haspopup="dialog">
          <span class="speed-lab">''',
'''        <button type="button" class="speed-netcol tap" id="sp-server-panel" aria-haspopup="dialog">
          <span class="sico" aria-hidden="true">''' + SRV + '''</span>
          <span class="speed-lab">''')

# The run control becomes the desktop's circle: one disc, play then stop.
sub('''        <button type="button" id="sp-run"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg><span id="sp-run-label" data-i18n="startTest">Start test</span></button>
        <button type="button" id="sp-stop" hidden data-i18n="stop">Stop</button>''',
'''        <button type="button" id="sp-run" aria-label="Start test"><svg class="ico-play" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg><svg class="ico-stop" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="7" y="7" width="10" height="10" rx="2"/></svg><span id="sp-run-label" data-i18n="startTest">Start test</span></button>''')

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
  const btn = $("sp-run");
  if (btn) btn.disabled = !!spCtl;
  const lbl = $("sp-run-label");
  if (lbl) lbl.textContent = spCtl ? t("measuring") : t("startTest");
  const stop = $("sp-stop");
  if (stop) stop.hidden = !spCtl;
  document.querySelectorAll(".sstat").forEach((b) => { b.disabled = !!spCtl; });
}''',
'''function spPaintRunState() {
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
}''')

sub3('$("sp-run").onclick = () => { void spRun("all"); };\n$("sp-stop").onclick = spStop;',
     '$("sp-run").onclick = () => { if (spCtl) spStop(); else void spRun("all"); };')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("js ok")

# ============================================================ CSS ==========
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub4(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)

# 1. The desktop's own ladder: lighter window, lighter panels.
sub4('''  --bg0: #10141c;
  --bg1: #131a29;
  --panel: #111826;
  --panel-hover: #161e30;
  --card: #111826;
  --field: #1a2338;''',
'''  --bg0: #141a25;
  --bg1: #172032;
  --panel: #151d2e;
  --panel-hover: #1b2539;
  --card: #151d2e;
  --field: #202b45;''')
sub4("  --glass-fill: rgb(16 20 28 / 0.55);", "  --glass-fill: rgb(21 29 46 / 0.62);")

# Floating layers follow the same lift.
sub4('''  background: rgb(16 20 28 / 0.72);
  backdrop-filter: blur(26px) saturate(150%);''',
'''  background: rgb(23 32 50 / 0.8);
  backdrop-filter: blur(26px) saturate(150%);''')
sub4('''  background: rgb(18 24 36 / 0.85);
  backdrop-filter: blur(30px) saturate(150%);''',
'''  background: rgb(21 29 46 / 0.92);
  backdrop-filter: blur(30px) saturate(150%);''')
sub4('''  .sec, .hero, .card { background: rgb(16 20 28 / 0.96); }
  .tabbar { background: rgb(16 20 28 / 0.98); }
  .sheet { background: rgb(18 24 36 / 0.98); }''',
'''  .sec, .hero, .card { background: rgb(21 29 46 / 0.96); }
  .tabbar { background: rgb(23 32 50 / 0.98); }
  .sheet { background: rgb(21 29 46 / 0.98); }''')

# 2. Icon box: the desktop's FactRow/MetricTile anatomy.
sub4(".fact { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; min-width: 0; padding: 0; }",
'''.fact { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; min-width: 0; padding: 0; }
.fact-ico { width: 30px; height: 30px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 10px; background: rgb(255 255 255 / 0.03); color: var(--accent-strong); }
.fact-ico svg { width: 15px; height: 15px; }''')
sub4(".fact-txt i { font-style: normal; font-size: 11px; line-height: 1.25; color: var(--txt3); }",
    ".fact-txt i { font-style: normal; font-size: 10.5px; font-weight: 500; letter-spacing: 0.08em; text-transform: uppercase; line-height: 1.25; color: var(--txt3); }")

# 3. Metric cells: bordered, rounded, icon box on top.
sub4(".speed-stats{display:grid; grid-template-columns:repeat(4,1fr); margin-top:14px; border-top:1px solid var(--line)}",
    ".speed-stats{display:grid; grid-template-columns:repeat(4,1fr); margin-top:14px; border:1px solid var(--line); border-radius:14px; background:rgb(255 255 255 / 0.02); overflow:hidden}")
sub4(".sstat{appearance:none; background:none; border:0; padding:8px 2px; display:flex; flex-direction:column; align-items:flex-start; gap:3px; color:inherit; text-align:start; min-width:0}",
'''.sstat{appearance:none; background:none; border:0; padding:11px 5px 11px 9px; display:flex; flex-direction:column; align-items:flex-start; gap:3px; color:inherit; text-align:start; min-width:0}
.sico{width:26px; height:26px; flex:none; display:grid; place-items:center; border:1px solid var(--line); border-radius:9px; background:rgb(255 255 255 / 0.03); color:var(--accent-strong)}
.sico svg{width:14px; height:14px}
.sstat .sico{margin-bottom:5px}''')
sub4(".sstat + .sstat{border-left:1px solid var(--line); padding-left:10px}", ".sstat + .sstat{border-left:1px solid var(--line)}")

# 4. Net card cells: icon box left, text column right (desktop FactRow).
sub4(".speed-netcol{min-width:0; padding:9px 13px; text-align:start; font:inherit; color:inherit; background:none; border-radius:0}",
    ".speed-netcol{min-width:0; padding:11px 12px; text-align:start; font:inherit; color:inherit; background:none; border-radius:0; display:grid; grid-template-columns:28px 1fr; column-gap:10px; align-items:center}\n.speed-netcol .sico{grid-row:1 / -1; width:28px; height:28px}")

# 5. The run disc.
sub4('''.speed-runrow{display:flex; align-items:center; gap:10px; margin-top:16px}
#sp-run{flex:1; height:44px; display:flex; align-items:center; justify-content:center; gap:8px; border-radius:14px; border:1px solid transparent; background:var(--accent); color:#fff; font-size:13.5px; font-weight:600}
#sp-run svg{width:14px; height:14px}''',
'''.speed-runrow{display:flex; align-items:center; justify-content:center; margin-top:16px}
#sp-run{width:112px; height:112px; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:6px; border-radius:50%; border:1px solid rgb(255 255 255 / 0.10); background:rgb(255 255 255 / 0.05); box-shadow:inset 0 1px 0 rgb(255 255 255 / 0.09); color:var(--txt); font-size:12.5px; font-weight:600}
#sp-run svg{width:19px; height:19px; color:var(--accent-strong)}
#sp-run .ico-stop{display:none}
#sp-run.stopping .ico-play{display:none}
#sp-run.stopping .ico-stop{display:block}
#sp-run:disabled{opacity:1}
#sp-run:active{transform:scale(0.98)}''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
