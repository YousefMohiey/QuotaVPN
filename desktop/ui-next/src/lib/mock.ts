/**
 * Browser fallback so the UI runs (and can be screenshotted) without the
 * Rust engine: `npm run dev` in a plain browser, or the preview harness.
 * Values mirror the shapes in lib.rs; the tunnel "runs" so connected-state
 * screens are reachable with ?vpn=1.
 */
import type { Card, CmdResult, TrafficState, TunnelState, UpdateInfo } from "./ipc"

const cards: Card[] = [
  { name: "Gamerz (playvalorant.com)", uuid: "11111111-1111-4111-8111-111111111111", card_type: "Gamerz", sni: "playvalorant.com", wg_private: "", wg_addr: "" },
  { name: "Streamerz (youtube.com)", uuid: "22222222-2222-4222-8222-222222222222", card_type: "Streamerz", sni: "youtube.com", wg_private: "", wg_addr: "" },
]

let running = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("vpn") === "1"
let rx = 0
let tx = 0
const started = Date.now()

const APPS = [
  "Discord", "Steam", "VALORANT", "Riot Client", "Google Chrome", "Firefox", "Opera GX",
  "Spotify", "Telegram", "WhatsApp", "Visual Studio Code", "OBS Studio", "Epic Games Launcher",
  "Battle.net", "League of Legends", "Minecraft Launcher", "Zoom", "Microsoft Teams", "Notepad++",
]

export async function mockCall<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 40))
  const ok = (msg = ""): CmdResult => ({ ok: true, msg })
  switch (cmd) {
    case "get_state":
      return { server_ip: "qc-speed.example.com", ssh_user: "ubuntu", ssh_port: 22, cards, version: "0.4.8", update_channel: "normal" } as T
    case "probe_server":
      return ok("Server reachable.") as T
    case "generate_card": {
      const name = String(args?.name || "New card")
      const kind = String(args?.kind || "Gamerz")
      const sni = String(args?.sni || "")
      cards.push({ name, uuid: crypto.randomUUID(), card_type: kind, sni, wg_private: "", wg_addr: "" })
      return ok("Card created.") as T
    }
    case "revoke_card": {
      const i = cards.findIndex((c) => c.uuid === args?.uuid)
      if (i >= 0) cards.splice(i, 1)
      return ok("Card revoked.") as T
    }
    case "copy_card":
      return ok("Link copied.") as T
    case "set_card_sni": {
      const c = cards.find((x) => x.uuid === args?.uuid)
      if (c) c.sni = String(args?.sni || c.sni)
      return ok("Domain updated.") as T
    }
    case "tunnel_start":
      running = true
      return ok("Engine started.") as T
    case "tunnel_stop":
      running = false
      return ok("Engine stopped.") as T
    case "tunnel_status":
      return { running, error: "" } as TunnelState as T
    case "tunnel_health":
      return 180 as T
    case "process_running":
      return false as T
    case "autostart_get":
      return false as T
    case "autostart_set":
      return undefined as T
    case "tunnel_traffic":
      if (running) {
        rx += 1_400_000 + Math.random() * 3_000_000
        tx += 180_000 + Math.random() * 900_000
      }
      return { rx, tx } as TrafficState as T
    case "tunnel_probe":
      return ok("Tunnel is up.") as T
    case "tunnel_log":
      return "[mock] engine log line\n[mock] tunnel up\n" as T
    case "tunnel_copy_log":
      return ok("Log copied.") as T
    case "import_card":
      return ok("Card added.") as T
    case "app_icons":
      // The preview has no executables to read icons from: the rows tile.
      return [] as unknown as T
    case "tunnel_apps":
      return JSON.stringify(APPS.map((label) => ({ pkg: label.toLowerCase().replace(/\s+/g, "") + ".exe", label }))) as T
    case "resolve_host":
      // The hero resolves the server name only while connected, and the
      // world then sees the server's address. Demo value: never a real one.
      return "198.51.100.7" as T
    case "net_info":
      // Exit facts for the demo: through the tunnel the world sees the
      // server, otherwise it sees the local address. Both are documentation
      // addresses (RFC 5737) so the demo never prints anything real.
      return (running
        ? { ip: "198.51.100.7", isp: "Nimbus Hosting", place: "Frankfurt, Germany" }
        : { ip: "203.0.113.42", isp: "WE", place: "Cairo, Egypt" }) as T
    case "check_update": {
      // The "update ready" reminder appears only when asked for (?upd=1);
      // the real check reports available only when GitHub says so.
      const wantUpd = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("upd") === "1"
      return { current: "0.4.8", latest: wantUpd ? "0.4.9" : "0.4.8", available: wantUpd, url: "#", notes: "" } as UpdateInfo as T
    }
    case "apply_update":
      return "Up to date." as T
    case "set_update_channel":
      return "Channel saved." as unknown as T
    case "we_quota_challenge": {
      const wantCode =
        typeof window !== "undefined" &&
        new URLSearchParams(window.location.search).get("quota") === "code"
      return { interaction: wantCode, image: "" } as unknown as T
    }
    case "we_quota_login":
      return { customer: "Demo User" } as unknown as T
    case "we_quota_usage":
      return [
        {
          offer: "PLUS 2",
          total: 200,
          used: 74.5,
          remain: 125.5,
          // the demo can be asked for fewer packages (?buckets=1|2) so the
          // adaptive layout is testable without touching a real account
          buckets: [
            { label: "Main Quota", item: "C_TED_Primary_Fixed_Data", total: 100, remain: 62.5 },
            { label: "Gaming quota X 2", item: "C_TED_Primary_Fixed_Sonic_Gaming", total: 50, remain: 35 },
            { label: "Streaming quota X 2", item: "C_TED_Primary_Fixed_Sonic_AllRacing", total: 50, remain: 28 },
          ].slice(
            0,
            (() => {
              const raw =
                typeof window !== "undefined"
                  ? new URLSearchParams(window.location.search).get("buckets")
                  : null
              const n = raw ? Number(raw) : 3
              return Number.isFinite(n) && n >= 1 && n <= 3 ? n : 3
            })(),
          ),
        },
      ] as unknown as T
    case "we_quota_logout":
      return undefined as T
    default:
      return ok() as T
  }
}

export const mockElapsed = () => Math.round((Date.now() - started) / 1000)
