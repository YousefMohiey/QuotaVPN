//! Card credentials and request signing, with no platform I/O.
//!
//! The Windows app owns the DPAPI-sealed store and the HTTPS transport. The
//! phone keeps its legacy SSH path for now, so this module has to compile on
//! every target: parsing, HMAC, request shape and response mapping only.
//!
//! The credential format is `qv1:<uuid>:<64 hex characters>`. The old
//! `qc1:` prefix is still accepted so existing cards keep working. The secret is
//! issued per card by the server, never derived from the VLESS UUID, and never
//! written to disk in the clear by anything in this module.

/// Where the card API listens. Kept next to the signing code so the client and
/// the release checks agree on one host.
pub const DEFAULT_API_BASE: &str = "https://quotavpn.duckdns.org:8444";

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

/// Canonical lowercase `8-4-4-4-12` UUID, or None.
///
/// The server helper validates the same shape, and signing uses the exact text
/// that was imported, so a stray uppercase or a missing hyphen must be refused
/// here rather than turned into a signature the server cannot match.
pub fn canonical_uuid(text: &str) -> Option<String> {
    let t = text.trim().to_ascii_lowercase();
    let bytes = t.as_bytes();
    if bytes.len() != 36 {
        return None;
    }
    for (i, c) in bytes.iter().enumerate() {
        let dash_here = matches!(i, 8 | 13 | 18 | 23);
        if dash_here {
            if *c != b'-' {
                return None;
            }
        } else if !c.is_ascii_hexdigit() {
            return None;
        }
    }
    Some(t)
}

/// Decode a 64-character hex secret. Used by the credential store as well as
/// by the `qv1:` parser, so the two can never disagree about the shape.
pub fn secret_from_hex(text: &str) -> Result<Vec<u8>, String> {
    let t = text.trim();
    if t.len() != 64 || !t.as_bytes().iter().all(|c| c.is_ascii_hexdigit()) {
        return Err("the secret is not 64 hex characters".into());
    }
    let mut out = Vec::with_capacity(32);
    for i in (0..t.len()).step_by(2) {
        let byte = u8::from_str_radix(&t[i..i + 2], 16)
            .map_err(|_| "the secret could not be decoded".to_string())?;
        out.push(byte);
    }
    Ok(out)
}

pub fn secret_to_hex(secret: &[u8]) -> String {
    hex(secret)
}

/// One card's management credential. Debug redacts the secret on purpose.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    pub uuid: String,
    pub secret: Vec<u8>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("uuid", &self.uuid)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// Parse one `qv1:<uuid>:<secret>` line. The old `qc1:` prefix is accepted.
pub fn parse_credential(line: &str) -> Result<Credential, String> {
    let t = line.trim();
    let rest = t
        .strip_prefix("qv1:")
        .or_else(|| t.strip_prefix("qc1:"))
        .ok_or("that is not a card credential (expected qv1:<id>:<secret>)")?;
    let (uuid_text, secret_text) = rest
        .split_once(':')
        .ok_or("the card credential is missing its secret")?;
    let uuid = canonical_uuid(uuid_text).ok_or("the card id in that line is malformed")?;
    let secret = secret_from_hex(secret_text)?;
    Ok(Credential { uuid, secret })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The parts of a `vless://` link the app needs to place a card: its UUID, the
/// SNI it rides, and the display label. Everything else is ignored on purpose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VlessCard {
    pub uuid: String,
    pub sni: String,
    pub name: String,
}

pub fn parse_vless(text: &str) -> Option<VlessCard> {
    let t = text.trim();
    if !t
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("vless://"))
    {
        return None;
    }
    let body = &t[8..];
    let (userinfo, after) = body.split_once('@')?;
    let uuid = canonical_uuid(userinfo)?;
    let before_frag = after.split('#').next().unwrap_or("");
    let query = before_frag.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut sni = String::new();
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key.eq_ignore_ascii_case("sni") {
            sni = percent_decode(value).trim().to_ascii_lowercase();
            break;
        }
    }
    let name = after
        .split_once('#')
        .map(|(_, fragment)| percent_decode(fragment))
        .unwrap_or_default()
        .trim()
        .to_string();
    Some(VlessCard { uuid, sni, name })
}

