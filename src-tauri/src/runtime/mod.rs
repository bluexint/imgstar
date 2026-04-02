//! Runtime layer entry point.
//!
//! This module wires together the long-lived runtime services used by the
//! Tauri backend. `adapter_runtime` now lives in a same-name folder and is
//! further split into upload, Cloudflare, and WAF submodules.

pub mod adapter_runtime;
pub mod event_bus;
pub mod plugin_runtime;
