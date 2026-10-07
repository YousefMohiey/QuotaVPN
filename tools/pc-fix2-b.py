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

# ================= Home.tsx: preset cards + Configuration heading =============
P = ROOT + r"\screens\Home.tsx"
t = load(P)
start = t.index("          {/* the presets */}")
end = t.index("          {/* the controls, one continuous column */}")
new_block = '''          {/* the presets: two cards, each one carrying its own story */}
          <div className="flex min-w-0 flex-1 flex-col">
            <h3 className="text-[13.5px] font-semibold text-txt">{t("presetHeading")}</h3>
            <div className="mt-2.5 grid grid-cols-2 gap-3">
              {(["Gamerz", "Streamerz"] as const).map((kind: PresetKind) => {
                const mine = cards.find((c) => c.card_type === kind)
                const sni = mine?.sni ?? DEFAULT_SNI[kind]
                const isActive = preset === kind
                const Icon = kind === "Gamerz" ? Gamepad2 : Tv
                const tags =
                  kind === "Gamerz"
                    ? [t("tagGaming"), t("tagLowLatency"), t("tagEaGames")]
                    : [t("tagStreaming"), t("tagVideoPlatforms"), t("tagHighStability")]
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
                      "flex min-h-[132px] flex-col rounded-[12px] border p-3.5 text-start transition-colors duration-200 disabled:cursor-wait disabled:opacity-70",
                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
                        : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",
                    )}
                  >
                    <span className="flex items-center justify-between gap-2">
                      <span
                        className={cn(
                          "grid size-9 shrink-0 place-items-center rounded-[10px] border transition-colors",
                          isActive
                            ? "border-[var(--brand-line)] bg-[var(--brand-bg)] text-brand-strong"
                            : "border-line bg-white/[0.03] text-txt3",
                        )}
                      >
                        <Icon className="size-4" aria-hidden />
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
                    </span>
                    <span
                      className={cn("mt-2.5 block truncate text-[13.5px] font-semibold", isActive ? "text-txt" : "text-txt2")}
                      dir="auto"
                    >
                      {t(kind === "Gamerz" ? "kindGamerz" : "kindStreamerz")}
                    </span>
                    <span className="mt-0.5 block truncate font-mono text-[11px] text-txt3">{sni}</span>
                    <span className="mt-auto flex flex-wrap gap-1.5 pt-3">
                      {tags.map((tag) => (
                        <span key={tag} className="rounded-full border border-line px-2 py-[3px] text-[10.5px] text-txt3">
                          {tag}
                        </span>
                      ))}
                    </span>
                  </button>
                )
              })}
            </div>
          </div>

'''
t = t[:start] + new_block + t[end:]
t = sub(t, '          <div className="flex shrink-0 flex-col justify-center gap-3 lg:w-[436px] lg:border-l lg:border-line lg:pl-6">\n            <ControlRow label={t("domainSni")} icon={ServerIcon}>',
        '          <div className="flex shrink-0 flex-col justify-center gap-3 lg:w-[436px] lg:border-l lg:border-line lg:pl-6">\n'
        '            <div>\n'
        '              <h3 className="text-[13.5px] font-semibold text-txt">{t("configHeading")}</h3>\n'
        '              <p className="mt-0.5 text-[12px] text-txt3">{t("configBody")}</p>\n'
        '            </div>\n'
        '            <ControlRow label={t("domainSni")} icon={ServerIcon}>')
save(P, t)
print("home preset cards ok")

# ================= Hero.tsx ==================================================
P = ROOT + r"\components\Hero.tsx"
t = load(P)

# 1. geo: cached first, one live lookup, refreshed whenever the tunnel flips
start = t.index("  // Footer facts: the exit address while connected, plus the server's")
end = t.index('  const ipText = exit?.ip || "-"\n') + len('  const ipText = exit?.ip || "-"\n')
new_geo = '''  // Footer facts: where this machine appears from, read off its public
  // address. The lookup races its providers on the Rust side, and the last
  // answer is cached so a launch never starts on a dash.
  const [geo, setGeo] = useState<{ ip: string; place: string; isp: string } | null>(() => {
    try {
      const raw = localStorage.getItem("qc-geo")
      return raw ? (JSON.parse(raw) as { ip: string; place: string; isp: string }) : null
    } catch {
      return null
    }
  })
  useEffect(() => {
    let alive = true
    let timer: number | undefined
    const grab = (n: number) => {
      void netInfo()
        .then((v) => {
          if (!alive) return
          if (v && v.ip) {
            setGeo(v)
            try {
              localStorage.setItem("qc-geo", JSON.stringify(v))
            } catch {
              /* private mode */
            }
            return
          }
          if (n < 2) timer = window.setTimeout(() => grab(n + 1), 2500)
        })
        .catch(() => {
          if (alive && n < 2) timer = window.setTimeout(() => grab(n + 1), 2500)
        })
    }
    grab(0)
    return () => {
      alive = false
      if (timer) window.clearTimeout(timer)
    }
  }, [connected])
  const placeShown = geo?.place ? flagFor(geo.place) + geo.place : "-"
  const ipText = geo?.ip || "-"
'''
t = t[:start] + new_geo + t[end:]

# 2. the dial is pressable while it is connecting, so a tap can call it off
t = sub(t, '<Dial state={state} onClick={toggle} disabled={busy} />',
        '<Dial state={state} onClick={toggle} disabled={busy && phase !== "connecting"} />')

# 3. the old beat: dial and reading sit a step further apart
t = sub(t, 'className={cn("flex w-full items-center", dialLeft ? "justify-start gap-6" : "justify-center")}',
        'className={cn("flex w-full items-center", dialLeft ? "justify-start gap-8" : "justify-center")}')

# 4. status text stays selectable (it is the one thing worth pasting)
t = sub(t, '<div className="mt-2 max-w-[520px] text-[11.5px] leading-snug text-txt3">{status}</div>',
        '<div className="mt-2 max-w-[520px] select-text text-[11.5px] leading-snug text-txt3">{status}</div>')

# 5. facts: three even columns on dividers, and the IP stays selectable
start = t.index("      {/* the connection facts: a calm strip along the card's floor */}")
end = t.index("    </section>")
new_facts = '''      {/* the connection facts: three even columns on dividers */}
      <div className="mt-4 grid grid-cols-3 border-t border-line pt-4">
        <Fact icon={Globe} label={t("yourLocation")} value={placeShown} />
        <Fact
          icon={Network}
          label={t("yourIp")}
          value={ipText}
          className="border-s border-line ps-4"
          valueClass="select-text"
        />
        <Fact
          icon={Activity}
          label={t("statusLbl")}
          value={connected ? t("connected") : t("notConnected")}
          dot={connected}
          className="border-s border-line ps-4"
        />
      </div>
'''
t = t[:start] + new_facts + t[end:]

# 6. Fact takes layout extras
t = sub(t, '''function Fact({
  icon: Icon,
  label,
  value,
  dot,
}: {
  icon: LucideIcon
  label: string
  value: string
  dot?: boolean
}) {
  return (
    <div className="flex min-w-0 items-center gap-2.5">''',
        '''function Fact({
  icon: Icon,
  label,
  value,
  dot,
  className,
  valueClass,
}: {
  icon: LucideIcon
  label: string
  value: string
  dot?: boolean
  className?: string
  valueClass?: string
}) {
  return (
    <div className={cn("flex min-w-0 items-center gap-2.5", className)}>''')
t = sub(t, '          <span className="truncate" dir="auto">\n            {value}',
        '          <span className={cn("truncate", valueClass)} dir="auto">\n            {value}')
save(P, t)
print("hero facts + geo ok")