/// What one paste produced. The credential is required; the link is optional,
/// but when it is present it must name the same card.
pub struct ParsedPaste {
    pub credential: Credential,
    pub sni: Option<String>,
    pub name: Option<String>,
}

pub fn parse_paste(text: &str) -> Result<ParsedPaste, String> {
    let mut credentials: Vec<Credential> = Vec::new();
    let mut links: Vec<VlessCard> = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("qv1:") || line.starts_with("qc1:") {
            credentials.push(parse_credential(line)?);
        } else if line.to_ascii_lowercase().starts_with("vless://") {
            if let Some(link) = parse_vless(line) {
                links.push(link);
            }
        }
    }
    if let Some(first) = credentials.first() {
        if credentials
            .iter()
            .any(|credential| credential.uuid != first.uuid)
        {
            return Err("paste one card at a time".into());
        }
    }
    let credential = credentials
        .into_iter()
        .next()
        .ok_or("paste the qv1 line your provider sent, not just the vless link")?;
    let link = links.into_iter().find(|link| link.uuid == credential.uuid);
    if link.is_none() && text.to_ascii_lowercase().contains("vless://") {
        return Err("the vless link and the credential line are for different cards".into());
    }
    let (sni, name) = match link {
        Some(link) => (
            (!link.sni.is_empty()).then_some(link.sni),
            (!link.name.is_empty()).then_some(link.name),
        ),
        None => (None, None),
    };
    Ok(ParsedPaste {
        credential,
        sni,
        name,
    })
}

/// HMAC-SHA256 over `op\uuid\nts\nnonce`, lowercase hex.
pub fn sign(secret: &[u8], op: &str, uuid: &str, ts: i64, nonce: &str) -> String {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
    let message = format!("{op}\n{uuid}\n{ts}\n{nonce}");
    hex(ring::hmac::sign(&key, message.as_bytes()).as_ref())
}

pub fn nonce() -> String {
    let mut buf = [0u8; 12];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut buf);
    hex(&buf)
}

pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The endpoint for one operation. A single `/v1/register` for everything was
/// the old stub; the server exposes a path per operation.
pub fn path_for(op: &str) -> Result<&'static str, String> {
    match op {
        "register" => Ok("/v1/register"),
        "revoke" => Ok("/v1/revoke"),
        "status" => Ok("/v1/status"),
        other => Err(format!("unknown card operation: {other}")),
    }
}

pub fn url_for(base: &str, op: &str) -> Result<String, String> {
    Ok(format!("{}{}", base.trim_end_matches('/'), path_for(op)?))
}

pub fn request_body(op: &str, uuid: &str, ts: i64, nonce: &str, sig: &str) -> String {
    serde_json::json!({
        "op": op,
        "uuid": uuid,
        "ts": ts,
        "nonce": nonce,
        "sig": sig,
    })
    .to_string()
}

