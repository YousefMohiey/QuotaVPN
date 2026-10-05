import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p):
    return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))

def sub(t, old, new, cnt=1):
    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:100])
    return t.replace(old, new, cnt)

# ============================================================ Dial =========
P = SRC + r"\components\Dial.tsx"
t = load(P)
# The blue arc joins the ring: the mockup's left-half progress arc. The
# press animation (tap scale, spinner, colour transitions) stays as-is.
t = sub(t, '''      {state === "connecting" && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite]"
        />
      )}
      <span className="flex flex-col items-center gap-2.5">
        <Power className="size-[30px]" strokeWidth={1.6} aria-hidden />
        <span className="text-[12.5px] font-medium tracking-[0.01em]">{label}</span>
      </span>''',
'''      <svg aria-hidden viewBox="0 0 100 100" className="pointer-events-none absolute -inset-[5px] size-[calc(100%+10px)]">
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
      {state === "connecting" && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite]"
        />
      )}
      <span className="flex flex-col items-center gap-2.5">
        <Power className="size-[34px]" strokeWidth={1.6} aria-hidden />
      </span>''')
save(P, t)
print("dial ok")

# ============================================================ Segmented ====
P = SRC + r"\components\Segmented.tsx"
t = load(P)
t = sub(t, '''    <div className={cn("inline-flex rounded-[10px] border border-line bg-white/[0.02] p-0.5", className)}>''',
'''    <div className={cn("inline-flex rounded-[13px] border border-line bg-white/[0.02] p-[3px]", className)}>''')
t = sub(t, '''            className={cn(
              "relative rounded-[8px] px-3 py-1.5 text-[12.5px] transition-colors",
              on ? "text-txt" : "text-txt3 hover:text-txt2",
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className="absolute inset-0 rounded-[8px] bg-white/[0.1]"
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}''',
'''            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",
              on ? "text-white" : "text-txt3 hover:text-txt2",
            )}
          >
            {on && (
              <motion.span
                layoutId={"seg-" + id}
                className="seg-on absolute inset-0 rounded-[10px]"
                transition={{ type: "spring", stiffness: 520, damping: 40 }}
              />
            )}''')
# Hairline dividers between the options, covered by the selected pill.
t = sub(t, '''        return (
          <button
            key={o.value}
            type="button"
            onClick={() => onChange(o.value)}
            aria-pressed={on}''',
'''        const prevOn = options[options.indexOf(o) - 1]?.value === value
        return (
          <button
            key={o.value}
            type="button"
            onClick={() => onChange(o.value)}
            aria-pressed={on}''')
t = sub(t, '''            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",
              on ? "text-white" : "text-txt3 hover:text-txt2",
            )}''',
'''            className={cn(
              "relative rounded-[10px] px-3 py-1.5 text-[12.5px] transition-colors",
              "not-first:border-l not-first:border-line not-first:rounded-l-none",
              on || prevOn ? "border-l-transparent" : "",
              on ? "text-white" : "text-txt3 hover:text-txt2",
            )}''')
save(P, t)
print("segmented ok")

# ============================================================ index.css ====
P = SRC + r"\index.css"
t = load(P)
t = sub(t, "--brand: #1f59b6;",
'''--brand: #1f59b6;''')
t = t.rstrip() + '''

/* The dial's state arc: a left-half progress ring, the mockup's blue
   stroke. Colour alone tells the state, so it follows the same three
   tokens the ring and the label use. */
.dial-arc {
  fill: none;
  stroke: var(--brand-vivid);
  stroke-width: 2.6;
  stroke-linecap: round;
  stroke-dasharray: 148 148;
  transform: rotate(-90deg) scale(-1, 1);
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
  background: linear-gradient(180deg, var(--brand-vivid), var(--brand));
  box-shadow:
    0 0 0 1px rgb(91 141 239 / 0.45),
    0 4px 14px rgb(31 89 182 / 0.35);
}
'''
save(P, t)
print("css ok")
