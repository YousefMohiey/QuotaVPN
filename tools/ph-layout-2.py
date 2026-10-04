import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")
old = '''      <label data-i18n="transport" id="transport-label">Connection</label>
      <div class="seg" id="transport-seg"><button data-transport="vless" class="on" data-i18n="trStandard">Standard</button><button data-transport="hy2" data-i18n="trGame">Hysteria2</button><button data-transport="wg" data-i18n="trWg">WireGuard</button></div>
      <p class="hint" id="transport-note"></p>'''
new = '''      <div class="tblock">
        <label data-i18n="transport" id="transport-label">Connection</label>
        <div class="seg" id="transport-seg"><button data-transport="vless" class="on" data-i18n="trStandard">Standard</button><button data-transport="hy2" data-i18n="trGame">Hysteria2</button><button data-transport="wg" data-i18n="trWg">WireGuard</button></div>
        <p class="hint" id="transport-note"></p>
      </div>'''
assert t.count(old) == 1
t = t.replace(old, new, 1)
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html tblock ok")

P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

old = ".view.on { display: flex; margin: 0; width: 100%; }"
new = ".view.on { display: flex; margin: 0; width: 100%; flex: 1 1 auto; min-height: 0; }"
assert t.count(old) == 1
t = t.replace(old, new, 1)

old = "#transport-label { margin-top: auto; }\n"
assert t.count(old) == 1
t = t.replace(old, "", 1)

old = "#view-connect > .hero, #view-connect > .card { flex-shrink: 0; }"
new = ("#view-connect > .hero, #view-connect > .card { flex-shrink: 0; }\n"
       "#setup-card { flex: 1 1 auto; display: flex; flex-direction: column; justify-content: space-between; }\n"
       ".tblock { display: flex; flex-direction: column; gap: 12px; }")
assert t.count(old) == 1
t = t.replace(old, new, 1)
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css fill ok")
