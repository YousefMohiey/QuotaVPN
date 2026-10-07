# Owner pass 3:
#  - Home preset cards go back to stacked rows (icon, name, domain, radio), no chips,
#    no left sub-heading, exactly like the picture.
#  - The location fact carries a chevron and re-detects on tap.
#  - The dial's inner block copies the old button: size-8 power, gap 2.5, 12.5px medium.
#  - The speed test ring takes the home dial's size and gap.
#  - Sentence punctuation leaves every UI string: desktop i18n, the phone's copy,
#    and the Rust messages.
import io, re, sys

ROOT = r"C:\Tools\QuotaCards"
FAIL = []

def rd(p):
    with io.open(p, "r", encoding="utf8", newline="") as f:
        return f.read()

def wr(p, s):
    with io.open(p, "w", encoding="utf8", newline="") as f:
        f.write(s)

def sub(path, old, new, label):
    s = rd(path)
    if old not in s:
        FAIL.append(label + " (anchor missing)")
        print("FAIL", label)
        return
    wr(path, s.replace(old, new, 1))
    print("ok  ", label)

def splice(path, start_anchor, end_anchor, new_middle, label):
    s = rd(path)
    i = s.find(start_anchor)
    j = s.find(end_anchor, i if i >= 0 else 0)
    if i < 0 or j < 0:
        FAIL.append(label + " (anchors missing)")
        print("FAIL", label)
        return
    wr(path, s[:i] + new_middle + s[j:])
    print("ok  ", label)

# ---------------------------------------------------------------- Dial.tsx
p = ROOT + r"\desktop\ui-next\src\components\Dial.tsx"
sub(p,
    '      <span className="flex flex-col items-center gap-2">\n'
    '        <Power className="size-[38px]" strokeWidth={1.7} aria-hidden />\n'
    '        <span className="text-[15px] font-semibold tracking-[-0.01em]">{label}</span>',
    '      {/* the old button, word for word: a size-8 power over a 12.5 label,\n'
    '          with the same 2.5 gap, so the word sits the way it always did */}\n'
    '      <span className="flex flex-col items-center gap-2.5">\n'
    '        <Power className="size-8" strokeWidth={1.75} aria-hidden />\n'
    '        <span className="text-[12.5px] font-medium tracking-[0.01em]">{label}</span>',
    "dial inner block")

# ---------------------------------------------------------------- Home.tsx
p = ROOT + r"\desktop\ui-next\src\screens\Home.tsx"
new_presets = '''          {/* the presets: two stacked rows, each carrying its own story */}
          <div className="flex min-w-0 flex-1 flex-col gap-2.5">
            {(["Gamerz", "Streamerz"] as const).map((kind: PresetKind) => {
              const mine = cards.find((c) => c.card_type === kind)
              const sni = mine?.sni ?? DEFAULT_SNI[kind]
              const isActive = preset === kind
              const Icon = kind === "Gamerz" ? Gamepad2 : Tv
              return (
                <button
                  key={kind}
                  type="button"
                  aria-pressed={isActive}
                  disabled={creating !== null}
                  onClick={() => {
                    if (creating) return
                    if (mine) {
                      setPreset(kind)
                      pickCard(mine.uuid)
                      return
                    }
                    setCreating(kind)
                    void ensurePresetCard(kind).finally(() => setCreating(null))
                  }}
                  className={cn(
                    "flex items-center gap-3 rounded-[12px] border p-4 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",
                    isActive
                      ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
                      : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",
                  )}
                >
                  <span
                    className={cn(
                      "grid size-11 shrink-0 place-items-center rounded-[10px] border transition-colors",
                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                        : "border-line bg-white/[0.03] text-txt3",
                    )}
                  >
                    <Icon className="size-[19px]" aria-hidden />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span
                      className={cn("block truncate text-[14.5px] font-semibold", isActive ? "text-txt" : "text-txt2")}
                      dir="auto"
                    >
                      {t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}
                    </span>
                    <span className="mt-1 block truncate font-mono text-[11.5px] text-txt3">{sni}</span>
                  </span>
                  <span
                    aria-hidden
                    className={cn(
                      "grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",
                      isActive ? "border-[var(--brand)]" : "border-line-strong",
                    )}
                  >
                    {isActive && <span className="size-2 rounded-full bg-[var(--brand)]" />}
                  </span>
                </button>
              )
            })}
          </div>

'''
splice(p,
      '          {/* the presets: two cards, each one carrying its own story */}',
      '          {/* the controls, one continuous column */}',
      new_presets,
      "home presets stacked")

