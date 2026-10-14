import { useSyncExternalStore } from "react"
import { api, type WeUnit } from "@/lib/ipc"

export type { WeUnit }
export type QuotaPhase = "idle" | "need-code" | "busy" | "ready" | "error"

export type QuotaSnap = {
  linked: boolean
  customer: string
  number: string
  units: WeUnit[]
  updatedAt: number | null
  phase: QuotaPhase
  msgKey: string
  msgRaw: string
  challengeImage: string
}

const NUM_KEY = "qc-we-number"
const PASS_KEY = "qc-we-pass"

/** The saved password, for finishing a code challenge the refresh started. */
export function savedQuotaPass(): string {
  return load(PASS_KEY)
}

function load(k: string): string {
  try {
    return localStorage.getItem(k) || ""
  } catch {
    return ""
  }
}

function save(k: string, v: string) {
  try {
    if (v) localStorage.setItem(k, v)
    else localStorage.removeItem(k)
  } catch {
    /* private mode */
  }
}

let snap: QuotaSnap = {
  linked: false,
  customer: "",
  number: load(NUM_KEY),
  units: [],
  updatedAt: null,
  phase: "idle",
  msgKey: "",
    msgRaw: "",
  challengeImage: "",
}

const listeners = new Set<() => void>()
function set(p: Partial<QuotaSnap>) {
  snap = { ...snap, ...p }
  listeners.forEach((l) => l())
}
function subscribe(l: () => void) {
  listeners.add(l)
  return () => {
    listeners.delete(l)
  }
}

export function useQuota(): QuotaSnap {
  return useSyncExternalStore(subscribe, () => snap)
}

const acctOf = (n: string) => `FBB${n.trim().replace(/^0+/, "")}`

function errKey(e: unknown): string {
  const s = String(e instanceof Error ? e.message : e)
  if (s.includes("NET:")) return "weNetFail"
  if (s.includes("login refused")) return "weLoginFail"
  if (s.includes("check refused") || s.includes("captcha")) return "weCaptchaFail"
  if (s.includes("offers refused")) return "weOffersFail"
  return "weGenericFail"
}

/** The server's own refusal word (Blocked and friends), safe to show. */
function errRaw(e: unknown): string {
  const s = String(e instanceof Error ? e.message : e)
  const m = s.match(/check refused ([A-Za-z-]+)/)
  return m ? m[1] : ""
}
async function pull(number: string, password: string, code: string): Promise<boolean> {
  const login = await api.weQuotaLogin(number, password, code)
  save(NUM_KEY, number)
  save(PASS_KEY, password)
  const units = await api.weQuotaUsage(acctOf(number))
  set({
    linked: true,
    customer: login.customer,
    number,
    units,
    updatedAt: Date.now(),
    phase: "ready",
    msgKey: "",
    msgRaw: "",
    challengeImage: "",
  })
  return true
}

/** Link step one: asks the server whether it wants a code read. When it
    does, this resolves false and the screen shows the image instead. */
export async function quotaLink(number: string, password: string, code = ""): Promise<boolean> {
  set({ phase: "busy", msgKey: "", msgRaw: "" })
  try {
    if (!code && snap.phase !== "need-code") {
      const ch = await api.weQuotaChallenge(number)
      if (ch.interaction) {
        set({ phase: "need-code", challengeImage: ch.image, number })
        return false
      }
    }
    return await pull(number, password, code)
  } catch (e) {
    set({ phase: "error", msgKey: errKey(e), msgRaw: errRaw(e) })
    return false
  }
}

let refreshTried = false

/** Silent refresh with the saved number and password. When the server
    wants a code read it stops at need-code for the Settings screen. */
export async function quotaRefresh(force = false): Promise<void> {
  const number = load(NUM_KEY)
  const password = load(PASS_KEY)
  if (!number || !password) return
  if (!force && (snap.linked || refreshTried)) return
  refreshTried = true
  if (!snap.linked) set({ phase: "busy", msgKey: "", msgRaw: "" })
  try {
    const ch = await api.weQuotaChallenge(number)
    if (ch.interaction) {
      set({ phase: "need-code", challengeImage: ch.image, number })
      return
    }
    await pull(number, password, "")
  } catch (e) {
    set({ phase: snap.linked ? "ready" : "error", msgKey: errKey(e), msgRaw: errRaw(e) })
  }
}

export async function quotaLogout(): Promise<void> {
  try {
    await api.weQuotaLogout()
  } catch {
    /* engine already forgot */
  }
  save(NUM_KEY, "")
  save(PASS_KEY, "")
  refreshTried = false
  set({
    linked: false,
    customer: "",
    units: [],
    updatedAt: null,
    phase: "idle",
    msgKey: "",
    msgRaw: "",
    challengeImage: "",
  })
}

/** The API counts in MB; paint GB once it matters. */
export function fmtQuota(v: number): string {
  if (v >= 1024) return `${(v / 1024).toFixed(1)} GB`
  return `${Math.round(v)} MB`
}
