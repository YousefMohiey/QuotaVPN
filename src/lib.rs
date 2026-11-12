//! QuotaCards library surface (used by the app and the smoke example).
#[cfg(not(target_os = "windows"))]
pub mod wincompat;
pub mod config;
#[cfg(target_os = "windows")]
pub mod lan;
#[cfg(target_os = "windows")]
pub mod dhcp_names;
pub mod fingerprint;
#[cfg(target_os = "windows")]
pub mod lan_ask;
#[cfg(target_os = "windows")]
pub mod lan_ctrl;
#[cfg(target_os = "windows")]
pub mod lan_names;
#[cfg(target_os = "windows")]
pub mod wpcap;
pub mod server;
pub mod ssh;
pub mod updater;
// The tunnel engine: Windows drives it through a scheduled task and wintun,
// Linux through pkexec and a real TUN device. Same public surface.
//
// Android is deliberately outside that list. Its tunnel is the Kotlin
// VpnService in tauri-plugin-qctunnel driving the vendored libbox, so nothing
// on the phone calls into this module, and the module is full of helpers that
// only exist under cfg(windows) - which made every Android build fail with six
// "cannot find function" errors. Ungated, that is not a warning: it is the
// reason no APK has been produced since v0.3.8.
#[cfg(any(windows, target_os = "linux"))]
pub mod vpn;

// One LAN device through the tunnel with nothing installed on it. Windows only: it
// works by taking over that device's idea of the gateway, which needs Npcap to send
// the ARP frame and per-interface forwarding to be on. The phone has a real
// VpnService, and Linux has the gateway design in workspace/gateway/.
#[cfg(windows)]
pub mod lan_route;