# ---------------------------------------------------------------- Hero.tsx
p = ROOT + r"\desktop\ui-next\src\components\Hero.tsx"
# lucide import: add ChevronDown
import io as _io
s = rd(p)
m = re.search(r'import \{([^}]*)\} from "lucide-react"', s)
if m and "ChevronDown" not in m.group(1):
    old_imp = m.group(0)
    names = sorted([n.strip() for n in m.group(1).split(",") if n.strip()] + ["ChevronDown"])
    wr(p, s.replace(old_imp, "import { " + ", ".join(names) + ' } from "lucide-react"', 1))
    print("ok   hero lucide import")
else:
    print("FAIL hero lucide import")

# geo nonce + busy state
sub(p,
    '  const [geo, setGeo] = useState<{ ip: string; place: string; isp: string } | null>(() => {',
    '  const [geoNonce, setGeoNonce] = useState(0)\n'
    '  const [geoBusy, setGeoBusy] = useState(false)\n'
    '  const [geo, setGeo] = useState<{ ip: string; place: string; isp: string } | null>(() => {',
    "hero geo nonce state")

# the geo effect: busy flags + nonce dep
s = rd(p)
i = s.find('  useEffect(() => {\n    let alive = true\n    let timer: number | undefined\n    const grab = (n: number) => {')
if i < 0:
    FAIL.append("hero geo effect (start anchor)")
    print("FAIL hero geo effect")
else:
    j = s.find('  }, [connected])', i)
    if j < 0:
        FAIL.append("hero geo effect (end anchor)")
        print("FAIL hero geo effect end")
    else:
        j_end = j + len('  }, [connected])')
        new_eff = '''  useEffect(() => {
    let alive = true
    let timer: number | undefined
    setGeoBusy(true)
    const grab = (n: number) => {
      void netInfo()
        .then((v) => {
          if (!alive) return
          if (v && v.ip) {
            setGeo(v)
            setGeoBusy(false)
            try {
              localStorage.setItem("qc-geo", JSON.stringify(v))
            } catch {
              /* private mode */
            }
            return
          }
          if (n < 2) timer = window.setTimeout(() => grab(n + 1), 2500)
          else setGeoBusy(false)
        })
        .catch(() => {
          if (alive && n < 2) timer = window.setTimeout(() => grab(n + 1), 2500)
          else setGeoBusy(false)
        })
    }
    grab(0)
    return () => {
      alive = false
      if (timer) window.clearTimeout(timer)
    }
  }, [connected, geoNonce])'''
        wr(p, s[:i] + new_eff + s[j_end:])
        print("ok   hero geo effect")

# the location fact gets the chevron + refresh
sub(p,
    '<Fact icon={Globe} label={t("yourLocation")} value={placeShown} />',
    '<Fact\n'
    '          icon={Globe}\n'
    '          label={t("yourLocation")}\n'
    '          value={placeShown}\n'
    '          chevron\n'
    '          busy={geoBusy}\n'
    '          onClick={() => {\n'
    '            setGeoBusy(true)\n'
    '            setGeoNonce((n) => n + 1)\n'
    '          }}\n'
    '        />',
    "hero location fact")

# the Fact component: chevron + button behaviour
new_fact = '''/** One footer fact: icon tile, quiet label, one value line. The location
    cell carries a chevron and re-detects the address when tapped. */
function Fact({
  icon: Icon,
  label,
  value,
  dot,
  className,
  valueClass,
  chevron,
  busy,
  onClick,
}: {
  icon: LucideIcon
  label: string
  value: string
  dot?: boolean
  className?: string
  valueClass?: string
  chevron?: boolean
  busy?: boolean
  onClick?: () => void
}) {
  const body = (
    <>
      <span className="grid size-[30px] shrink-0 place-items-center rounded-[8px] border border-line bg-white/[0.03] text-brand-strong">
        <Icon className="size-[15px]" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="whitespace-nowrap text-[11px] text-txt3">{label}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[12.5px] text-txt">
          {dot !== undefined && (
            <span className={cn("size-[6px] shrink-0 rounded-full", dot ? "bg-[var(--green)]" : "bg-[var(--red)]")} aria-hidden />
          )}
          <span className={cn("truncate", valueClass)} dir="auto">
            {value}
          </span>
        </div>
      </div>
      {chevron && (
        <ChevronDown
          aria-hidden
          className={cn(
            "ms-auto size-4 shrink-0 text-txt3 transition-transform duration-300",
            busy && "rotate-180",
          )}
        />
      )}
    </>
  )
  if (onClick) {
    return (
      <button
        type="button"
        onClick={onClick}
        aria-label={label}
        className={cn("flex min-w-0 items-center gap-2.5 text-start", className)}
      >
        {body}
      </button>
    )
  }
  return <div className={cn("flex min-w-0 items-center gap-2.5", className)}>{body}</div>
}

'''
splice(p, 'function Fact({', 'function StatTile(', new_fact, "hero Fact component")

