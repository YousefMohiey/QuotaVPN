import { useEffect, useRef, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { Activity, ArrowDown, ArrowUp, ChevronDown, ChevronRight, Cloud, Gauge, Gamepad2, Globe, History, Play, RefreshCw, Server, Square, Tv, Video, Wifi } from "lucide-react"
import { PickerDialog } from "@/components/PickerDialog"
import { SpeedGraph } from "@/components/SpeedGraph"
import { useApp } from "@/state/app"
import { useI18n, type StrKey } from "@/lib/i18n"
import { measureDownload, measurePing, measureUpload } from "@/lib/speedtest"
import { CLOUDFLARE, loadPool, pickFastest, type SpeedServer } from "@/lib/speedservers"
import { isTauri, netInfo, onSpeedPhase, onSpeedResult, onSpeedTick, speedDown, speedLatency, speedUp, speedtestCli, speedtestCliReady, type CliSpeed } from "@/lib/ipc"
import { cn } from "@/lib/utils"

const PING_PROBES = 8
const PHASE_SECONDS = 9
const BARS_MEMORY = 96
const EASE_OUT = [0.1, 0.9, 0.2, 1] as const
const SPRING = { type: "spring", stiffness: 320, damping: 34 } as const

/** Mean absolute delta between consecutive samples: the same jitter reading
 *  the browser-side measurer reports, for the backend path. */
const meanAbsDelta = (xs: number[]): number =>
  xs.length < 2 ? 0 : xs.slice(1).reduce((a, v, i) => a + Math.abs(v - xs[i]), 0) / (xs.length - 1)

/** "we · Giza" for the server the official client picked. */
const cliLabel = (c: CliSpeed): string =>
  [c.server_name, c.server_location].filter(Boolean).join(" · ").trim()

type Phase = "idle" | "ping" | "download" | "upload" | "done"
type Result = { ping: number | null; jitter: number | null; down: number | null; up: number | null }
export type Run = { at: number; ping: number | null; jitter: number | null; down: number | null; up: number | null; target: string }
/** A history page is only useful if it keeps more than a handful, so the
    store holds a long list (the old inline strip capped it at 3). */
const HISTORY_MAX = 100
type NetInfo = { ip: string; isp: string; place: string }

const EMPTY: Result = { ping: null, jitter: null, down: null, up: null }

export function loadHistory(): Run[] {
  try {
    const raw = localStorage.getItem("qc-speed-history")
    const arr = raw ? (JSON.parse(raw) as Array<Partial<Run>>) : []
    if (!Array.isArray(arr)) return []
    return arr
      .filter((h) => h && typeof h === "object")
      .map((h) => ({
        at: typeof h.at === "number" ? h.at : Date.now(),
        ping: typeof h.ping === "number" ? h.ping : null,
        jitter: typeof h.jitter === "number" ? h.jitter : null,
        down: typeof h.down === "number" ? h.down : null,
        up: typeof h.up === "number" ? h.up : null,
        target: typeof h.target === "string" ? h.target : "",
      }))
      .slice(0, HISTORY_MAX)
  } catch {
    return []
  }
}

/** Where a run measures against. Cloudflare's speed endpoints are the public
    reference (that is the site's own API, CORS-open); the QuotaVPN server
    option measures the card's own path through the tunnel. */
export type TestServer = "cloudflare" | "own"

export type RunUrls = {
  ping: string
  down: (bytes: number) => string
  up: string
}

/** Exit IP plus provider for the speed page. Both services are HTTPS and
    CORS-open; with the tunnel up this reports the server's address, which is
    what a speed test should show. Failure just means no provider line. */
async function resolveNetInfo(signal: AbortSignal): Promise<NetInfo | null> {
  // In the app the backend resolves this first: the webview is at the mercy
  // of whatever the host network or a policy does to these JSON APIs, the
  // Rust client is not. Whatever it returns wins; otherwise fall through to
  // the browser chain so the preview still shows something real.
  if (isTauri()) {
    try {
      const r = await netInfo()
      if (r && r.ip) return r
    } catch {
      /* keep going */
    }
  }
  const timeout = AbortSignal.timeout(8000)
  try {
    const r = await fetch("https://ipwho.is/", { signal: timeout, cache: "no-store" })
    if (r.ok) {
      const d = await r.json()
      if (d && d.success !== false && typeof d.ip === "string" && d.ip) {
        const isp = String(d.connection?.isp || d.connection?.org || "")
        const place = [d.city, d.country].filter(Boolean).join(", ")
        return { ip: d.ip, isp, place }
      }
    }
  } catch {
    /* try the next one */
  }
  try {
    const r = await fetch("https://ipapi.co/json/", { signal: timeout, cache: "no-store" })
    if (r.ok) {
      const d = await r.json()
      if (d && typeof d.ip === "string" && d.ip) {
        return {
          ip: d.ip,
          isp: String(d.org || ""),
          place: [d.city, d.country_name].filter(Boolean).join(", "),
        }
      }
    }
  } catch {
    /* no provider line then */
  }
  if (signal.aborted) return null
  return null
}

export function saveHistory(runs: Run[]) {
  try {
    localStorage.setItem("qc-speed-history", JSON.stringify(runs.slice(0, HISTORY_MAX)))
  } catch {
    /* private mode: history just does not stick */
  }
}

export function Speed({ onOpenHistory }: { onOpenHistory: () => void }) {
  const { t } = useI18n()
  const { card, hardRefresh, refreshing } = useApp()

  const [phase, setPhase] = useState<Phase>("idle")
  const [caption, setCaption] = useState(() => t("idle"))
  const [value, setValue] = useState(0)
  const [unit, setUnit] = useState<"Mbps" | "ms">("Mbps")
  const [samples, setSamples] = useState<number[]>([])
  const [result, setResult] = useState<Result>(EMPTY)
  const [hint, setHint] = useState("")
  const [history, setHistory] = useState<Run[]>(() => loadHistory())
  const [netInfo, setNetInfo] = useState<NetInfo | null>(null)
  const [pool, setPool] = useState<SpeedServer[]>([CLOUDFLARE])
  const [picked, setPicked] = useState<SpeedServer | null>(null)
  // True while the server list is loading and the nearest one is being found,
  // so the panel can say so instead of showing Cloudflare as if it were the
  // answer. Also gates the official client: it is fetched once at arrival,
  // never in the middle of a run.
  const [picking, setPicking] = useState(true)
  const [cliReady, setCliReady] = useState(false)
  const [pickOpen, setPickOpen] = useState(false)
  // Which measurement the graph belongs to, so the line keeps its colour
  // after the run ends (green upload, amber ping, blue download).
  const [lastKind, setLastKind] = useState<"ping" | "down" | "up">("down")

  const abort = useRef<AbortController | null>(null)
  const gate = useRef(0)
  const failNote = useRef("")
  // The samples array mirrored in a ref, so a phase can snapshot its own
  // shape the moment it finalizes. After the run, the graph shows the
  // headline result's shape (download), not whatever phase ran last.
  const samplesRef = useRef<number[]>([])
  const pingSnap = useRef<number[]>([])
  const downSnap = useRef<number[]>([])
  const upSnap = useRef<number[]>([])
  const clearSamples = () => {
    samplesRef.current = []
    setSamples([])
  }

  const running = phase === "ping" || phase === "download" || phase === "upload"

  // The reading panel slides in only after the circle has had room to move,
  // the same beat Home uses between its dial and the info panel. Mounting
  // both at once is what makes the motion feel fast and rough.
  const [showPanel, setShowPanel] = useState(false)
  useEffect(() => {
    if (!running) {
      setShowPanel(false)
      return
    }
    const id = window.setTimeout(() => setShowPanel(true), 250)
    return () => window.clearTimeout(id)
  }, [running])

  // Parked only while measuring: when the run ends the ring goes back to
  // the middle (Start again), so the page always settles the same way.
  const parked = running
  // Which server the run goes to. Loaded on arrival and picked by round trip,
  // like a speed test does, so the reading uses the nearest host instead of a
  // fixed one; Cloudflare stays in the pool as the always-available entry.
  useEffect(() => {
    const ctl = new AbortController()
    void loadPool(ctl.signal)
      .then(async (pool) => {
        if (ctl.signal.aborted) return
        setPool(pool)
        const best = await pickFastest(pool, ctl.signal)
        if (!ctl.signal.aborted) setPicked(best)
      })
      .catch(() => {
        /* the Cloudflare entry is always a valid answer */
      })
      .finally(() => {
        if (!ctl.signal.aborted) setPicking(false)
      })
    return () => ctl.abort()
  }, [])

  // Fetch the official client once, in the background, so pressing Start never
  // waits on a download. If it never arrives, Start runs the built-in
  // measurement instead.
  useEffect(() => {
    let alive = true
    if (!isTauri()) return
    void speedtestCliReady()
      .then((ok) => {
        if (alive) setCliReady(ok)
      })
      .catch(() => {
        if (alive) setCliReady(false)
      })
    return () => {
      alive = false
    }
  }, [])

  // The exit address, resolved on arrival so the facts are on screen before a
  // run, not only after one. A failed lookup gets one quiet retry: the first
  // request after launch can race the network stack coming up.
  useEffect(() => {
    const ctl = new AbortController()
    let retry: number | undefined
    const look = (first: boolean) =>
      void resolveNetInfo(ctl.signal).then((info) => {
        if (ctl.signal.aborted) return
        if (info) setNetInfo(info)
        else if (first) retry = window.setTimeout(() => look(false), 4000)
      })
    look(true)
    return () => {
      ctl.abort()
      if (retry) window.clearTimeout(retry)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [card?.uuid])

  useEffect(() => {
    setResult(EMPTY)
    setValue(0)
    clearSamples()
    setCaption(t("idle"))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [card?.uuid])

  useEffect(() => () => abort.current?.abort(), [])

  const push = (v: number, force = false) => {
    const now = performance.now()
    if (!force && now - gate.current < 90) return
    gate.current = now
    setValue(v)
    const next = [...samplesRef.current.slice(-(BARS_MEMORY - 1)), v]
    samplesRef.current = next
    setSamples(next)
  }

  const runPing = async (srv: SpeedServer, signal: AbortSignal) => {
    setPhase("ping")
    setCaption(t("pingTitle"))
    setUnit("ms")
    setValue(0)
    clearSamples()
    setHint(t("pingHint"))
    try {
      if (isTauri()) {
        const ms = await speedLatency(srv.ping, PING_PROBES)
        if (!ms.length) throw new Error("no probes")
        for (const v of ms) push(v)
        const ping = Math.min(...ms)
        const jitter = meanAbsDelta(ms)
        setResult((prev) => ({ ...prev, ping, jitter }))
        push(ping, true)
        setValue(ping)
        pingSnap.current = samplesRef.current.slice()
        return { ping, jitter }
      }
      const r = await measurePing("net", PING_PROBES, {
        signal,
        pingUrl: srv.ping,
        onPing: (ms) => push(ms),
      })
      setResult((prev) => ({ ...prev, ping: r.ping, jitter: r.jitter }))
      push(r.ping)
      setValue(r.ping)
      pingSnap.current = samplesRef.current.slice()
      return r
    } catch {
      if (!signal.aborted) setHint(t("noReply"))
      return null
    }
  }

  const runThroughput = async (direction: "down" | "up", srv: SpeedServer, signal: AbortSignal) => {
    setPhase(direction === "down" ? "download" : "upload")
    setCaption(direction === "down" ? t("chDown") : t("chUp"))
    setUnit("Mbps")
    setValue(0)
    clearSamples()
    setHint(direction === "down" ? t("downHint") : t("upHint"))
    try {
      let mbps: number | null
      let reason = ""
      if (isTauri()) {
        // Backend measurement: streams are counted after arrival and upload
        // chunks are timed to the server's ack, which is the difference
        // between a real number and a buffer-inflated one. The live-tick
        // listener is best effort: if it cannot be registered (a refused
        // permission, an older runtime), the phase still measures.
        let off: () => void = () => {}
        try {
          off = await onSpeedTick((v) => {
            if (!signal.aborted) push(v)
          })
        } catch {
          off = () => {}
        }
        try {
          const out =
            direction === "down"
              ? await speedDown(srv.downUrls, PHASE_SECONDS)
              : await speedUp(srv.up, PHASE_SECONDS)
          mbps = out?.mbps ?? null
          if (mbps === null && out?.note) reason = out.note
        } finally {
          try {
            off()
          } catch {
            /* nothing to detach */
          }
        }
      } else {
        const opts = { seconds: PHASE_SECONDS, signal, onTick: (v: number) => push(v) }
        mbps =
          direction === "down"
            ? await measureDownload("net", { ...opts, downUrl: () => srv.downUrls[0] })
            : await measureUpload("net", { ...opts, upUrl: srv.up })
      }
      if (mbps === null) throw new Error(reason || "no throughput")
      setResult((prev) => (direction === "down" ? { ...prev, down: mbps } : { ...prev, up: mbps }))
      push(mbps, true)
      setValue(mbps)
      if (direction === "down") downSnap.current = samplesRef.current.slice()
      else upSnap.current = samplesRef.current.slice()
      return mbps
    } catch (e) {
      if (!signal.aborted) {
        // Tauri rejects with a plain string, so surfacing only Error.message
        // hides exactly the failure that needs seeing. Show whatever came.
        const msg =
          typeof e === "string"
            ? e
            : e instanceof Error
              ? e.message
              : ""
        const why = msg && msg !== "no throughput" ? msg : t("noReply")
        failNote.current = why
        setHint(why)
      }
      return null
    }
  }

  const remember = (r: Result, label: string) => {
    if (r.ping === null && r.down === null && r.up === null) return
    const next = [{ at: Date.now(), ...r, target: label }, ...history].slice(0, HISTORY_MAX)
    setHistory(next)
    try {
      localStorage.setItem("qc-speed-history", JSON.stringify(next))
    } catch {
      /* private mode: history just does not stick */
    }
  }

  const run = async (which: "all" | "ping" | "down" | "up") => {
    if (running) return
    abort.current?.abort()
    const ctl = new AbortController()
    abort.current = ctl
    const s = ctl.signal
    const acc: Result = { ...result }
    // What the headline will be: ping runs show ms, upload-only runs show
    // up, everything else lands on download. The finished graph follows it.
    setLastKind(which === "ping" ? "ping" : which === "up" ? "up" : "down")
    downSnap.current = []
    upSnap.current = []
    pingSnap.current = []

    // A run always has a server: if the arrival pick has not landed yet (slow
    // network, first second after launch), pick now rather than defaulting.
    let server = picked
    if (!server) {
      server = await pickFastest(pool, s)
      if (s.aborted) return
      setPicked(server)
    }
    const label = server.label

    setNetInfo(null)
    void resolveNetInfo(s).then((info) => {
      if (!s.aborted) setNetInfo(info)
    })

    // The official speedtest.net client runs the whole test itself: its own
    // server selection (which is how the reading ends up comparable to the
    // website's, provider-hosted servers included) and its own timing. It
    // reports live Mbps as it goes, so the bars animate the same way. Anything
    // about it failing, missing, or throwing falls through to the built-in
    // measurer below; nothing here may leave the run stuck.
    if (isTauri() && which === "all" && cliReady) {
      let cli: CliSpeed | null = null
      try {
        setPhase("ping")
        setCaption(t("pingTitle"))
        setUnit("ms")
        setValue(0)
        clearSamples()
        setHint(t("pingHint"))
        const offPhase = await onSpeedPhase((p) => {
          if (s.aborted) return
          if (p === "select") {
            // The client is picking its server: show that instead of a dead
            // screen for the several seconds it takes.
            setPhase("ping")
            setCaption(t("findingServer"))
            setUnit("Mbps")
            setValue(0)
            clearSamples()
          } else if (p === "download") {
            setPhase("download")
            setCaption(t("chDown"))
            setUnit("Mbps")
            setValue(0)
            clearSamples()
          } else if (p === "upload") {
            setPhase("upload")
            setCaption(t("chUp"))
            setUnit("Mbps")
            setValue(0)
            clearSamples()
          }
        }).catch(() => () => {})
        const offTick = await onSpeedTick((v) => {
          if (!s.aborted) {
            push(v)
            setValue(v)
          }
        }).catch(() => () => {})
        // Values as each phase finalizes: the ping the moment it is measured,
        // the download the moment its phase ends. Nothing waits for the whole
        // run to finish to become visible.
        const offResult = await onSpeedResult((part) => {
          if (!s.aborted) setResult((prev) => ({ ...prev, ...part }))
        }).catch(() => () => {})
        try {
          cli = await speedtestCli()
        } finally {
          offPhase()
          offTick()
          offResult()
        }
      } catch {
        cli = null
      }
      if (s.aborted) return
      if (cli && (cli.down_mbps !== null || cli.up_mbps !== null)) {
        const next: Result = {
          ping: cli.ping_ms ?? null,
          jitter: cli.jitter_ms ?? null,
          down: cli.down_mbps ?? null,
          up: cli.up_mbps ?? null,
        }
        setResult(next)
        remember(next, cliLabel(cli) || label)
        if (cli.server_name) {
          setPicked({
            id: "ookla-cli",
            label: cliLabel(cli),
            detail: [cli.server_country, cli.isp].filter(Boolean).join(" · "),
            host: cli.server_host || "speedtest.net",
            ping: "",
            downUrls: [],
            up: "",
          })
        }
        setPhase("done")
        setValue(next.down ?? next.up ?? 0)
        clearSamples()
        return
      }
      // The client ran but produced nothing usable: fall through to the
      // built-in measurement rather than showing an idle screen.
      setPhase("idle")
      setCaption(t("idle"))
    }

    // Each phase reports back here so the history line reflects the run that
    // just happened, not whatever the tiles happened to hold before.
    //
    // If the picked server turns out to answer nothing (a provider host that
    // blocks or drops traffic from wherever the tunnel exits, for example),
    // the run does not end there: it moves down the pool and finishes on
    // Cloudflare, which answers from anywhere. Only a run that produced a
    // throughput is written to the history.
    const candidates: SpeedServer[] = []
    const addCandidate = (srv?: SpeedServer | null) => {
      if (srv && !candidates.some((c) => c.id === srv.id)) candidates.push(srv)
    }
    addCandidate(server)
    // Cloudflare second: if the provider host is unreachable from wherever the
    // network currently exits (a tunnel in another country, a blocked port),
    // this is the one that still answers, so the run ends with real numbers
    // instead of an empty screen.
    addCandidate(CLOUDFLARE)
    for (const srv of pool.slice(0, 4)) addCandidate(srv)

    let used = server
    for (let attempt = 0; attempt < candidates.length && attempt < 3; attempt++) {
      const srv = candidates[attempt]
      used = srv
      if (attempt > 0) {
        clearSamples()
        setValue(0)
      }
      if (which === "all" || which === "ping") {
        const p = await runPing(srv, s)
        if (p) {
          acc.ping = p.ping
          acc.jitter = p.jitter
        }
        if (s.aborted) return
      }
      if (which === "all" || which === "down") {
        const d = await runThroughput("down", srv, s)
        if (d !== null) acc.down = d
        if (s.aborted) return
      }
      if (which === "all" || which === "up") {
        const u = await runThroughput("up", srv, s)
        if (u !== null) acc.up = u
        if (s.aborted) return
      }
      const gotThroughput = acc.down !== null || acc.up !== null
      if (gotThroughput || which !== "all") break
      // Nothing came back: report it and try the next server in the pool.
      if (attempt === 0) setHint(t("srvRetry"))
    }

    if (used.id !== server.id && (acc.down !== null || acc.up !== null)) {
      setPicked(used)
    }
    const gotThroughput = acc.down !== null || acc.up !== null
    setPhase("done")
    if (gotThroughput || which === "ping") {
      // The big readout shows what was just measured. It must not fall back to
      // the ping value on a failed run: that is how "Up 12.2" happened, with
      // the upload dead and the ping number left standing under an Up caption.
      setValue(which === "ping" ? acc.ping ?? 0 : acc.down ?? acc.up ?? 0)
      remember(acc, used.label || label)
    } else {
      setValue(0)
      setCaption(t("idle"))
      // Leave the reason on screen: it is the only thing that tells the user
      // (and the next debugging pass) which step died.
      setHint(failNote.current || t("noReply"))
    }
  }

  const stop = () => {
    abort.current?.abort()
    setPhase("idle")
    setCaption(t("idle"))
    setHint(t("stopped"))
  }

  // Pull everything this page shows again: the app state (so the card
  // domains are current), the server pool and the exit address.
  const refreshAll = async () => {
    const ctl = new AbortController()
    setPicking(true)
    try {
      await hardRefresh()
      const pool = await loadPool(ctl.signal)
      if (ctl.signal.aborted) return
      setPool(pool)
      const best = await pickFastest(pool, ctl.signal)
      if (!ctl.signal.aborted) setPicked(best)
    } catch {
      /* keep the current list */
    } finally {
      setPicking(false)
    }
    void resolveNetInfo(ctl.signal).then((info) => {
      if (info && !ctl.signal.aborted) setNetInfo(info)
    })
  }

  const verdicts = buildVerdicts(result, t)
  const shown =
    phase === "done"
      ? lastKind === "up"
        ? upSnap.current
        : lastKind === "ping"
          ? pingSnap.current
          : downSnap.current
      : samples
  const peak = Math.max(0, ...shown)
  const srv = picked ?? CLOUDFLARE
  const sponsor = srv.label.includes(" · ") ? srv.label.split(" · ")[0].trim() : srv.label
  // The line keeps the colour of whatever was measured last.
  const kind: "ping" | "down" | "up" = running
    ? phase === "ping"
      ? "ping"
      : phase === "upload"
        ? "up"
        : "down"
    : lastKind
  const accent = kind === "up" ? "var(--green)" : kind === "ping" ? "var(--amber)" : "var(--brand)"

  const chooseServer = async (id: string) => {
    if (id === "auto") {
      setPicking(true)
      try {
        const best = await pickFastest(pool, new AbortController().signal)
        setPicked(best)
      } finally {
        setPicking(false)
      }
      return
    }
    const s = pool.find((x) => x.id === id)
    if (s) setPicked(s)
  }

  const pickerItems = [
    { value: "auto", label: t("srvAuto"), sub: t("srvAutoSub") },
    ...pool.map((s) => ({ value: s.id, label: s.label, sub: s.detail })),
  ]

  return (
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-4">
      <div className="flex items-end justify-between gap-6">
        <div className="min-w-0 flex-1">
          <h1 className="text-[30px] font-semibold leading-tight text-txt">{t("tabSpeed")}</h1>
          <p className="mt-1 text-[15px] text-txt2">{t("speedTag")}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2.5">
          <button
            type="button"
            onClick={() => setPickOpen(true)}
            disabled={running}
            aria-label={t("pickServer")}
            title={t("pickServer")}
            className="flex h-9 min-w-[150px] max-w-[230px] items-center gap-2 rounded-[10px] border border-line bg-white/[0.02] px-3 text-[13px] text-txt transition-colors hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] disabled:cursor-not-allowed"
          >
            <Globe className="size-3.5 shrink-0 text-txt3" aria-hidden />
            <span className="min-w-0 flex-1 truncate" dir="auto">
              {picking ? t("findingServer") : srv.label}
            </span>
            <ChevronDown className="size-3.5 shrink-0 text-txt3" aria-hidden />
          </button>
          <button
            type="button"
            onClick={() => void refreshAll()}
            disabled={refreshing}
            aria-label={t("refresh")}
            title={t("refresh")}
            className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-line bg-white/[0.02] text-txt2 transition-colors duration-200 hover:border-[var(--brand-line)] hover:bg-[var(--brand-bg)] hover:text-brand-strong disabled:cursor-wait"
          >
            <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} aria-hidden />
          </button>
        </div>
      </div>

      {/* One ring starts the run; while it runs it parks left with a spring,
          the same way Home's dial does, and the reading slides in beside it. */}
      <section className="rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]">
        <div className="flex min-h-[224px] items-center px-5 py-3">
          <motion.div
            layout
            transition={SPRING}
            className={cn("flex w-full items-center", parked ? "justify-start gap-7" : "justify-center")}
          >
            <motion.div layout transition={SPRING} className="flex shrink-0 items-center justify-center">
              <StartCircle running={running} done={phase === "done"} onStart={() => void run("all")} onStop={stop} />
            </motion.div>

            <AnimatePresence mode="popLayout">
              {showPanel && (
                <motion.div
                  key="read"
                  initial={{ opacity: 0, x: 16 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: 12 }}
                  transition={{ duration: 0.32, ease: EASE_OUT, delay: 0.06 }}
                  className="min-w-0 flex-1"
                >
                  <div className="flex items-baseline justify-between gap-4">
                    <div className="flex min-w-0 items-baseline gap-2">
                      <span
                        className="text-[44px] leading-none font-light tabular-nums text-txt"
                        style={{ letterSpacing: "-0.02em" }}
                      >
                        {unit === "ms" ? Math.round(value) : value >= 100 ? value.toFixed(0) : value.toFixed(1)}
                      </span>
                      <span className="shrink-0 text-[13px] text-txt3">{unit}</span>
                    </div>
                    {shown.length > 0 && (
                      <span className="shrink-0 text-[11.5px] tabular-nums text-txt3">
                        {t("peak")} {unit === "ms" ? Math.round(peak) : peak.toFixed(1)} {unit}
                      </span>
                    )}
                  </div>
                  <div className="mt-1.5 truncate text-[12.5px] text-txt2">
                    {phase === "done" ? t("doneLabel") : caption}
                  </div>
                  <div className="mt-3">
                    <SpeedGraph samples={shown} accent={accent} active={running} />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        </div>

        <div className="grid grid-cols-4 divide-x divide-line border-t border-line">
          <MetricTile icon={Gauge} title={t("pingTitle")} value={result.ping} unit={t("ms")} onClick={() => void run("ping")} disabled={running} />
          <MetricTile icon={Activity} title={t("jitter")} value={result.jitter} unit={t("ms")} onClick={() => void run("ping")} disabled={running} />
          <MetricTile
            icon={ArrowDown}
            title={t("chDown")}
            value={phase === "download" ? value : result.down}
            unit={t("mbps")}
            onClick={() => void run("down")}
            disabled={running}
          />
          <MetricTile
            icon={ArrowUp}
            title={t("chUp")}
            value={phase === "upload" ? value : result.up}
            unit={t("mbps")}
            onClick={() => void run("up")}
            disabled={running}
          />
        </div>

        {/* what the numbers mean, in one row, inside the same pane */}
        {verdicts.length > 0 && (
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2 border-t border-line px-5 py-3">
            {verdicts.map((v) => (
              <span key={v.label} className="flex items-center gap-2 text-[12px]">
                <v.icon className="size-3.5 shrink-0 text-txt3" strokeWidth={1.7} aria-hidden />
                <span
                  aria-hidden
                  className={cn(
                    "size-1.5 rounded-full",
                    v.tone === "ok" && "bg-[var(--green)]",
                    v.tone === "warn" && "bg-[var(--amber)]",
                    v.tone === "bad" && "bg-[var(--red)]",
                  )}
                />
                <span className="text-txt2">{v.text}</span>
              </span>
            ))}
          </div>
        )}
      </section>

      {/* Both ends of the test, stated the way a speed test states them:
          what is being measured against, who hosts it, and who you are. */}
      <section
        className={cn(
          "grid divide-x divide-line rounded-[16px] border border-line bg-[rgb(21_29_46/0.62)]",
          netInfo ? "grid-cols-3" : "grid-cols-2",
        )}
      >
        <FactRow
          icon={Server}
          title={t("selServer")}
          main={srv.label}
          lines={picking ? [t("findingServer")] : [srv.detail || srv.host]}
        />
        <FactRow
          icon={Cloud}
          title={t("hostedBy")}
          main={sponsor}
          lines={srv.id === "cloudflare" ? ["speed.cloudflare.com", "Cloudflare, Inc."] : [srv.host]}
        />
        {netInfo && (
          <FactRow
            icon={Wifi}
            title={t("yourConn")}
            main={netInfo.isp || netInfo.ip}
            lines={[netInfo.isp ? netInfo.ip : "", netInfo.place]}
          />
        )}
      </section>

      <div className="flex items-center justify-end gap-4 px-1">
        <p aria-live="polite" className="sr-only">
          {hint}
        </p>
        {history.length > 0 && (
          <button
            type="button"
            onClick={onOpenHistory}
            className="group flex shrink-0 items-center gap-1.5 text-[12.5px] text-txt3 transition-colors hover:text-brand-strong"
          >
            <History className="size-3.5" strokeWidth={1.7} aria-hidden />
            <span>{t("history")}</span>
            <span className="tabular-nums">{history.length}</span>
            <ChevronRight className="size-3.5 transition-transform duration-200 group-hover:translate-x-0.5" aria-hidden />
          </button>
        )}
      </div>

      <PickerDialog
        open={pickOpen}
        onOpenChange={setPickOpen}
        title={t("pickServer")}
        search={t("sheetSearch")}
        items={pickerItems}
        value={picked?.id ?? "auto"}
        onPick={(v) => void chooseServer(v)}
      />
    </div>
  )
}

type Verdict = { icon: typeof Gamepad2; label: string; tone: "ok" | "warn" | "bad"; text: string }

/**
 * Plain language instead of raw numbers: what the line can actually do.
 * Thresholds are the published practical ones - competitive play needs jitter
 * under ~10 ms, 1080p needs ~25 Mbps, 4K wants 50+.
 */
export function buildVerdicts(r: Result, t: (k: StrKey) => string): Verdict[] {
  const { ping, jitter, down } = r
  const out: Verdict[] = []

  if (ping !== null && jitter !== null) {
    if (ping <= 45 && jitter <= 10) out.push({ icon: Gamepad2, label: t("vGaming"), tone: "ok", text: t("vCompReady") })
    else if (ping <= 80 && jitter <= 20) out.push({ icon: Gamepad2, label: t("vGaming"), tone: "warn", text: t("vCasual") })
    else out.push({ icon: Gamepad2, label: t("vGaming"), tone: "bad", text: t("vLag") })
  }

  if (down !== null) {
    if (down >= 50) out.push({ icon: Tv, label: t("vStreaming"), tone: "ok", text: t("v4k") })
    else if (down >= 25) out.push({ icon: Tv, label: t("vStreaming"), tone: "ok", text: t("v1080") })
    else if (down >= 12) out.push({ icon: Tv, label: t("vStreaming"), tone: "warn", text: t("v720") })
    else out.push({ icon: Tv, label: t("vStreaming"), tone: "bad", text: t("vSlow") })
  }

  if (ping !== null && jitter !== null) {
    if (jitter <= 15 && ping <= 80) out.push({ icon: Video, label: t("vCalls"), tone: "ok", text: t("vCallsOk") })
    else out.push({ icon: Video, label: t("vCalls"), tone: "warn", text: t("vCallsBad") })
  }

  return out
}

/** The one button that matters here: a plain ring, like the app's dial. */
function StartCircle({
  running,
  done,
  onStart,
  onStop,
}: {
  running: boolean
  done: boolean
  onStart: () => void
  onStop: () => void
}) {
  const { t } = useI18n()
  const label = running ? t("stop") : done ? t("startAgain") : t("startTest")

  return (
    <motion.button
      type="button"
      onClick={running ? onStop : onStart}
      aria-label={label}
      whileTap={{ scale: 0.985 }}
      transition={{ type: "spring", stiffness: 460, damping: 32 }}
      className={cn(
        "relative grid size-[224px] place-items-center rounded-full border bg-[radial-gradient(circle_at_50%_36%,rgb(255_255_255/0.07),rgb(255_255_255/0.02)_74%)] transition-colors",
        running
          ? "border-[var(--brand-line)] text-brand-strong"
          : "border-[rgb(255_255_255/0.2)] text-txt hover:border-[var(--brand-line)] hover:text-brand-strong",
      )}
    >
      {running && (
        <span
          aria-hidden
          className="absolute -inset-px rounded-full border-2 border-transparent border-t-[var(--brand)] [animation:spin_1.15s_linear_infinite]"
        />
      )}
      <span className="flex flex-col items-center gap-2.5">
        {running ? (
          <Square className="size-8" strokeWidth={1.75} aria-hidden />
        ) : (
          <Play className="ms-1 size-9" strokeWidth={1.75} aria-hidden />
        )}
        <span className="text-[12.5px] font-medium tracking-[0.01em]">{label}</span>
      </span>
    </motion.button>
  )
}

function MetricTile({
  icon: Icon,
  title,
  value,
  unit,
  onClick,
  disabled,
}: {
  icon: typeof Gauge
  title: string
  value: number | null
  unit: string
  onClick: () => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={cn(
        "flex min-w-0 items-center gap-3 px-4 py-3 text-start transition-colors",
        !disabled && "hover:bg-white/[0.03]",
      )}
    >
      <span className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-line bg-[rgb(255_255_255/0.03)] text-brand-strong">
        <Icon className="size-4" strokeWidth={1.7} aria-hidden />
      </span>
      <span className="min-w-0">
        <span className="block truncate text-[11.5px] text-txt3">{title}</span>
        <span
          className={cn(
            "mt-0.5 block truncate text-[17px] font-medium tabular-nums",
            value === null ? "text-txt3" : "text-txt",
          )}
        >
          {value === null ? "-" : unit === "ms" ? Math.round(value) : value.toFixed(1)}
          <span className="ms-1 text-[11px] font-normal text-txt3">{unit}</span>
        </span>
      </span>
    </button>
  )
}

function FactRow({
  icon: Icon,
  title,
  main,
  lines,
}: {
  icon: typeof Gauge
  title: string
  main: string
  lines: string[]
}) {
  return (
    <div className="flex min-w-0 items-center gap-3.5 px-5 py-3.5">
      <span className="grid size-10 shrink-0 place-items-center rounded-[12px] border border-line bg-[rgb(255_255_255/0.03)] text-brand-strong">
        <Icon className="size-[18px]" strokeWidth={1.7} aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="text-[10.5px] font-medium tracking-[0.08em] text-txt3 uppercase">{title}</div>
        <div className={cn("mt-0.5 leading-snug font-semibold break-words text-txt", fitSize(main, true))} dir="auto">
          {main}
        </div>
        {lines.filter(Boolean).map((l, i) => (
          <div key={i} className={cn("truncate text-txt3", fitSize(l, false))} dir="auto">
            {l}
          </div>
        ))}
      </div>
    </div>
  )
}

/** Long server names must fit their column: the value steps down a size as
 *  it grows, and anything still too long wraps instead of clipping. */
function fitSize(s: string, main: boolean): string {
  const n = s.length
  if (main) return n > 30 ? "text-[12px]" : n > 20 ? "text-[13px]" : "text-[14px]"
  return n > 30 ? "text-[10px]" : n > 24 ? "text-[10.5px]" : "text-[11.5px]"
}
