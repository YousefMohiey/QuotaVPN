import io
P = r"C:\Tools\QuotaCards\android\tauri-app\ui\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def sub4(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])
    t = t.replace(old, new, cnt)
# ---- the header
sub4('''/* Hero: status up top, one switch below. Compact: Home never scrolls. */''',
'''.app-head { display: flex; align-items: center; gap: 10px; padding: 2px 2px 0; }
.ah-logo { width: 34px; height: 34px; flex: none; border-radius: 9px; }
.ah-txt { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.15; }
.ah-txt b { font-size: 15px; font-weight: 700; letter-spacing: -0.01em; color: var(--txt); }
.ah-txt i { font-style: normal; font-size: 11px; color: var(--txt3); }
.ah-chip { flex: none; display: flex; align-items: center; gap: 6px; min-height: 36px; padding: 0 10px; border-radius: 11px; border: 1px solid var(--line); background: rgb(255 255 255 / 0.03); color: var(--txt2); font-size: 12.5px; }
.ah-chip svg { width: 14px; height: 14px; color: var(--accent-strong); flex: none; }
.ah-chip .chev { width: 13px; height: 13px; color: var(--txt3); }
.ah-chip:active { background: rgb(255 255 255 / 0.06); }

.hero {''')

# ---- the dial: smaller with the arc, roomier label, state line
sub4("""  width: 180px;
  height: 180px;""", """  width: 152px;
  height: 152px;""")
sub4(".hero-btn .power { width: 50px; height: 50px; position: relative; z-index: 1; }",
'''.hero-btn .power { width: 42px; height: 42px; position: relative; z-index: 1; }
.dial-arc { position: absolute; inset: -4px; width: calc(100% + 8px); height: calc(100% + 8px); pointer-events: none; }
.dial-arc circle { fill: none; stroke: var(--accent-strong); stroke-width: 3; stroke-linecap: round; stroke-dasharray: 148 148; transform: rotate(-90deg) scale(-1, 1); transform-origin: center; }
.hero.connected .dial-arc circle { stroke: var(--green); }
.hero.connecting .dial-arc circle { stroke: var(--accent-strong); animation: pulse 1.1s ease-in-out infinite; }''')
sub4(".btn-action { font-size: 13.5px; line-height: 1.3; font-weight: 570; color: var(--txt2); margin: 6px 0 4px; }",
     ".btn-action { font-size: 19px; line-height: 1.2; font-weight: 700; letter-spacing: -0.01em; color: var(--txt); margin: 8px 0 2px; }")

# ---- the state line under the action word
sub4(".hero-sub { font-size: 12.5px;",
'''.hero-line { display: inline-flex; align-items: center; gap: 7px; margin: 5px 0 0; font-size: 12.5px; color: var(--txt2); }
.hl-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--red); flex: none; }
.hero.connected .hl-dot { background: var(--green); }
.hero.connecting .hl-dot { background: var(--accent-strong); animation: pulse 1.1s ease-in-out infinite; }
.hero-sub { font-size: 12.5px;''')

# ---- the facts panel: three cells, icon boxes, dividers
sub4(".facts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin: 18px 0 0; border-top: 1px solid var(--line); padding-top: 15px; }",
'''.facts-panel { display: grid; grid-template-columns: repeat(3, 1fr); padding: 12px 12px; }
.facts-panel .fact { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 0; }
.facts-panel .fact + .fact { border-left: 1px solid var(--line); padding-left: 10px; }
.fact-ico { width: 26px; height: 26px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 9px; background: rgb(255 255 255 / 0.03); color: var(--accent-strong); }
.fact-ico svg { width: 13px; height: 13px; }
.fact-ico .ip-mark { font-size: 9.5px; font-weight: 700; letter-spacing: 0.02em; color: var(--accent-strong); }''')

