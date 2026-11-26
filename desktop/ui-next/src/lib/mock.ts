/**
 * Browser fallback so the UI runs (and can be screenshotted) without the
 * Rust engine: `npm run dev` in a plain browser, or the preview harness.
 * Values mirror the shapes in lib.rs; the tunnel "runs" so connected-state
 * screens are reachable with ?vpn=1.
 */
import type { Card, CardStatus, CmdResult, DirectStatus, LanStatus, Space, SpaceProbe, SpacesState, TrafficState, TunnelState, UpdateInfo } from "./ipc"

const cards: Card[] = [
  { name: "Gamerz (playvalorant.com)", uuid: "11111111-1111-4111-8111-111111111111", card_type: "Gamerz", sni: "playvalorant.com", wg_private: "", wg_addr: "", registered: true },
  { name: "Streamerz (youtube.com)", uuid: "22222222-2222-4222-8222-222222222222", card_type: "Streamerz", sni: "youtube.com", wg_private: "", wg_addr: "", registered: true },
]

let running = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("vpn") === "1"
let rx = 0
let tx = 0
const started = Date.now()

// The Spaces demo: one owned space, a live member and a sleeping one, so the
// roster, the invite block and the online count can all be judged without a
// coordinator. Nothing here ships.
const demoMember = (id: string, name: string, address: string, online: boolean, self: boolean, owner: boolean) => ({
  id,
  name,
  address,
  online,
  self_pc: self,
  owner,
  last_seen: Math.floor(Date.now() / 1000) - (online ? 20 : 3600),
})
// ?spaces=empty shows the first-run state without clearing the demo data.
const EMPTY_SPACES =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("spaces") === "empty"

let demoSpaces: Space[] = EMPTY_SPACES ? [] : [
  {
    id: "K7M2Q9X4",
    name: "Gaming",
    address: "10.77.3.7",
    subnet: "10.77.3.0/24",
    owner: true,
    invite: "qvs1:K7M2Q9X4:6f1a2b3c4d5e6f708192a3b4c5d6e7f8",
    online: 2,
    members: [
      demoMember("m-1", "Yousef-PC", "10.77.3.7", true, true, true),
      demoMember("m-2", "Sara-Laptop", "10.77.3.8", true, false, false),
      demoMember("m-3", "Studio-PC", "10.77.3.9", false, false, false),
    ],
  },
]
let demoSpaceActive = EMPTY_SPACES ? "" : "K7M2Q9X4"
let demoDirect: DirectStatus = {
  running: false,
  peer: "",
  address: "",
  adapter: "",
  error: "",
}
// What the live tunnel actually loaded. Changing the selected space does not
// change this until the next connect, which is the state the screen must show.
let demoTunnelSpace = running ? demoSpaceActive : ""

