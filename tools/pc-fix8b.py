# Re-apply the edits pc-fix8.py collected but never wrote (its fail-guard
# exited before the save). Writes unconditionally, reports every miss.
import io

HOME = r"C:\Tools\QuotaCards\desktop\ui-next\src\screens\Home.tsx"
s = io.open(HOME, "r", encoding="utf8", newline="").read()
hit = miss = 0

def sub(old, new, label, n=1):
    global s, hit, miss
    if old not in s:
        print("MISS", label)
        miss += 1
        return
    s = s.replace(old, new, n)
    print("ok  ", label)
    hit += 1

sub('<h2 className="text-[16.5px] font-semibold text-txt">{t("setupTitle")}</h2>',
    '<h2 className="text-[20px] font-semibold text-txt">{t("setupTitle")}</h2>',
    "card heading 20px")
sub('"flex items-center gap-3 rounded-[12px] border p-4 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",',
    '"flex items-center gap-3.5 rounded-[12px] border p-5 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",',
    "preset row padding")
sub('"grid size-11 shrink-0 place-items-center rounded-[10px] border transition-colors",',
    '"grid size-12 shrink-0 place-items-center rounded-[10px] border transition-colors",',
    "preset tile 48px")
sub('<Icon className="size-[19px]" aria-hidden />',
    '<Icon className="size-[20px]" aria-hidden />',
    "preset glyph size")
sub('className={cn("block truncate text-[14.5px] font-semibold", isActive ? "text-txt" : "text-txt2")}',
    'className={cn("block truncate text-[15px] font-semibold", isActive ? "text-txt" : "text-txt2")}',
    "preset name size")
sub('className="group flex h-10 w-full items-center justify-between gap-3 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"',
    'className="group flex h-12 w-full items-center justify-between gap-3 rounded-[10px] border border-line bg-white/[0.02] px-3.5 text-[13.5px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"',
    "dropdown boxes h-12", 2)
sub('className="h-10 rounded-[10px] border-line bg-white/[0.02] text-[13px]"',
    'className="h-12 rounded-[10px] border-line bg-white/[0.02] text-[13.5px]"',
    "custom input h-12")
sub('className="h-10 shrink-0 rounded-[10px] px-3.5 text-[13px]"',
    'className="h-12 shrink-0 rounded-[10px] px-3.5 text-[13.5px]"',
    "custom apply h-12")
sub('"grid size-[26px] shrink-0 place-items-center rounded-[8px]",',
    '"grid size-7 shrink-0 place-items-center rounded-[8px]",',
    "control tile 28px")
sub('<Icon className="size-[13px]" aria-hidden />',
    '<Icon className="size-[14px]" aria-hidden />',
    "control glyph size")

io.open(HOME, "w", encoding="utf8", newline="").write(s)
print("\napplied", hit, "missed", miss)
