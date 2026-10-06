import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"
PHONE = r"C:\Tools\QuotaCards\android\tauri-app\ui"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# ============================================================ Dial =========
P = SRC + r"\components\Dial.tsx"
t = load(P)
# The blue arc goes, and so does every shadow that read as glow. What is
# left is a hint of glass: a barely-there translucent fill, one lit line
# along the top, and a quiet ring.
t = sub(t, '''      <svg aria-hidden viewBox="0 0 100 100" className="pointer-events-none absolute -inset-[5px] size-[calc(100%+10px)]">
        <circle
          cx="50"
          cy="50"
          r="47"
          className={cn(
            "dial-arc",
            state === "on" ? "dial-arc-on" : state === "connecting" ? "dial-arc-connecting" : "",
          )}
        />
      </svg>
''', '')
t = sub(t, '''      className={cn(
        "relative grid size-[172px] place-items-center rounded-full border transition-[background-color,border-color,box-shadow] duration-300 disabled:opacity-70",
        /* the glass: a light translucent pane with a soft sheen at the
           top, a lit edge, inner thickness and a real float underneath */
        "bg-[rgb(255_255_255/0.07)] backdrop-blur-[16px] backdrop-saturate-150",
        "bg-[radial-gradient(120%_120%_at_50%_0%,rgb(255_255_255/0.13),transparent_55%)]",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.25),inset_0_-22px_44px_rgb(0_0_0/0.28),0_22px_50px_rgb(0_0_0/0.45)]",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.16)] text-txt hover:border-[var(--brand-line)] hover:bg-[rgb(255_255_255/0.08)]",
      )}''',
'''      className={cn(
        "relative grid size-[172px] place-items-center rounded-full border transition-colors duration-300 disabled:opacity-70",
        /* a hint of glass: translucent fill, one lit line, nothing else */
        "bg-white/[0.05] backdrop-blur-[14px] backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.10)]",
        state === "on"
          ? "border-[var(--green-line)] text-[var(--green)]"
          : state === "connecting"
            ? "border-[var(--brand-line)] text-brand-strong"
            : "border-[rgb(255_255_255/0.14)] text-txt hover:border-[rgb(255_255_255/0.26)]",
      )}''')
save(P, t); print("dial ok")

# ============================================================ index.css ====
P = SRC + r"\index.css"
t = load(P)
t = sub(t, '''/* The dial's state arc: a left-half progress ring, the mockup's blue
   stroke. Colour alone tells the state, so it follows the same three
   tokens the ring and the label use. */
.dial-arc {
  fill: none;
  stroke: var(--brand-vivid);
  stroke-width: 3;
  stroke-linecap: round;
  stroke-dasharray: 148 148;
  /* rotate(90) puts the swept half on the LEFT, like the mockup: the
     dash starts at 3 o'clock, so a clockwise quarter lands it 12-to-6. */
  transform: rotate(90deg);
  transform-origin: center;
  transition: stroke 0.3s ease;
}
.dial-arc-on {
  stroke: var(--green);
}
.dial-arc-connecting {
  stroke: var(--brand-strong);
  animation: pulse 1.1s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.45; }
}

/* The segmented control's selected pill: the mockup's blue gradient,
   bright edge and soft lift. */
.seg-on {
  background: linear-gradient(180deg, #3d84ff, #1747a0);
  box-shadow:
    0 0 0 1px rgb(91 141 239 / 0.45),
    0 4px 14px rgb(31 89 182 / 0.35);
}
''',
'''/* The connecting dot breathes; nothing else on the home moves by itself. */
.pulse-dot {
  animation: pulse 1.1s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.45; }
}
''')
save(P, t); print("css ok")

# ============================================================ Segmented ====
P = SRC + r"\components\Segmented.tsx"
t = load(P)
t = sub(t, '''    <div className={cn("rounded-[13px] border border-line bg-white/[0.02] p-[3px]", fill ? "flex w-full" : "inline-flex", className)}>''',
'''    <div className={cn("rounded-[10px] border border-line bg-white/[0.02] p-0.5", fill ? "flex w-full" : "inline-flex", className)}>''')
t = sub(t, '''        const prevOn = options[options.indexOf(o) - 1]?.value === value
        return (''', '''        return (''')
t = sub(t, '''            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",
              fill && "flex-1",
              "not-first:border-l not-first:border-line not-first:rounded-l-none",
              on || prevOn ? "border-l-transparent" : "",
              on ? "text-white" : "text-txt3 hover:text-txt2",
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className="seg-on absolute inset-0 rounded-[10px]"
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}''',
'''            className={cn(
              "relative rounded-[8px] px-3 py-1.5 text-[12.5px] transition-colors",
              fill && "flex-1",
              on ? "text-txt" : "text-txt3 hover:text-txt2",
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className="absolute inset-0 rounded-[8px] bg-white/[0.1]"
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}''')
save(P, t); print("segmented ok")

# ============================================================ Home =========
P = SRC + r"\screens\Home.tsx"
t = load(P)
t = sub(t, '''                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)] shadow-[0_0_0_1px_var(--brand-line),0_8px_22px_rgb(31_89_182/0.28)]"
                        : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",''',
'''                      isActive
                        ? "border-[var(--brand-line)] bg-[var(--brand-bg)]"
                        : "border-line bg-white/[0.02] hover:border-[var(--brand-line)]",''')
save(P, t); print("home ok")

# ============================================================ phone seg ====
P = PHONE + r"\style.css"
t = load(P)
t = sub(t, '''.seg {
  display: flex;
  gap: 0;
  margin: 2px 0 2px;
  padding: 3px;
  background: rgb(255 255 255 / 0.02);
  border: 1px solid var(--line);
  border-radius: 13px;
}
.seg button { flex: 1; font-weight: 500; border: none; background: transparent; color: var(--txt3); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; border-radius: 10px; }
.seg button + button { border-left: 1px solid var(--line); border-top-left-radius: 0; border-bottom-left-radius: 0; }
.seg button.on + button { border-left-color: transparent; }
.seg button.on {
  background: linear-gradient(180deg, var(--accent-hi), var(--accent));
  color: #fff;
  border-left-color: transparent;
  box-shadow: 0 0 0 1px rgb(91 141 239 / 0.45), 0 4px 14px rgb(31 89 182 / 0.35);
}''',
'''.seg {
  display: flex;
  gap: 3px;
  margin: 2px 0 2px;
  padding: 3px;
  background: rgb(255 255 255 / 0.02);
  border: 1px solid var(--line);
  border-radius: var(--r-ctl);
}
.seg button { flex: 1; font-weight: 500; border: none; background: transparent; color: var(--txt3); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.seg button.on {
  background: rgb(255 255 255 / 0.10);
  color: var(--txt);
  box-shadow: none;
}''')
save(P, t); print("phone seg ok")
