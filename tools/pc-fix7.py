# Organize the Connection setup card so both columns share one rhythm:
#  - both columns open with the same heading, so their content starts on one line
#  - the preset cards grow to the controls column's height, so both end together
#  - the segmented control takes the other controls' height, so the third row
#    matches the first two and the row gaps read even
import io, sys

HOME = r"C:\Tools\QuotaCards\desktop\ui-next\src\screens\Home.tsx"
SEG = r"C:\Tools\QuotaCards\desktop\ui-next\src\components\Segmented.tsx"

s = io.open(HOME, "r", encoding="utf8", newline="").read()

# 1. left column: heading + cards that share the column height
old = '''          {/* the presets: two stacked rows, each carrying its own story */}
          <div className="flex min-w-0 flex-1 flex-col gap-2.5">
            {(["Gamerz", "Streamerz"] as const).map((kind: PresetKind) => {'''
new = '''          {/* the presets: two stacked rows, each carrying its own story.
              The column opens with a heading like the controls one, so both
              sides start on the same line, and the rows share out the height
              so both sides also end together. */}
          <div className="flex min-w-0 flex-1 flex-col gap-3">
            <h3 className="text-[13.5px] font-semibold text-txt">{t("presetHeading")}</h3>
            <div className="flex flex-1 flex-col gap-2.5">
              {(["Gamerz", "Streamerz"] as const).map((kind: PresetKind) => {'''
if old not in s:
    print("FAIL left column header anchor")
    sys.exit(1)
s = s.replace(old, new, 1)

# close the new wrapper
old = '''                </button>
              )
            })}
          </div>

          {/* the controls, one continuous column */}'''
new = '''                </button>
                )
              })}
            </div>
          </div>

          {/* the controls, one continuous column */}'''
if old not in s:
    print("FAIL left column close anchor")
    sys.exit(1)
s = s.replace(old, new, 1)

# 2. the preset button grows with the column
old = '"flex items-center gap-3 rounded-[12px] border p-4 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",'
new = '"flex flex-1 items-center gap-3 rounded-[12px] border p-4 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",'
if old not in s:
    print("FAIL preset button anchor")
    sys.exit(1)
s = s.replace(old, new, 1)

# 3. right column: heading only, top aligned with the left one
old = '''          <div className="flex shrink-0 flex-col justify-center gap-3 lg:w-[364px] lg:border-l lg:border-line lg:pl-6">
            <div>
              <h3 className="text-[13.5px] font-semibold text-txt">{t("configHeading")}</h3>
              <p className="mt-0.5 text-[12px] text-txt3">{t("configBody")}</p>
            </div>'''
new = '''          <div className="flex shrink-0 flex-col gap-3 lg:w-[364px] lg:border-l lg:border-line lg:pl-6">
            <h3 className="text-[13.5px] font-semibold text-txt">{t("configHeading")}</h3>'''
if old not in s:
    print("FAIL right column anchor")
    sys.exit(1)
s = s.replace(old, new, 1)

io.open(HOME, "w", encoding="utf8", newline="").write(s)
print("home setup card reorganised")

# 4. segmented options take the same height as the dropdown controls
t = io.open(SEG, "r", encoding="utf8", newline="").read()
old = '''              "relative rounded-[8px] py-1.5 transition-colors",'''
new = '''              "relative grid h-[34px] place-items-center rounded-[8px] transition-colors",'''
if old not in t:
    print("FAIL segmented anchor")
    sys.exit(1)
t = t.replace(old, new, 1)
io.open(SEG, "w", encoding="utf8", newline="").write(t)
print("segmented heights match the controls")