# ---------------------------------------------------------------- Speed.tsx
p = ROOT + r"\desktop\ui-next\src\screens\Speed.tsx"
sub(p,
    '"group relative grid size-[208px] place-items-center rounded-full border transition-colors duration-300",',
    '"group relative grid size-[172px] place-items-center rounded-full border transition-colors duration-300",',
    "speed ring size")
sub(p,
    'className={cn("flex w-full items-center", parked ? "justify-start gap-7" : "justify-center")}',
    'className={cn("flex min-h-[172px] w-full items-center", parked ? "justify-start gap-8" : "justify-center")}',
    "speed row"

)

# ---------------------------------------------------------------- punctuation
TRAIL = re.compile(r'^[.?!\u061f\u2026]+$')
def strip_line(line):
    # walk the line, strip trailing sentence punctuation inside "..." literals
    out = []
    i = 0
    while i < len(line):
        c = line[i]
        if c == '"':
            j = i + 1
            buf = []
            while j < len(line):
                if line[j] == "\\":
                    buf.append(line[j:j+2]); j += 2; continue
                if line[j] == '"':
                    break
                buf.append(line[j]); j += 1
            if j < len(line) and line[j] == '"':
                content = "".join(buf)
                stripped = content.rstrip(".?!\u061f\u2026 ")
                # keep numeric/technical values untouched
                if stripped != content and stripped and not re.search(r'\d$', stripped):
                    out.append('"' + stripped + '"')
                else:
                    out.append('"' + content + '"')
                i = j + 1
                continue
        out.append(c)
        i += 1
    return "".join(out)

for rel, label in [
    (r"\desktop\ui-next\src\lib\i18n\en.ts", "en.ts punctuation"),
    (r"\desktop\ui-next\src\lib\i18n\ar.ts", "ar.ts punctuation"),
    (r"\android\tauri-app\ui\app.js", "phone app.js punctuation"),
]:
    p = ROOT + rel
    src = rd(p)
    lines = src.split("\n")
    changed = 0
    res = []
    for ln in lines:
        stripped_ln = ln.lstrip()
        if stripped_ln.startswith("//") or stripped_ln.startswith("*") or stripped_ln.startswith("/*"):
            res.append(ln)
            continue
        nl = strip_line(ln)
        if nl != ln:
            changed += 1
        res.append(nl)
    wr(p, "\n".join(res))
    print("ok  ", label, "lines changed:", changed)

# rust messages: strip the trailing period inside sentence-like literals
for rel, label in [
    (r"\desktop\src-tauri\src\lib.rs", "lib.rs punctuation"),
    (r"\desktop\src-tauri\src\vpn.rs", "vpn.rs punctuation"),
]:
    p = ROOT + rel
    src = rd(p)
    lines = src.split("\n")
    changed = 0
    res = []
    for ln in lines:
        stripped_ln = ln.lstrip()
        if stripped_ln.startswith("//") or stripped_ln.startswith("*"):
            res.append(ln)
            continue
        def fix(m):
            global changed
            content = m.group(1)
            if content.endswith(".") and len(content) > 5 and content[0].isupper() and " " in content:
                changed += 1
                return '"' + content[:-1] + '"'
            return m.group(0)
        nl = re.sub(r'"([^"\\]*)"', fix, ln)
        res.append(nl)
    wr(p, "\n".join(res))
    print("ok  ", label, "literals changed:", changed)

if FAIL:
    print("\nFAILURES:", FAIL)
    sys.exit(1)
print("\nALL EDITS OK")
