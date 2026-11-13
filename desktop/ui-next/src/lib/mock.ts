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

// ?slow=1 stretches every call so the connecting state can be looked at
// instead of photographed by luck.
const SLOW = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("slow") === "1"


// The network screen in the dev render: a handful of devices, a live verdict
// and counters that keep climbing, so the roster and its rates can be judged
// without touching a real network. Nothing here ships.
const DEMO_DEVICES = [
  { ip: "192.168.1.2", mac: "9c:69:d3:4c:30:c8", name: "win-studio", self_pc: true, source: "self", protected: true, protected_always: false, live: true },
  { ip: "192.168.1.1", mac: "f0:a9:51:c1:eb:da", name: "Huawei device", self_pc: false, source: "router", protected: true, protected_always: true, live: true },
  { ip: "192.168.1.3", mac: "04:7c:16:7d:e7:02", name: "Micro-star intl", self_pc: false, source: "vendor", protected: false, protected_always: false, live: true },
  { ip: "192.168.1.6", mac: "18:1d:ea:e0:56:ff", name: "Ei", self_pc: false, source: "dhcp", protected: false, protected_always: false, live: true },
  { ip: "192.168.1.23", mac: "b2:8c:39:3a:0e:91", name: "Private address", self_pc: false, source: "mdns", protected: false, protected_always: false, live: true },
  { ip: "192.168.1.35", mac: "1e:92:58:2f:e9:4f", name: "Private address", self_pc: false, source: "vendor", protected: false, protected_always: false, live: false },
  { ip: "192.168.1.46", mac: "82:d0:c2:a6:fe:c8", name: "private device", self_pc: false, source: "vendor", protected: false, protected_always: false, live: true },
]

const DEMO_RATE: Record<string, [number, number]> = {
  "192.168.1.2": [12_000, 340_000],
  "192.168.1.1": [4_000, 9_000],
  "192.168.1.3": [420_000, 96_000],
  "192.168.1.6": [1_800_000, 60_000],
  "192.168.1.23": [180_000, 12_000],
  "192.168.1.35": [0, 0],
  "192.168.1.46": [64_000, 3_000],
}

// The packet driver's state in the demo: installed unless the page asks for
// ?npcap=0 (missing, so the install path is visible), ?npcap=instfail (missing
// and the install refuses) or ?npcap=unknown (the readiness read fails, which
// must leave the screen without a prompt).
const NPCAP_MODE = (() => {
  try {
    return new URLSearchParams(window.location.search).get("npcap") || "on"
  } catch (e) {
    return "on"
  }
})()
let npcapInstalled = NPCAP_MODE === "on" || NPCAP_MODE === "claimfail"

// The one routed device, so the stage and the per row control have something
// real to show. A claim on the router or on this machine is refused with the
// kind of message the engine sends, so the failure path is visible too.
let routedDevice: { ip: string; mac: string } | null = null

const DEMO_DESTINATIONS = [
  { process: "VALORANT", host: "162.249.72.1", up: 412_000, down: 8_640_000 },
  { process: "Discord", host: "cloudfront.net", up: 96_000, down: 1_240_000 },
  { process: "Google Chrome", host: "youtube.com", up: 48_000, down: 940_000 },
]

const demoCounters: Record<string, { up: number; down: number }> = {}
let demoAt = Date.now()

function demoTraffic() {
  const now = Date.now()
  const dt = Math.max(0.2, (now - demoAt) / 1000)
  demoAt = now
  for (const d of DEMO_DEVICES) {
    const [down, up] = DEMO_RATE[d.ip] ?? [0, 0]
    const c = demoCounters[d.ip] ?? { up: 0, down: 0 }
    c.down += Math.round(down * dt)
    c.up += Math.round(up * dt)
    demoCounters[d.ip] = c
  }
  // The counters carry the hardware address, because that is what the screen
  // joins on: a row keyed only by address stops matching the moment the lease
  // moves, and the demo would show dashes for every device.
  return DEMO_DEVICES.map((d) => ({
    mac: d.mac,
    ip: d.ip,
    up: demoCounters[d.ip].up,
    down: demoCounters[d.ip].down,
  }))
}

export async function mockCall<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, SLOW ? 2600 : 40))
  const ok = (msg = ""): CmdResult => ({ ok: true, msg })
  switch (cmd) {
    case "get_state":
      return { server_ip: "qc-speed.example.com", ssh_user: "ubuntu", ssh_port: 22, cards, version: "0.4.15", update_channel: "normal" } as T
    case "lan_known":
    case "lan_devices":
      return DEMO_DEVICES as T
    case "lan_monitor_active":
      return true as T
    case "lan_traffic":
      return demoTraffic() as T
    case "lan_status":
      return null as T
    case "lan_guard":
      return { watching: true, attacker_mac: "" } as T
    case "lan_npcap_ready":
      if (NPCAP_MODE === "unknown") throw new Error("the driver state could not be read")
      return npcapInstalled as T
    case "lan_route_diagnose": {
      await new Promise((r) => setTimeout(r, 600))
      const ip = String(args?.ip || "")
      const mac = String(args?.mac || "")
      const target = ip ? `${ip} ${mac}`.trim() : "no device given"
      return [
        "Npcap driver: loaded, version 1.79",
        "this machine: 192.168.1.2, hardware 9c:69:d3:4c:30:c8",
        "gateway: 192.168.1.1, hardware f0:a9:51:c1:eb:da",
        "neighbour table gateway: f0:a9:51:c1:eb:da, ours: no",
        "forwarding: on, IPv4 routing enabled",
        "engine: running, tun up at 172.19.0.1",
        "intention: none recorded",
        "firewall inbound rule: absent",
        "firewall forward rule: absent",
        `frames from ${target} in 6s: 148`,
      ] as T
    }
    case "lan_install_npcap":
      if (NPCAP_MODE === "instfail")
        throw new Error("the vendor download was refused (HTTP 403)")
      await new Promise((r) => setTimeout(r, 1600))
      npcapInstalled = true
      return "Npcap is installed. Routing works now." as T
    case "lan_route_device": {
      // The state says the driver is there and the claim still refuses for it:
      // this is the case the alert has to answer with the install.
      if (NPCAP_MODE === "claimfail")
        throw new Error("Npcap is not installed, so this device cannot be routed")
      const ip = String(args?.ip || "")
      const mac = String(args?.mac || "")
      const dev = DEMO_DEVICES.find((d) => d.ip === ip)
      if (!dev) throw new Error("that device is no longer on the network")
      if (dev.self_pc) throw new Error("this machine cannot be routed through itself")
      if (dev.protected_always)
        throw new Error("the router cannot be routed: its hardware address holds the gateway")
      routedDevice = { ip, mac: mac || dev.mac }
      return true as T
    }
    case "lan_unroute_device":
      routedDevice = null
      return true as T
    case "lan_routed_device":
      return routedDevice as T
    case "tunnel_destinations":
      return DEMO_DESTINATIONS as T
    case "npcap_ready":
      return true as T
    case "router_status":
      return true as T
    case "router_autolink":
      return true as T
    case "copy_text":
      return true as T
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
      return { current: "0.4.15", latest: wantUpd ? "0.5.0" : "0.4.15", available: wantUpd, url: "#", notes: "" } as UpdateInfo as T
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
