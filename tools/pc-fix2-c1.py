import io

ROOT = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p):
    return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:4000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))

def sub(t, old, new, n=1):
    assert t.count(old) == n, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, n)

# ---- ipc.ts: the icon command ----
P = ROOT + r"\lib\ipc.ts"
t = load(P)
t = sub(t, '/** Exit-address facts for the speed page, resolved backend-side. */',
        '/** Per-app icons as base64 PNGs, pulled straight from the executables.\n'
        '    An empty string means the backend could not read one. */\n'
        'export type AppIcon = { pkg: string; png: string }\n'
        'export const appIcons = (paths: string[]): Promise<AppIcon[]> => call<AppIcon[]>("app_icons", { paths })\n\n'
        '/** Exit-address facts for the speed page, resolved backend-side. */')
save(P, t)
print("ipc.ts ok")

# ---- mock.ts: no icons in the preview ----
P = ROOT + r"\lib\mock.ts"
t = load(P)
anchor = '    case "tunnel_apps":'
assert t.count(anchor) == 1
t = sub(t, anchor,
        '    case "app_icons":\n      return []\n' + anchor)
save(P, t)
print("mock.ts ok")

# ---- i18n: the little "selected" word for the count ----
P = ROOT + r"\lib\i18n\en.ts"
t = load(P)
t = sub(t, '  "selected": "Selected",\n', '  "selected": "Selected",\n  "selOf": "selected",\n')
save(P, t)
P = ROOT + r"\lib\i18n\ar.ts"
t = load(P)
t = sub(t, '  "selected": "\u0627\u0644\u0645\u062d\u062f\u062f",\n',
        '  "selected": "\u0627\u0644\u0645\u062d\u062f\u062f",\n  "selOf": "\u0645\u062d\u062f\u062f",\n')
save(P, t)
print("i18n selOf ok")
