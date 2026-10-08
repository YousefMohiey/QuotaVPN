# Organization pass, driven by the ui-ux-pro-max guidelines and
# design-taste-frontend's pre-flight:
#  - one page scaffold for every screen (same max width, same gap, same height
#    behaviour) instead of the three variants in the tree
#  - the type scale collapses from 18 steps to 10 (uz guideline: consistent
#    modular scale, no random sizes)
#  - every screen gets its missing h1 (sr-only) so the heading order is h1 -> h2 -> h3
#  - the location fact, the only clickable thing without hover feedback, gets
#    the same hover treatment as the other chevron rows
import io, re, os

ROOT = r"C:\Tools\QuotaCards\desktop\ui-next\src"
stats = []

def edit(path, old, new, label, count=1):
    s = io.open(path, "r", encoding="utf8", newline="").read()
    if old not in s:
        print("FAIL", label)
        return False
    io.open(path, "w", encoding="utf8", newline="").write(s.replace(old, new, count))
    print("ok  ", label)
    return True

# ---- 1. scaffolds -----------------------------------------------------------
edit(ROOT + r"\screens\Speed.tsx",
     'className="mx-auto flex min-h-[calc(100vh-84px)] w-full max-w-[1040px] flex-col gap-4"',
     'className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3"',
     "speed scaffold")
edit(ROOT + r"\screens\SpeedHistory.tsx",
     'className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-4"',
     'className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3"',
     "history scaffold")
edit(ROOT + r"\screens\SpeedResult.tsx",
     'className="mx-auto flex w-full max-w-[1040px] flex-col gap-4"',
     'className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3"',
     "result scaffold")
edit(ROOT + r"\screens\Apps.tsx",
     'className="mx-auto flex w-full max-w-[1040px] flex-col gap-3"',
     'className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3"',
     "apps scaffold")
edit(ROOT + r"\screens\Settings.tsx",
     'className="mx-auto flex w-full max-w-[1040px] flex-col gap-3"',
     'className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3"',
     "settings scaffold")

# ---- 2. type scale ----------------------------------------------------------
MERGES = [
    ("text-[10.5px]", "text-[11px]"),
    ("text-[10px]", "text-[11px]"),
    ("text-[12px]", "text-[12.5px]"),
    ("text-[13px]", "text-[13.5px]"),
    ("text-[14.5px]", "text-[15px]"),
    ("text-[14px]", "text-[15px]"),
    ("text-[15.5px]", "text-[15px]"),
    ("text-[17px]", "text-[16.5px]"),
]
changed = {k: 0 for k, _ in MERGES}
for base, _dirs, files in os.walk(ROOT):
    for f in files:
        if not f.endswith((".tsx", ".ts")):
            continue
        fp = os.path.join(base, f)
        s = io.open(fp, "r", encoding="utf8", newline="").read()
        o = s
        for old, new in MERGES:
            if old in s:
                changed[old] += s.count(old)
                s = s.replace(old, new)
        if s != o:
            io.open(fp, "w", encoding="utf8", newline="").write(s)
print("type scale merges:", {k: v for k, v in changed.items() if v})

# ---- 3. sr-only h1 per screen ----------------------------------------------
H1 = {
    r"\screens\Home.tsx": 'tabHome',
    r"\screens\Speed.tsx": 'tabSpeed',
    r"\screens\SpeedHistory.tsx": 'tabHistory',
    r"\screens\Settings.tsx": 'tabSettings',
    r"\screens\Apps.tsx": 'routing',
}
for rel, key in H1.items():
    p = ROOT + rel
    s = io.open(p, "r", encoding="utf8", newline="").read()
    anchor = 'max-w-[1040px] flex-col gap-3">\n'
    if anchor not in s:
        print("FAIL h1 anchor", rel)
        continue
    h1 = anchor + '      <h1 className="sr-only">{t("' + key + '")}</h1>\n'
    io.open(p, "w", encoding="utf8", newline="").write(s.replace(anchor, h1, 1))
    print("ok   h1", rel)

# ---- 4. hover feedback on the location fact ---------------------------------
p = ROOT + r"\components\Hero.tsx"
s = io.open(p, "r", encoding="utf8", newline="").read()
old_btn = '''        className={cn("flex min-w-0 items-center gap-2.5 text-start", className)}'''
new_btn = '''        className={cn("group flex min-w-0 items-center gap-2.5 text-start", className)}'''
old_chev = '''          className={cn(
            "ms-auto size-4 shrink-0 text-txt3 transition-transform duration-300",
            busy && "rotate-180",
          )}'''
new_chev = '''          className={cn(
            "ms-auto size-4 shrink-0 text-txt3 transition-[color,transform] duration-300",
            "group-hover:translate-y-px group-hover:text-brand-strong",
            busy && "rotate-180",
          )}'''
if old_btn in s and old_chev in s:
    s = s.replace(old_btn, new_btn, 1).replace(old_chev, new_chev, 1)
    io.open(p, "w", encoding="utf8", newline="").write(s)
    print("ok   location fact hover")
else:
    print("FAIL location fact hover")
