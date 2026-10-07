import { useEffect, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { isTauri } from "@/lib/ipc"
import { AnimatePresence, motion } from "motion/react"
import omenUrl from "./assets/omen.png"
import { Sidebar, type Tab } from "@/components/Sidebar"
import { WindowControls } from "@/components/WindowControls"
import { QuotaToast } from "@/components/QuotaToast"
import { Home } from "@/screens/Home"
import { Speed } from "@/screens/Speed"
import { Voice } from "@/screens/Voice"
import { SpeedHistory } from "@/screens/SpeedHistory"
import { SpeedResult } from "@/screens/SpeedResult"
import { Settings } from "@/screens/Settings"
import { Apps } from "@/screens/Apps"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const
const TABS: Tab[] = ["home", "speed", "voice", "history", "result", "apps", "settings"]

export default function App() {
  // The hash is the router: #speed, #settings, #apps. Small, but it
  // makes each screen linkable and lets the preview harness open one cold.
  const [tab, setTab] = useState<Tab>(() => {
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
  }

  const openResult = (at: number) => {
    setResultAt(at)
    go("result")
  }

  // Drag from anywhere: the window moves from any non-interactive surface,
  // so no title strip or edge grab is needed. Buttons, fields, dialogs and
  // scrollbar gutters keep their own gestures.
  useEffect(() => {
    if (!isTauri()) return
    // The window is undecorated, so the caption strip keeps the title-bar
    // gestures: a double press in the top strip toggles maximize. The click
    // count rides on mousedown as e.detail, the signal Tauri's own drag
    // regions use (a dblclick event never arrives once the OS takes the
    // pointer for the move loop).
    const onDown = (e: MouseEvent) => {
      if (e.button !== 0) return
      const el = e.target as HTMLElement | null
      if (!el) return
      if (el.closest('button, a, input, textarea, select, [role="dialog"], [data-no-drag], [contenteditable="true"]')) return
      // A press on a scrollbar must scroll, not move the window.
      const r = el.getBoundingClientRect()
      if (el.scrollHeight > el.clientHeight && e.clientX >= r.left + el.clientWidth - 18) return
      if (el.scrollWidth > el.clientWidth && e.clientY >= r.top + el.clientHeight - 18) return
      if (e.detail === 2 && e.clientY < 64) {
        void getCurrentWindow().toggleMaximize().catch(() => {})
        return
      }
      void getCurrentWindow().startDragging().catch(() => {})
    }
    window.addEventListener("mousedown", onDown)
    return () => window.removeEventListener("mousedown", onDown)
  }, [])

  // The first entry carries the hash too, so a back from anywhere lands on a
  // named screen instead of a blank one.
  useEffect(() => {
    try {
      history.replaceState(null, "", "#" + tabRef.current)
    } catch {
      /* file:// */
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Back/forward, the mouse's own back and forward buttons, and the number
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
      setTab(TABS.includes(h) ? h : "home")
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const el = e.target instanceof HTMLElement ? e.target : null
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
  }, [])

  return (
    <>
      {/* the night scene the glass refracts: fixed, inert, behind everything */}
      <div className="scene" aria-hidden>
        <span className="orb orb-a" />
        <span className="orb orb-b" />
        <span className="orb orb-c" />
        <span className="orb orb-d" />
      </div>
      <WindowControls />
      <div className="relative z-10 flex h-full">
        <Sidebar tab={tab} onTab={go} />
        <main id="content" className="min-w-0 flex-1 overflow-y-auto scroll-pb-6">
          {/* Omen rides here, outside the page-slide animation, as ONE image
              with its own real transparency: no cover cropping, no mask
              ramp, no filter, no overlay. Nothing left that can band, seam,
              or edge. Fully transparent anywhere else. */}
          <img
            aria-hidden
            src={omenUrl}
            alt=""
            draggable={false}
            className={`pointer-events-none fixed top-[48%] right-[-40px] z-0 h-[132%] w-auto -translate-y-1/2 select-none object-contain object-right transition-opacity duration-300 [mask-image:linear-gradient(to_left,black_0%,black_16%,rgba(0,0,0,0.55)_34%,rgba(0,0,0,0.12)_52%,transparent_70%)] [-webkit-mask-image:linear-gradient(to_left,black_0%,black_16%,rgba(0,0,0,0.55)_34%,rgba(0,0,0,0.12)_52%,transparent_70%)] ${
              tab === "voice" ? "opacity-80" : "opacity-0"
            }`}
          />
          {/* the caption strip: pt-14 below keeps the window buttons in clear
              space, and this soft fade hides content that scrolls up behind
              them, so they are never in the way of a page */}
          <div
            aria-hidden
            className="pointer-events-none sticky top-0 z-20 -mb-14 h-14 bg-gradient-to-b from-[#141a25] via-[#141a25]/85 to-transparent"
          />
          <div className="relative mx-auto h-full w-full max-w-[900px] px-8 pb-6 pt-14">
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.div
                key={tab}
                className="h-full"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6, transition: { duration: 0.1 } }}
                transition={{ duration: 0.2, ease: EASE_OUT }}
              >
                {tab === "home" && <Home onOpenApps={() => go("apps")} />}
                {tab === "speed" && <Speed onOpenHistory={() => go("history")} />}
                {tab === "voice" && <Voice />}
                {tab === "history" && <SpeedHistory onOpenResult={openResult} />}
                {tab === "result" && <SpeedResult runAt={resultAt} onBack={() => go("history")} />}
                {tab === "settings" && <Settings />}
                {tab === "apps" && <Apps onBack={() => go("home")} />}
              </motion.div>
            </AnimatePresence>
          </div>
        </main>
        <QuotaToast />
      </div>
    </>
  )
}
