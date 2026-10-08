# Follow the owner's UI picture for the Connection setup card:
#  - equal columns (the right one was pinned to 364px)
#  - preset rows at the picture's proportions: 20px padding, 48px tile, 15px name
#  - control rows on a 48px height with a 16px gap, matching the picture
#  - the Connection type segmented sits in the same 48px framed box as the two
#    dropdowns, with the selected option as a blue pill
#  - headings up to the picture's scale (20px card, 17px column)
import io, re, sys

HOME = r"C:\Tools\QuotaCards\desktop\ui-next\src\screens\Home.tsx"
s = io.open(HOME, "r", encoding="utf8", newline="").read()
orig = s
FAIL = []

def sub(old, new, label, n=1):
    global s
    if old not in s:
        FAIL.append(label)
        print("FAIL", label)
        return
    s = s.replace(old, new, n)
    print("ok  ", label)

# 1. card heading size
sub('<h2 className="text-[16.5px] font-semibold text-txt">{t("setupTitle")}</h2>',
    '<h2 className="text-[20px] font-semibold text-txt">{t("setupTitle")}</h2>',
    "card heading 20px")

# 2. preset row proportions
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

# 3. columns become equal halves
sub('<div className="flex shrink-0 flex-col justify-center gap-3 lg:w-[364px] lg:border-l lg:border-line lg:pl-6">\n            <h3 className="text-[13.5px] font-semibold text-txt">{t("configHeading")}</h3>\n            <p className="mt-0.5 text-[12.5px] text-txt3">{t("configBody")}</p>',
    '<div className="flex min-w-0 flex-1 flex-col gap-4 lg:border-l lg:border-line lg:pl-6">\n            <div>\n              <h3 className="text-[17px] font-semibold text-txt">{t("configHeading")}</h3>\n              <p className="mt-1 text-[13.5px] text-txt2">{t("configBody")}</p>\n            </div>',
    "right column equal half + heading block")

# 4. the two dropdown boxes take the picture's height
sub('className="group flex h-10 w-full items-center justify-between gap-3 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"',
    'className="group flex h-12 w-full items-center justify-between gap-3 rounded-[10px] border border-line bg-white/[0.02] px-3.5 text-[13.5px] text-txt transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)]"',
    "server + routing boxes h-12", 2)

# 5. the custom domain editor matches
sub('className="h-10 rounded-[10px] border-line bg-white/[0.02] text-[13px]"',
    'className="h-12 rounded-[10px] border-line bg-white/[0.02] text-[13.5px]"',
    "custom input h-12")
sub('className="h-10 shrink-0 rounded-[10px] px-3.5 text-[13px]"',
    'className="h-12 shrink-0 rounded-[10px] px-3.5 text-[13.5px]"',
    "custom apply h-12")

# 6. the segmented takes the framed box
sub('''                <Segmented
                  id="transport"
                  value={transport}
                  onChange={setTransport}
                  fill''',
    '''                <Segmented
                  id="transport"
                  value={transport}
                  onChange={setTransport}
                  boxed''',
    "transport segmented boxed")

# 7. control row label scale + tile size
sub('"flex w-[138px] shrink-0 items-center gap-2 pt-[2px] text-[12.5px] text-txt2"',
    '"flex w-[150px] shrink-0 items-center gap-2.5 text-[13.5px] text-txt2"',
    "control label scale")
sub('"grid size-[26px] shrink-0 place-items-center rounded-[8px]",',
    '"grid size-7 shrink-0 place-items-center rounded-[8px]",',
    "control tile 28px")
sub('<Icon className="size-[13px]" aria-hidden />',
    '<Icon className="size-[14px]" aria-hidden />',
    "control glyph size")

if FAIL:
    print("\nFAILURES:", FAIL)
    sys.exit(1)
io.open(HOME, "w", encoding="utf8", newline="").write(s)
print("\nhome follows the picture")
