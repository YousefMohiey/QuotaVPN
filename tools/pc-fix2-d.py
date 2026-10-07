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

# ============ Speed.tsx: the Start button wears the Home dial's frame ========
P = ROOT + r"\screens\Speed.tsx"
t = load(P)
t = sub(t, '''  const [pulse, setPulse] = useState(0)
  const [flash, setFlash] = useState(0)

  const press = () => {
    setPulse((p) => p + 1)
    setFlash((f) => f + 1)
    if (running) onStop()
    else onStart()
  }''',
        '''  const [flash, setFlash] = useState(0)

  const press = () => {
    setFlash((f) => f + 1)
    if (running) onStop()
    else onStart()
  }''')
t = sub(t, '''    <motion.button
      type="button"
      onClick={press}
      aria-label={label}
      whileTap={{ scale: 0.97 }}
      transition={{ type: "spring", stiffness: 520, damping: 30 }}
      className="group relative grid size-[208px] place-items-center rounded-full"
    >
      {/* the disc: same glass as the cards, no outline */}
      <span
        aria-hidden
        className={cn(
          "absolute inset-[16px] rounded-full bg-[rgb(255_255_255/0.05)] shadow-[inset_0_1px_0_0_rgb(255_255_255/0.09)] backdrop-blur-xl transition-colors duration-200",
          "group-hover:bg-[rgb(255_255_255/0.075)]",
          running && "bg-[rgb(255_255_255/0.075)]",
        )}
      />
      {/* press feedback: a soft flash inside the disc, a pulse outside it */}
      {flash > 0 && (
        <span
          key={flash}
          aria-hidden
          className="qc-flash pointer-events-none absolute inset-[16px] rounded-full bg-[rgb(255_255_255/0.09)] opacity-0"
        />
      )}
      {pulse > 0 && (
        <span
          key={pulse}
          aria-hidden
          className="qc-pulse pointer-events-none absolute inset-[8px] rounded-full border-[1.5px] border-[rgb(46_123_246/0.7)] opacity-0"
        />
      )}''',
        '''    <motion.button
      type="button"
      onClick={press}
      aria-label={label}
      whileTap={{ scale: 0.985 }}
      transition={{ type: "spring", stiffness: 460, damping: 32 }}
      className={cn(
        /* the same frame as the Home dial: translucent fill, one lit line */
        "group relative grid size-[208px] place-items-center rounded-full border transition-colors duration-300",
        "bg-white/[0.05] backdrop-blur-[14px] backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.10)]",
        running
          ? "border-[rgb(255_255_255/0.26)]"
          : "border-[rgb(255_255_255/0.14)] hover:border-[rgb(255_255_255/0.26)]",
      )}
    >
      {/* press feedback: a soft flash inside the glass */}
      {flash > 0 && (
        <span
          key={flash}
          aria-hidden
          className="qc-flash pointer-events-none absolute inset-px rounded-full bg-[rgb(255_255_255/0.09)] opacity-0"
        />
      )}''')
t = sub(t, '''/** The one button that matters here: a frosted glass disc with the action
    inside, no ring, no arc (owner call: the blue line around it is gone).
    A press still answers: the disc flashes, a pulse leaves the edge, and
    the icon and label swap through a small morph. */''',
        '''/** The one button that matters here: the Home dial's frame, a translucent
    glass ring with the action inside. A press still answers: the ring
    flashes softly and the icon and label swap through a small morph. */''')
save(P, t)
print("speed button ok")

# ============ App.tsx: real history, mouse buttons, number row ===============
P = ROOT + r"\App.tsx"
t = load(P)
t = sub(t, 'const EASE_OUT = [0.1, 0.9, 0.2, 1] as const',
        'const EASE_OUT = [0.1, 0.9, 0.2, 1] as const\nconst TABS: Tab[] = ["home", "speed", "voice", "history", "result", "apps", "settings"]')
t = sub(t, '''  const [tab, setTab] = useState<Tab>(() => {
    const h = (typeof location !== "undefined" ? location.hash.slice(1) : "") as Tab
    return (["home", "speed", "voice", "history", "result", "apps", "settings"] as Tab[]).includes(h) ? h : "home"
  })
  // The stored run opened in the result view, if any.
  const [resultAt, setResultAt] = useState<number | null>(null)

  const go = (t: Tab) => {
    setTab(t)
    try {
      history.replaceState(null, "", "#" + t)
    } catch {
      /* file:// */
    }
  }''',
        '''  const [tab, setTab] = useState<Tab>(() => {
    const h = (typeof location !== "undefined" ? location.hash.slice(1) : "") as Tab
    return TABS.includes(h) ? h : "home"
  })
  // The stored run opened in the result view, if any.
  const [resultAt, setResultAt] = useState<number | null>(null)
  const tabRef = useRef(tab)
  tabRef.current = tab
  const lastPop = useRef(0)

  // Every move becomes a real history entry, so the shell's own back and
  // forward (mouse buttons, keyboard) walk the app instead of doing nothing.
  const go = (t: Tab) => {
    if (t === tabRef.current) return
    setTab(t)
    try {
      history.pushState(null, "", "#" + t)
    } catch {
      /* file:// */
    }
  }''')
