import io

P = r"C:\Tools\QuotaCards\desktop\ui-next\src\screens\Apps.tsx"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# The list stops being one card with dividers: every row is its own card,
# rounded like the speed history rows, gaps between.
t = sub(t, '''      <div className="overflow-hidden rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
        <div
          role="listbox"''',
'''      <div>
        <div
          className="flex flex-col gap-2"
          role="listbox"''')

t = sub(t, '''                    "flex w-full scroll-mt-36 items-center gap-3 border-b border-line px-4 py-2.5 text-start transition-colors last:border-b-0 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-[var(--brand-line)]",
                    appsMode === "all" && "opacity-70",
                    on
                      ? "bg-[var(--brand-bg)]"
                      : focused
                        ? "bg-white/[0.04]"
                        : "hover:bg-white/[0.02]",''',
'''                    "flex w-full scroll-mt-36 items-center gap-3 rounded-[12px] border border-line bg-[rgb(21_29_46/0.62)] px-4 py-2.5 text-start transition-colors hover:border-line-strong focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-[var(--brand-line)]",
                    appsMode === "all" && "opacity-70",
                    on ? "border-[var(--brand-line)] bg-[var(--brand-bg)]" : focused && "bg-white/[0.04]",''')

# Same checkbox as the history: a small rounded square, blue when picked.
t = sub(t, '''                      "grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors",''',
'''                      "grid size-4 shrink-0 place-items-center rounded-[5px] border transition-colors",''')

nl = "\r\n" if b"\r\n" in io.open(P, "rb").read()[:2000] else "\n"
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
print("apps cards ok")
