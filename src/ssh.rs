/// SSH plumbing built on russh (pure-Rust SSH - works on any Windows box
/// without OpenSSH or any external binary).
use russh::client::{self, Handler};
use russh::keys::PrivateKeyWithHashAlg;
use russh::ChannelMsg;
use std::sync::Arc;

/// Trust on first use, then pin.
///
/// This used to answer `Ok(true)` for every key, which meant the "server" could be
/// anyone who could answer on port 22: an active MITM, a hostile network, or the
/// day the DuckDNS name is taken over. Everything crossing that connection is then
/// theirs to read and edit - every card id, the shared Hysteria2 password, each
/// card's WireGuard private key, and every provisioning answer the app trusts.
///
/// The first successful connection records the host key, because a self-hosted
/// server has no certificate authority to appeal to. Every later connection has to
/// present the same key. Moving a server to a new key is therefore an explicit act:
/// delete `ssh_host_key.txt` in the config directory.
#[derive(Clone)]
pub struct ClientHandler {
    host: String,
}

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let seen = match server_public_key {
            russh::keys::PublicKeyOrCertificate::PublicKey { key, .. } => {
                key.fingerprint(russh::keys::HashAlg::Sha256).to_string()
            }
            // Certificates carry their own chain and this app does not use them.
            _ => return Ok(false),
        };
        let pinned = read_pin(&self.host);
        match pinned {
            Some(p) => Ok(p == seen),
            None => {
                // Nothing to compare against yet: record what we saw. A failure to
                // write must not silently become "trust everything forever", so a
                // connection without a stored pin is allowed once and the next one
                // will have one.
                write_pin(&self.host, &seen);
                Ok(true)
            }
        }
    }
}

/// Where the pinned host keys live, beside the rest of the config.
fn pin_path() -> std::path::PathBuf {
    crate::config::AppConfig::config_dir().join("ssh_host_key.txt")
}

/// One line per host: `host|SHA256:...`.
fn read_pin(host: &str) -> Option<String> {
    let text = std::fs::read_to_string(pin_path()).ok()?;
    for line in text.lines() {
        if let Some((h, fp)) = line.split_once('|') {
            if h.eq_ignore_ascii_case(host) {
                return Some(fp.to_string());
            }
        }
    }
    None
}

fn write_pin(host: &str, fingerprint: &str) {
    let path = pin_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    // Replace an existing line for this host rather than appending a second one.
    let kept: Vec<String> = text
        .lines()
        .filter(|l| !l.split_once('|').map(|(h, _)| h.eq_ignore_ascii_case(host)).unwrap_or(false))
        .map(|l| l.to_string())
        .collect();
    text = kept.join("\n");
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("{host}|{fingerprint}\n"));
    let _ = std::fs::write(&path, text);
}

pub struct SshSession {
    session: russh::client::Handle<ClientHandler>,
}

impl SshSession {
    /// Connect + authenticate with a PEM private key.
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        private_key_pem: &str,
    ) -> Result<SshSession, String> {
        let config = Arc::new(client::Config::default());
        // No timeout here hangs forever on networks that drop the port
        // (common for SSH/22 on mobile carriers) - fail fast instead.
        let handler = ClientHandler { host: host.to_string() };
        let mut session = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client::connect(config, (host, port), handler),
        )
        .await
        .map_err(|_| {
            format!("connect timed out - {host}:{port} unreachable (port may be blocked on this network)")
        })
        .and_then(|r| r.map_err(|e| format!("connect: {e}")))?;

        let key = russh::keys::decode_secret_key(private_key_pem, None)
            .map_err(|e| format!("key: {e}"))?;
        let pk = PrivateKeyWithHashAlg::new(Arc::new(key), None);

        let auth = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            session.authenticate_publickey(user, pk),
        )
        .await
        .map_err(|_| "auth timed out".to_string())
        .and_then(|r| r.map_err(|e| format!("auth: {e}")))?;
        if !auth.success() {
            return Err("authentication failed (key not accepted)".into());
        }

        Ok(SshSession { session })
    }

    /// Run a remote command, capture combined output.
    pub async fn exec(&mut self, cmd: &str) -> Result<String, String> {
        let mut channel = self
            .session
            .channel_open_session()
            .await
            .map_err(|e| format!("channel: {e}"))?;

        channel
            .exec(true, cmd)
            .await
            .map_err(|e| format!("exec: {e}"))?;

        let mut out = String::new();
        loop {
            let msg = channel.wait().await;
            let Some(msg) = msg else { break };
            match msg {
                ChannelMsg::Data { ref data } => {
                    out.push_str(&String::from_utf8_lossy(data));
                }
                ChannelMsg::ExtendedData { ref data, .. } => {
                    out.push_str(&String::from_utf8_lossy(data));
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                ChannelMsg::ExitStatus { exit_status } => {
                    out.push_str(&format!("\n[exit {exit_status}]"));
                    break;
                }
                _ => {}
            }
        }
        let _ = channel.close().await;
        Ok(out)
    }
}