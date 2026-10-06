import io

P = r"C:\Tools\QuotaCards\desktop\ui-next\src\screens\Home.tsx"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

old = '''      <div className="flex w-[138px] shrink-0 items-center gap-2 pt-[2px] text-[12px] text-txt2">
        {Icon && (
          <span className="grid size-[26px] shrink-0 place-items-center rounded-[8px] border border-line bg-white/[0.03] text-txt2">
            <Icon className="size-[13px]" aria-hidden />
          </span>
        )}
        <span className="min-w-0 truncate">{label}</span>
      </div>'''
new = '''      <div className="flex w-[138px] shrink-0 items-center gap-2 pt-[2px] text-[12px] text-txt2">
        {/* the icon slot is reserved even without an icon so every label
            starts on the same x, rows included */}
        <span
          className={cn(
            "grid size-[26px] shrink-0 place-items-center rounded-[8px]",
            Icon && "border border-line bg-white/[0.03] text-txt2",
          )}
        >
          {Icon ? <Icon className="size-[13px]" aria-hidden /> : null}
        </span>
        <span className="min-w-0 truncate">{label}</span>
      </div>'''
assert t.count(old) == 1, "count %d" % t.count(old)
t = t.replace(old, new, 1)

nl = "\r\n" if b"\r\n" in io.open(P, "rb").read()[:2000] else "\n"
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
print("label slot ok")
