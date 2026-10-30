//! QuotaCards library surface (used by the app and the smoke example).
pub mod config;
pub mod lan;
pub mod lan_ctrl;
pub mod lan_names;
#[cfg(target_os = "windows")]
pub mod wpcap;
pub mod server;
pub mod ssh;
pub mod updater;
// TUN engine control is Windows-only (wintun/taskkill/routes). Mobile
// builds reuse config/server/ssh and drive their own platform tunnel.
#[cfg(target_os = "windows")]
pub mod vpn;