const demoSpacesState = (): SpacesState => ({
  spaces: demoSpaces,
  active: demoSpaceActive,
  tunnel_space: running ? demoTunnelSpace : "",
  tunnel_on: running,
})

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
  { ip: "192.168.1.2", mac: "9c:69:d3:4c:30:c8", name: "verhafter-pc", self_pc: true, source: "self", protected: true, protected_always: false, live: true },
  { ip: "192.168.1.1", mac: "f0:a9:51:c1:eb:da", name: "Huawei device", self_pc: false, source: "router", protected: true, protected_always: true, live: true },
  { ip: "192.168.1.3", mac: "b2:8c:39:3a:0e:92", name: "Pixel 9", self_pc: false, source: "router", protected: false, protected_always: false, live: true },
  { ip: "192.168.1.6", mac: "18:1d:ea:e0:56:ff", name: "WIN-1734L96PVNJ", self_pc: false, source: "router", protected: false, protected_always: false, live: true },
  { ip: "192.168.1.23", mac: "b2:8c:39:3a:0e:91", name: "V2514", self_pc: false, source: "router", protected: false, protected_always: false, live: true },
  { ip: "192.168.1.35", mac: "1e:92:58:2f:e9:4f", name: "thabet-s-A53", self_pc: false, source: "router", protected: false, protected_always: false, live: false },
  { ip: "192.168.1.46", mac: "82:d0:c2:a6:fe:c8", name: "uplus-haier-tv", self_pc: false, source: "router", protected: false, protected_always: false, live: true },
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
let demoActive: LanStatus | null = null

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
      return {
        server_ip: "qv-speed.example.com",
        cards,
        version: "0.4.15",
        update_channel: "normal",
      } as T
    case "spaces_state":
      return demoSpacesState() as T
    case "space_device_name":
      return "Preview-PC" as T
    case "space_create": {
      const name = String(args?.name ?? "New space").trim() || "New space"
      const device = String(args?.deviceName ?? "This PC").trim() || "This PC"
      const id = "NEWSPACE"
      const space: Space = {
        id,
        name,
        address: "10.77.9.2",
        subnet: "10.77.9.0/24",
        owner: true,
        invite: `qvs1:${id}:9a8b7c6d5e4f30211234567890abcdef`,
        online: 1,
        members: [demoMember("m-new", device, "10.77.9.2", true, true, true)],
      }
      demoSpaces = [...demoSpaces.filter((s) => s.id !== id), space]
      demoSpaceActive = id
      return demoSpacesState() as T
    }
    case "space_join": {
      const device = String(args?.deviceName ?? "This PC").trim() || "This PC"
      const id = "JOINSPC1"
      const space: Space = {
        id,
        name: "Joined space",
        address: "10.77.4.3",
        subnet: "10.77.4.0/24",
        owner: false,
        invite: "",
        online: 2,
        members: [
          demoMember("m-own", "Owner-PC", "10.77.4.2", true, false, true),
          demoMember("m-me", device, "10.77.4.3", true, true, false),
        ],
      }
      demoSpaces = [...demoSpaces.filter((s) => s.id !== id), space]
      demoSpaceActive = id
      return demoSpacesState() as T
    }
    case "space_refresh":
      return demoSpacesState() as T
    case "space_direct_status":
      return demoDirect as T
    case "space_direct_start": {
      await new Promise((r) => setTimeout(r, 1600))
      demoDirect = {
        running: true,
        peer: "Sara-Laptop",
        address: "10.77.3.7",
        adapter: "QuotaSpaceK7M2",
        error: "",
      }
      return demoDirect as T
    }
    case "space_direct_stop":
      demoDirect = {
        running: false,
        peer: demoDirect.peer,
        address: demoDirect.address,
        adapter: demoDirect.adapter,
        error: "",
      }
      return demoDirect as T
    case "space_probe": {
      await new Promise((r) => setTimeout(r, 1200))
      const relay =
        typeof window !== "undefined" &&
        new URLSearchParams(window.location.search).get("relay") === "1"
      const result: SpaceProbe = relay
        ? {
            direct: false,
            rtt_ms: null,
            self_endpoint: "203.0.113.42:51234",
            peer_endpoint: "198.51.100.7:51820",
            note: "no direct packet arrived; the relay would be used",
          }
        : {
            direct: true,
            rtt_ms: 9.4,
            self_endpoint: "203.0.113.42:51234",
            peer_endpoint: "198.51.100.7:51820",
            note: "",
          }
      return result as T
    }
    case "space_activate": {
      const id = String(args?.id ?? "")
      demoSpaceActive = id
      return demoSpacesState() as T
    }
    case "space_leave":
    case "space_delete": {
      const id = String(args?.id ?? "")
      demoSpaces = demoSpaces.filter((s) => s.id !== id)
      if (demoSpaceActive === id) demoSpaceActive = ""
      return demoSpacesState() as T
    }
    case "space_rotate": {
      const id = String(args?.id ?? "")
      demoSpaces = demoSpaces.map((s) =>
        s.id === id ? { ...s, invite: `qvs1:${id}:00112233445566778899aabbccddeeff` } : s,
      )
      return demoSpacesState() as T
    }
    case "space_kick": {
      const id = String(args?.id ?? "")
      const memberId = String(args?.memberId ?? "")
      demoSpaces = demoSpaces.map((s) =>
        s.id === id ? { ...s, members: s.members.filter((m) => m.id !== memberId) } : s,
      )
      return demoSpacesState() as T
    }
    case "lan_known":
    case "lan_devices":
      return DEMO_DEVICES as T
    case "lan_scan":
      return true as T
    case "lan_monitor_active":
      return true as T
    case "lan_traffic":
      return demoTraffic() as T
    case "lan_status":
      if (!demoActive) return null as T
      demoTraffic()
      if (demoCounters[demoActive.ip]) {
        demoActive.up_bytes = demoCounters[demoActive.ip].up
        demoActive.down_bytes = demoCounters[demoActive.ip].down
      }
      return { ...demoActive } as T
    case "lan_start": {
      if (!npcapInstalled) throw new Error("Npcap is not installed")
      const ip = String(args?.ip || "")
      const dev = DEMO_DEVICES.find((d) => d.ip === ip)
      if (!dev) throw new Error("that device is no longer on the network")
      if (dev.self_pc) throw new Error("this PC cannot be controlled")
      if (dev.protected_always)
        throw new Error("the router cannot be controlled: its hardware address holds the gateway")
      demoActive = {
        ip,
        mac: String(args?.mac || dev.mac),
        down_kbps: 0,
        up_kbps: 0,
        blocked: Boolean(args?.blocked),
        up_bytes: 0,
        down_bytes: 0,
      }
      return true as T
    }
    case "lan_stop":
      demoActive = null
      return true as T
    case "lan_limits":
      if (demoActive) {
        demoActive.down_kbps = Math.max(0, Number(args?.downKbps) || 0)
        demoActive.up_kbps = Math.max(0, Number(args?.upKbps) || 0)
      }
      return true as T
    case "lan_block":
      if (demoActive) demoActive.blocked = Boolean(args?.on)
      return true as T
    case "lan_guard":
      return { watching: true, attacker_mac: "" } as T
    case "lan_claim_ready":
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
    case "pipeline_ready":
      return true as T
    case "router_status":
      return true as T
    case "router_autolink":
      return true as T
    case "copy_text":
      return true as T
    case "card_status":
      return {
        imported: true,
        uuid: String(args?.uuid || cards[0]?.uuid || ""),
        ok: true,
        message: "Active on the server.",
      } as CardStatus as T
    case "import_credential": {
      const text = String(args?.text || "")
      const match = text.match(/qc1:([0-9a-fA-F-]{36}):/)
      const uuid = match ? match[1].toLowerCase() : crypto.randomUUID()
      const kind = String(args?.kind || "Gamerz")
      if (!cards.some((c) => c.uuid === uuid)) {
        cards.push({ name: "Card", uuid, card_type: kind, sni: "ea.com", wg_private: "", wg_addr: "", registered: false })
      }
      return { ok: true, msg: "Credential imported.", uuid } as T
    }
    case "forget_card": {
      const i = cards.findIndex((c) => c.uuid === args?.uuid)
      if (i >= 0) cards.splice(i, 1)
      return ok("Card removed from this PC. The server still has it.") as T
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
      demoTunnelSpace = demoSpaceActive
      return ok("Engine started.") as T
    case "tunnel_stop":
      running = false
      demoTunnelSpace = ""
      return ok("Engine stopped.") as T
    case "tunnel_status":
      return { running, error: "" } as TunnelState as T
    case "tunnel_health":
      return 180 as T
    case "process_running":
      return false as T
    case "game_running":
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