t = sub(t, 'import { useEffect, useState } from "react"', 'import { useEffect, useRef, useState } from "react"')
t = sub(t, '''  // Back/forward and anything else that moves the hash keeps the shell in step.
  useEffect(() => {
    const onHash = () => {
      const h = location.hash.slice(1) as Tab
      if ((["home", "speed", "voice", "history", "result", "apps", "settings"] as Tab[]).includes(h)) setTab(h)
    }
    window.addEventListener("hashchange", onHash)
    return () => window.removeEventListener("hashchange", onHash)
  }, [])''',
        '''  // Back/forward, the mouse's own back and forward buttons, and the number
  // row all move the shell. The hash is the source of truth, so anything the
  // shell itself navigates lands in the same place.
  useEffect(() => {
    const KEYS: Record<string, Tab> = {
      "1": "home",
      "2": "voice",
      "3": "speed",
      "4": "history",
      "5": "settings",
    }
    const onPop = () => {
      lastPop.current = Date.now()
      const h = location.hash.slice(1) as Tab
      if (TABS.includes(h)) setTab(h)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const el = e.target as HTMLElement | null
      if (el && el.closest('input, textarea, select, [contenteditable="true"], [role="dialog"]')) return
      const hit = KEYS[e.key]
      if (!hit) return
      e.preventDefault()
      go(hit)
    }
    const onAux = (e: MouseEvent) => {
      if (e.button !== 3 && e.button !== 4) return
      e.preventDefault()
      const back = e.button === 3
      const seen = lastPop.current
      // If the shell moved on this press by itself, let it stand; otherwise
      // walk our own entries a beat later.
      window.setTimeout(() => {
        if (lastPop.current !== seen) return
        try {
          if (back) history.back()
          else history.forward()
        } catch {
          /* file:// */
        }
      }, 70)
    }
    window.addEventListener("popstate", onPop)
    window.addEventListener("keydown", onKey)
    window.addEventListener("mousedown", onAux)
    return () => {
      window.removeEventListener("popstate", onPop)
      window.removeEventListener("keydown", onKey)
      window.removeEventListener("mousedown", onAux)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])''')
save(P, t)
print("app nav ok")

# ============ state/app.tsx: cancel a connect in flight ======================
P = ROOT + r"\state\app.tsx"
t = load(P)
t = sub(t, '''  const busyRef = useRef(false)
  const vpnRef = useRef(false)''',
        '''  const busyRef = useRef(false)
  const vpnRef = useRef(false)
  const phaseRef = useRef<Phase>("idle")
  // Every connect run carries a generation; cancelling bumps it so the run
  // that is still in flight stops applying state the moment it returns.
  const connectGenRef = useRef(0)''')
t = sub(t, '''  busyRef.current = busy
  vpnRef.current = vpnOn''',
        '''  busyRef.current = busy
  vpnRef.current = vpnOn
  phaseRef.current = phase''')
t = sub(t, '''  const connect = useCallback(async () => {
    teardownRef.current += 1''',
        '''  const connect = useCallback(async () => {
    teardownRef.current += 1
    const gen = ++connectGenRef.current
    const stale = () => connectGenRef.current !== gen''')
t = sub(t, '''          const g = await api.generateCard(preset, preset, DEFAULT_SNI[preset])
          if (!g.ok) {''',
        '''          const g = await api.generateCard(preset, preset, DEFAULT_SNI[preset])
          if (stale()) return
          if (!g.ok) {''')
t = sub(t, '''      await api.probeServer()''',
        '''      await api.probeServer()
      if (stale()) return''')
t = sub(t, '''      const r = await api.start(uuid, mode, apps, transport, voiceOn)
      if (!r.ok) {''',
        '''      const r = await api.start(uuid, mode, apps, transport, voiceOn)
      if (stale()) {
        // The run was called off while the engine was starting: make sure
        // whatever it just brought up comes back down.
        void api.stop().catch(() => {})
        return
      }
      if (!r.ok) {''')
t = sub(t, '''      for (let i = 0; i < 8; i++) {
        const st = await api.status()
        if (st.running) break''',
        '''      for (let i = 0; i < 8; i++) {
        if (stale()) return
        const st = await api.status()
        if (st.running) break''')
t = sub(t, '''      const probe = await api.probeTunnel().catch(() => null)
      setConnected(!!probe?.ok)
      setPhase("on")''',
        '''      const probe = await api.probeTunnel().catch(() => null)
      if (stale()) return
      setConnected(!!probe?.ok)
      setPhase("on")''')
t = sub(t, '''    } catch (e) {
      setStatus(String(e))
      setPhase("idle")
    } finally {
      setBusy(false)
    }
  }, [apps, appsMode, cards, cardUuid, preset, t, transport])''',
        '''    } catch (e) {
      if (!stale()) {
        setStatus(String(e))
        setPhase("idle")
      }
    } finally {
      // A cancelled run leaves busy alone: the cancel path owns it now.
      if (!stale()) setBusy(false)
    }
  }, [apps, appsMode, cards, cardUuid, preset, t, transport])''')
t = sub(t, '''  const toggle = useCallback(() => {
    if (busyRef.current) return
    void (vpnRef.current ? disconnect() : connect())
  }, [connect, disconnect])''',
        '''  // The dial stays live while it is connecting, so a press in that window
  // calls the whole attempt off instead of being ignored.
  const cancelConnect = useCallback(async () => {
    connectGenRef.current += 1
    await disconnect()
  }, [disconnect])

  const toggle = useCallback(() => {
    if (phaseRef.current === "connecting") {
      void cancelConnect()
      return
    }
    if (busyRef.current) return
    void (vpnRef.current ? disconnect() : connect())
  }, [cancelConnect, connect, disconnect])''')
save(P, t)
print("cancel connect ok")
