//! Voice-rule probe: writes a voice-only tun config from the app's own card
//! (never prints the uuid), checks it with the engine's decoder, and leaves
//! the file in place for the socks reachability harness (swap the tun
//! inbound for a socks listener, run the engine, curl a voice IP through it
//! versus direct). No tunnel is spawned here.
use quotacards::config::DEFAULT_HOST;
use quotacards::vpn;

fn main() {
    let cfg = std::env::var("APPDATA").unwrap_or_default() + "\\quotacards\\config.json";
    let raw = std::fs::read_to_string(&cfg).expect("read app config");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let cards = v["cards"].as_array().expect("cards");
    let card = cards
        .iter()
        .find(|c| c["card_type"] == "Gamerz")
        .or_else(|| cards.last())
        .expect("a card");
    let uuid = card["uuid"].as_str().expect("uuid").to_string();
    let sni = card["sni"].as_str().unwrap_or("example.com").to_string();
    println!("using card: sni={sni} uuid_len={}", uuid.len());

    let list: Vec<String> = vec![];
    let p = vpn::write_tun_config(&uuid, DEFAULT_HOST, &sni, "allow", &list, true, "vless", "", None)
        .expect("write voice config");
    println!("config: {p:?}");
    vpn::check_config().expect("engine accepts the voice config");
    println!("engine check OK");

    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    let rules = v["route"]["rules"].as_array().unwrap();
    println!("route rules: {}", rules.len());
    for r in rules {
        let out = r["outbound"].as_str().unwrap_or("-");
        let what = if let Some(a) = r["ip_cidr"].as_array() {
            format!("ip_cidr x{} first={}", a.len(), a[0])
        } else if let Some(a) = r["process_name"].as_array() {
            format!(
                "process x{} port={:?} domain={:?}",
                a.len(),
                r.get("port_range").cloned().unwrap_or(serde_json::Value::Null),
                r.get("domain_suffix").cloned().unwrap_or(serde_json::Value::Null)
            )
        } else {
            format!("{} {:?}", r["action"].as_str().unwrap_or("rule"), r.get("network"))
        };
        println!("  [{out}] {what}");
    }
    println!("route.final: {}", v["route"]["final"]);
}
