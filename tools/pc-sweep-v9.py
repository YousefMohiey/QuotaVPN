import io

SRC = r"C:\Tools\QuotaCards\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# Settings: the cards were stranded at the top of a tall page.
P = SRC + r"\screens\Settings.tsx"
t = load(P)
t = sub(t, '    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-3">',
        '    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-3">')
t = sub(t, '''      </div>

      <SettingCard
        icon={<Languages className="size-5" strokeWidth={1.7} aria-hidden />}''',
'''      </div>

      {/* the cards sit centered in the page instead of stranded at the top */}
      <div className="flex min-h-0 flex-1 flex-col justify-center gap-3">
      <SettingCard
        icon={<Languages className="size-5" strokeWidth={1.7} aria-hidden />}''')
t = sub(t, '''      />
    </div>
  )
}''',
'''      />
      </div>
    </div>
  )
}''')
save(P, t); print("settings ok")

# History: the empty state now fills the page and centers its message.
P = SRC + r"\screens\SpeedHistory.tsx"
t = load(P)
t = sub(t, '    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-4">',
        '    <div className="mx-auto flex h-full min-h-0 w-full max-w-[1040px] flex-col gap-4">')
t = sub(t, '        <section className="flex flex-col items-center justify-center rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] px-5 py-12 text-center">',
        '        <section className="flex flex-1 flex-col items-center justify-center rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)] px-5 py-12 text-center">')
save(P, t); print("history ok")
