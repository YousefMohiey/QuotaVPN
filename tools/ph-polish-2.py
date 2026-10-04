import io
UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ---------------------------------------------------------------- css ------
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")
old = """.opt .check { width: 22px; height: 22px; flex: none; margin-inline-start: auto; color: var(--accent-strong); visibility: hidden; }
.opt[aria-selected="true"] .check { visibility: visible; }
.opt .check { color: #fff; }
.opt .check svg { stroke-width: 2.8; }"""
new = """/* Selection marks, two shapes: the sheet pickers carry one check on the
   chosen row only; app rows carry a checkbox each, empty until picked
   (the desktop's own anatomy: 18px circle, 1.5px edge, filled brand blue
   with a white check when on). */
.opt .check { flex: none; margin-inline-start: auto; color: #fff; }
.opt .check svg { stroke-width: 2.8; }
.sheet-list .opt .check { width: 22px; height: 22px; visibility: hidden; }
.sheet-list .opt[aria-selected="true"] .check { visibility: visible; }
.apps-list .opt .check { width: 18px; height: 18px; display: grid; place-items: center; border: 1.5px solid var(--line-strong); border-radius: 50%; }
.apps-list .opt .check svg { width: 12px; height: 12px; opacity: 0; }
.apps-list .opt[aria-selected="true"] .check { border-color: var(--accent); background: var(--accent); }
.apps-list .opt[aria-selected="true"] .check svg { opacity: 1; }
.apps-list.mode-all .opt { opacity: 0.7; }"""
assert t.count(old) == 1
t = t.replace(old, new, 1)
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")

# ---------------------------------------------------------------- js -------
P = UI + r"\app.js"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")
old = 'const CHECK_SVG = `<svg class="check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 13l4 4L19 7"/></svg>`;'
new = 'const CHECK_SVG = `<span class="check"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg></span>`;'
assert t.count(old) == 1
t = t.replace(old, new, 1)

old = """function renderAppsList(filter) {
  const list = $("apps-list");
  list.innerHTML = "";"""
new = """function renderAppsList(filter) {
  const list = $("apps-list");
  list.innerHTML = "";
  // In "all apps" the rows are a reference, not a checklist: dim them.
  list.classList.toggle("mode-all", appsMode === "all");"""
assert t.count(old) == 1
t = t.replace(old, new, 1)
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("js ok")