# ---- the setup: sub line, radio rings, row icons, connection type head
sub4(".card h2 { margin: 0 2px 4px;", ".setup-sub { margin: 3px 2px 0; font-size: 12px; line-height: 1.4; color: var(--txt3); }\n.card h2 { margin: 0 2px 0;")
sub4(".preset-sni{display:block; width:100%; font-size:10.5px; line-height:1.3; color:var(--txt3); font-variant-numeric:tabular-nums; overflow:hidden; text-overflow:ellipsis; white-space:nowrap}",
'''.preset-sni{display:block; width:100%; font-size:10.5px; line-height:1.3; color:var(--txt3); font-variant-numeric:tabular-nums; overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
.preset-radio{width:18px; height:18px; flex:none; display:grid; place-items:center; border:1.5px solid var(--line-strong); border-radius:50%}
.preset.on .preset-radio{border-color:var(--accent-strong)}
.preset-dot{width:9px; height:9px; border-radius:50%; background:var(--accent-strong); opacity:0}
.preset.on .preset-dot{opacity:1}''')
sub4(".homenet-row { display: flex; align-items: center; gap: 8px; padding: 13px 2px;",
'''.row-ico { width: 26px; height: 26px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 9px; background: rgb(255 255 255 / 0.03); color: var(--txt2); }
.row-ico svg { width: 13px; height: 13px; }
.homenet-row { display: flex; align-items: center; gap: 10px; padding: 13px 2px;''')
sub4(".homenet-label { color: var(--txt3); font-size: 11.5px; line-height: 1.3; font-weight: 500; letter-spacing: 0.01em; flex: none; }",
'''.homenet-label { color: var(--txt2); font-size: 13px; line-height: 1.3; font-weight: 500; flex: none; }
.ct-head { display: inline-flex; align-items: center; gap: 6px; align-self: stretch; }
.ct-head .info-i { display: grid; place-items: center; width: 15px; height: 15px; color: var(--txt3); }
.ct-head .info-i svg { width: 13px; height: 13px; }''')

# ---- the segmented control: track with hairline dividers, blue gradient
#      selected pill with a soft lit edge
sub4('''.seg {
  display: flex;
  gap: 3px;
  margin: 2px 0 2px;
  padding: 3px;
  background: rgb(255 255 255 / 0.02);
  border: 1px solid var(--line);
  border-radius: var(--r-ctl);
}''',
'''.seg {
  display: flex;
  gap: 0;
  margin: 2px 0 2px;
  padding: 3px;
  background: rgb(255 255 255 / 0.02);
  border: 1px solid var(--line);
  border-radius: 13px;
}''')
sub4(".seg button { flex: 1; font-weight: 500; border: none; background: transparent; color: var(--txt3); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }",
'''.seg button { flex: 1; font-weight: 500; border: none; background: transparent; color: var(--txt3); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; border-radius: 10px; }
.seg button + button { border-left: 1px solid var(--line); border-top-left-radius: 0; border-bottom-left-radius: 0; }
.seg button.on + button { border-left-color: transparent; }''')
sub4('''.seg button.on {
  background: rgb(255 255 255 / 0.10);
  color: var(--txt);
  box-shadow: none;
}''',
'''.seg button.on {
  background: linear-gradient(180deg, var(--accent-hi), var(--accent));
  color: #fff;
  border-left-color: transparent;
  box-shadow: 0 0 0 1px rgb(91 141 239 / 0.45), 0 4px 14px rgb(31 89 182 / 0.35);
}''')

# ---- the nav: blue active with an underline and a soft backdrop
sub4('''.tabbar button.on {
  color: var(--txt);
  background: rgb(255 255 255 / 0.09);
  border-color: var(--line-strong);
}''',
'''.tabbar button.on {
  color: var(--accent-strong);
  background: rgb(255 255 255 / 0.06);
  border-color: transparent;
}
.tabbar button.on span { color: var(--accent-strong); }
.tabbar button.on::after { content: ""; width: 16px; height: 2px; border-radius: 2px; background: var(--accent-strong); margin-top: 1px; }''')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