/// Turn the server's answer into the sentence the window shows.
pub fn map_response(code: u16, payload: &str) -> Result<String, String> {
    match code {
        200 => Ok(payload.to_string()),
        400 => Err("the card server did not understand that request".into()),
        401 => Err("the card server rejected the request time; check this PC's clock".into()),
        403 => Err("this device is not authorised to manage that card".into()),
        404 => Err("that card is not registered on the server yet".into()),
        409 => Err("duplicate request refused; try again".into()),
        429 => Err("too many attempts; wait a minute".into()),
        500..=599 => Err("the card server could not apply the change".into()),
        other => Err(format!("the card server answered {other}: {payload}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";

    fn secret_hex() -> String {
        "ab".repeat(32)
    }

    #[test]
    fn uuid_shape_is_enforced() {
        assert_eq!(canonical_uuid(UUID).as_deref(), Some(UUID));
        assert_eq!(
            canonical_uuid("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA").as_deref(),
            Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        );
        assert!(canonical_uuid("11111111111141118111111111111111").is_none());
        assert!(canonical_uuid("11111111-1111-4111-8111-11111111111z").is_none());
        assert!(canonical_uuid("not-a-uuid").is_none());
    }

    #[test]
    fn credential_line_parses_and_rejects_junk() {
        let credential = parse_credential(&format!("qv1:{UUID}:{}", secret_hex())).unwrap();
        assert_eq!(credential.uuid, UUID);
        assert_eq!(credential.secret.len(), 32);
        // The pre-rename prefix still opens existing cards.
        let legacy = parse_credential(&format!("qc1:{UUID}:{}", secret_hex())).unwrap();
        assert_eq!(legacy.uuid, UUID);
        assert!(parse_credential("qv1:bad:00").is_err());
        assert!(parse_credential(&format!("qv1:{UUID}:xy")).is_err());
        assert!(parse_credential("vless://whatever").is_err());
    }

    #[test]
    fn vless_link_gives_sni_and_name() {
        let link = parse_vless(&format!(
            "vless://{UUID}@example.com:443?type=tcp&security=tls&sni=YouTube.com#My%20Card"
        ))
        .unwrap();
        assert_eq!(link.uuid, UUID);
        assert_eq!(link.sni, "youtube.com");
        assert_eq!(link.name, "My Card");
    }

    #[test]
    fn paste_needs_the_credential_line_and_matching_links() {
        let text = format!(
            "qv1:{UUID}:{}\nvless://{UUID}@host:443?sni=ea.com#Gamer",
            secret_hex()
        );
        let parsed = parse_paste(&text).unwrap();
        assert_eq!(parsed.sni.as_deref(), Some("ea.com"));
        assert_eq!(parsed.name.as_deref(), Some("Gamer"));
        assert!(parse_paste("vless://11111111-1111-4111-8111-111111111111@host:443").is_err());
        let other = format!(
            "qv1:{UUID}:{}\nvless://22222222-2222-4222-8222-222222222222@host:443?sni=ea.com",
            secret_hex()
        );
        assert!(parse_paste(&other).is_err());
        let two = format!(
            "qv1:{UUID}:{}\nqc1:22222222-2222-4222-8222-222222222222:{}",
            secret_hex(),
            secret_hex()
        );
        assert!(parse_paste(&two).is_err());
        assert!(parse_vless("😀😀😀").is_none());
    }

    #[test]
    fn signature_matches_the_known_vector() {
        let secret: Vec<u8> = (0u8..32).collect();
        let sig = sign(
            &secret,
            "register",
            UUID,
            1_760_000_000,
            "00112233445566778899aabb",
        );
        assert_eq!(
            sig,
            "97743c34bf4b74c6efc0cca76d1f13695e3c35b3f53fc042134baee211488042"
        );
    }

    #[test]
    fn each_operation_has_its_own_path() {
        assert_eq!(
            url_for(DEFAULT_API_BASE, "register").unwrap(),
            "https://quotavpn.duckdns.org:8444/v1/register"
        );
        assert_eq!(
            url_for(DEFAULT_API_BASE, "status").unwrap(),
            "https://quotavpn.duckdns.org:8444/v1/status"
        );
        assert!(url_for(DEFAULT_API_BASE, "bogus").is_err());
    }

    #[test]
    fn responses_say_what_happened() {
        assert_eq!(map_response(200, "OK").unwrap(), "OK");
        assert!(
            map_response(403, "")
                .unwrap_err()
                .contains("not authorised")
        );
        assert!(
            map_response(404, "")
                .unwrap_err()
                .contains("not registered")
        );
        assert!(map_response(429, "").unwrap_err().contains("too many"));
        assert!(
            map_response(502, "")
                .unwrap_err()
                .contains("could not apply")
        );
    }
}
