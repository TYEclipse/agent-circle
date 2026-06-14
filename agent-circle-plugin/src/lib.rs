//! # Agent Circle Plugin SDK
//!
//! S09R97 — The official SDK for building plugins that extend
//! Agent Circle with custom behaviours, protocols, and CLI commands.
//!
//! ## Quick start
//!
//! ```bash
//! cargo new --lib my-plugin
//! cd my-plugin
//! ```
//!
//! `Cargo.toml`:
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! agent-circle-plugin = "0.1"
//! ```
//!
//! `src/lib.rs`:
//! ```ignore
//! use agent_circle_plugin::*;
//! use std::fmt;
//!
//! struct HelloPlugin;
//!
//! impl AgentPlugin for HelloPlugin {
//!     fn manifest(&self) -> PluginManifest {
//!         PluginManifest {
//!             id: PluginId::new("hello"),
//!             name: "Hello World".into(),
//!             version: "0.1.0".into(),
//!             author: "dev".into(),
//!             description: "Says hello".into(),
//!         }
//!     }
//!
//!     fn on_chat_message(&mut self, msg: &PluginMessage) -> PluginResult<bool> {
//!         if msg.content.contains("hello") {
//!             tracing::info!("Hello from plugin!");
//!             return Ok(true); // consumed
//!         }
//!         Ok(false)
//!     }
//! }
//!
//! declare_plugin!(HelloPlugin);
//! ```
//!
//! Build:  `cargo build --release`
//! Output: `target/release/libmy_plugin.so`
//!
//! ## Loading
//!
//! Drop the `.so` into `~/.agent-circle/plugins/` and restart the
//! daemon.  The plugin registry scans this directory on startup and
//! calls `on_init` → `on_start`.
//!
//! ## Reference
//!
//! - [`AgentPlugin`] — the trait your plugin must implement
//! - [`declare_plugin!`] — macro to export the C-ABI entry point
//! - [`PluginManifest`], [`PluginMessage`], [`PluginError`] — supporting types

// ── Re-exports ────────────────────────────────────────────────────────
pub use agent_circle_core::plugin::{
    AgentPlugin, PluginError, PluginErrorKind, PluginId, PluginManifest, PluginMessage,
    PluginResult,
};

// ── Plugin entry-point macro ──────────────────────────────────────────
/// Generate the `#[no_mangle] extern "C" fn plugin_create()` entry point
/// for a plugin type that implements [`AgentPlugin`] + [`Default`].
///
/// # Example
///
/// ```ignore
/// use agent_circle_plugin::*;
///
/// #[derive(Default)]
/// struct MyPlugin;
///
/// impl AgentPlugin for MyPlugin {
///     fn manifest(&self) -> PluginManifest { /* ... */ }
/// }
///
/// declare_plugin!(MyPlugin);
/// ```
///
/// The expanded code:
/// ```ignore
/// #[no_mangle]
/// pub extern "C" fn plugin_create() -> *mut dyn AgentPlugin {
///     let plugin: Box<dyn AgentPlugin> = Box::<MyPlugin>::default();
///     Box::into_raw(plugin)
/// }
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin:ty) => {
        #[no_mangle]
        pub extern "C" fn plugin_create() -> *mut dyn $crate::AgentPlugin {
            let plugin: ::std::boxed::Box<dyn $crate::AgentPlugin> =
                ::std::boxed::Box::<$plugin>::default();
            ::std::boxed::Box::into_raw(plugin)
        }
    };
}

// ── Plugin writing guide (doc-only) ───────────────────────────────────
// KEPT as module documentation above; no runtime code needed in the SDK.
