import io, json

BASE = r"C:\Tools\QuotaCards"
UI = BASE + r"\desktop\ui-next\src"

def load(p): return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:2000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))
def sub(t, old, new):
    assert t.count(old) == 1, "COUNT %d FOR %r" % (t.count(old), old[:90])
    return t.replace(old, new, 1)

# ---- 1. the hidden window keeps its JS alive -------------------------------
P = BASE + r"\desktop\src-tauri\tauri.conf.json"
raw = io.open(P, encoding="utf-8", newline="").read()
cfg = json.loads(raw)
win = cfg["app"]["windows"][0]
win["additionalBrowserArgs"] = (
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion"
    " --disable-background-timer-throttling --disable-renderer-backgrounding"
    " --disable-backgrounding-occluded-windows"
)
# keep the file's own formatting: re-emit with the same indent and newline
nl = "\r\n" if "\r\n" in raw else "\n"
out = json.dumps(cfg, indent=2, ensure_ascii=False)
if not out.endswith("\n"):
    out += "\n"
io.open(P, "w", encoding="utf-8", newline="").write(out.replace("\n", nl) if nl == "\r\n" else out)
print("browser args set")

# ---- 2. a failed arm retries; any look at the window ticks ----------------
P = UI + r"\state\app.tsx"
t = load(P)
t = sub(t, '''    if (game && !gameWasUp.current) {
      // The game just appeared: arm the helper unless it is already on.
      gameWasUp.current = true
      if (!cardUuidRef.current || on || voiceBusyRef.current) return
      const armed = await voiceHelperRef.current(true)
      if (!armed) return
      try {''',
'''    if (game && !gameWasUp.current) {
      // The game just appeared: arm the helper unless it is already on.
      // A failed arm leaves the latch open so the next tick retries: a cold
      // engine or a flaky first packet must not cost the whole session.
      if (on) {
        gameWasUp.current = true
        return
      }
      if (!cardUuidRef.current || voiceBusyRef.current) return
      const armed = await voiceHelperRef.current(true)
      if (!armed) return
      gameWasUp.current = true
      try {''')
t = sub(t, '''    return () => {
      dead = true
      un?.()
      window.clearInterval(id)
    }
  }, [])''',
'''    // The window spends most of its life hidden, so any moment the user
    // actually looks at it is a free chance to catch up on the game state.
    const bump = () => void gameTickRef.current()
    document.addEventListener("visibilitychange", bump)
    window.addEventListener("focus", bump)
    return () => {
      dead = true
      un?.()
      window.clearInterval(id)
      document.removeEventListener("visibilitychange", bump)
      window.removeEventListener("focus", bump)
    }
  }, [])''')
save(P, t); print("retry + visibility ok")
