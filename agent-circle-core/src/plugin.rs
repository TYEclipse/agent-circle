//! S09R91 — Plugin trait definition.
//!
//! Every Agent Circle plugin implements [`AgentPlugin`] and exports it
//! via a C-ABI entry point (`plugin_create`).
//!
//! ## Lifecycle
//!
//!   load → init → start → (running) → stop → unload
//!
//! | Hook | When | Typical use |
//! |------|------|-------------|
//! | `on_load` | Plugin discovered | Validate environment, register types |
//! | `on_init` | Daemon starting | Open connections, allocate resources |
//! | `on_start` | Daemon running | Activate behaviour, subscribe to events |
//! | `on_stop` | Daemon stopping | Graceful shutdown, persist state |
//! | `on_unload` | Plugin removed | Release all resources |

use std::fmt;

/// Unique identifier for a loaded plugin instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(String);

impl PluginId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata describing a plugin.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

/// Return type for plugin lifecycle hooks.
pub type PluginResult<T = ()> = Result<T, PluginError>;

/// Lightweight error type for plugin operations.
#[derive(Debug)]
pub struct PluginError {
    pub plugin_id: PluginId,
    pub kind: PluginErrorKind,
    pub message: String,
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "plugin {} {:?}: {}",
            self.plugin_id.as_str(),
            self.kind,
            self.message
        )
    }
}

impl std::error::Error for PluginError {}

#[derive(Debug)]
pub enum PluginErrorKind {
    Load,
    Init,
    Start,
    Stop,
    Unload,
    #[allow(dead_code)]
    Config,
}

/// A chat message received from a peer.
#[derive(Debug, Clone)]
pub struct PluginMessage {
    pub peer_id: String,
    pub content: String,
}

/// The core plugin trait.
///
/// Plugins implement this trait and export a single `plugin_create()`
/// function via `#[no_mangle] pub extern "C"`.
pub trait AgentPlugin: Send + Sync {
    /// Metadata — called once after loading.
    fn manifest(&self) -> PluginManifest;

    /// Called when the plugin binary is loaded into memory.
    fn on_load(&mut self) -> PluginResult {
        Ok(())
    }

    /// Called when the daemon starts initialising.
    fn on_init(&mut self) -> PluginResult {
        Ok(())
    }

    /// Called when the daemon is fully running and accepting connections.
    fn on_start(&mut self) -> PluginResult {
        Ok(())
    }

    /// Called when the daemon begins a graceful shutdown.
    fn on_stop(&mut self) -> PluginResult {
        Ok(())
    }

    /// Called when the plugin is unloaded (libraries dropped).
    fn on_unload(&mut self) -> PluginResult {
        Ok(())
    }

    /// Handle an incoming chat message.  Return `true` if the message
    /// was consumed (no further processing by other plugins or the
    /// core message handler).
    fn on_chat_message(&mut self, _msg: &PluginMessage) -> PluginResult<bool> {
        Ok(false)
    }

    /// Return extra CLI subcommands this plugin exposes.
    /// Each entry is `("command-name", "description")`.
    fn cli_subcommands(&self) -> Vec<(String, String)> {
        vec![]
    }
}
