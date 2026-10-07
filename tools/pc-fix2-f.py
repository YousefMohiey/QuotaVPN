import io

P = r"C:\Tools\QuotaCards\desktop\ui-next\src\screens\Home.tsx"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(t, old, new, n=1):
    assert t.count(old) == n, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, n)

# The mockup gives the presets the wider half; the controls column is a
# little narrower than it was so the preset cards can hold their chips on
# one line.
t = sub(t, 'className="flex shrink-0 flex-col justify-center gap-3 lg:w-[436px] lg:border-l lg:border-line lg:pl-6"',
        'className="flex shrink-0 flex-col justify-center gap-3 lg:w-[364px] lg:border-l lg:border-line lg:pl-6"')
t = sub(t, '<span key={tag} className="rounded-full border border-line px-2 py-[3px] text-[10.5px] text-txt3">',
        '<span key={tag} className="rounded-full border border-line px-1.5 py-[2px] text-[10px] text-txt3">')
t = sub(t, '<span className="mt-auto flex flex-wrap gap-1.5 pt-3">',
        '<span className="mt-auto flex flex-wrap gap-1 pt-3">')

nl = "\r\n" if b"\r\n" in io.open(P, "rb").read()[:4000] else "\n"
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
print("home widths ok")
