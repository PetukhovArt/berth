pub mod agent_sessions;
#[cfg(feature = "desktop")]
mod app_menu;
pub mod berth;
mod changes;
mod cleanup;
mod commands;
pub mod connectors;
mod daemon_start;
#[cfg(feature = "desktop")]
mod desktop;
pub mod files;
pub mod legacy;
mod mr_checkout;
pub mod platform;
pub mod remote;
mod review;
pub mod rpc;
pub mod settings;
pub mod sink;
pub mod stratum;
#[cfg(feature = "desktop")]
mod taskbar;
#[cfg(feature = "desktop")]
mod tray;
#[cfg(feature = "desktop")]
mod working_copy;

#[cfg(feature = "desktop")]
pub const IDENTIFIER: &str = "dev.berth.desktop";

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    desktop::run();
}
