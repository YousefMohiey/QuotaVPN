//! QuotaCards library surface (used by the app and the smoke example).
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
pub mod vpn;