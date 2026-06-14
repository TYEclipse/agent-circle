//! S09R92-R93 — Plugin registry: discovery, loading, lifecycle.
//!
//! Plugins live as shared libraries in `{data_dir}/plugins/`.
//! Each exports a single `plugin_create()` function returning
//! `Box<dyn AgentPlugin>`.  The registry walks the plugin directory
//! at startup, loads each `.so`/`.dylib`/`.dll`, and drives the
//! lifecycle hooks.

use crate::plugin::{AgentPlugin, PluginId, PluginManifest, PluginResult};
use std::collections::HashMap;
use std::path::Path;
use tracing::{error, info, warn};

type PluginBox = Box<dyn AgentPlugin>;

/// A loaded plugin with its library handle.
struct LoadedPlugin {
    plugin: PluginBox,
    #[allow(dead_code)] // library must stay alive while plugin is loaded
    _lib: libloading::Library,
}

/// Manages plugin discovery, loading, and lifecycle.
pub struct PluginRegistry {
    plugins: HashMap<PluginId, LoadedPlugin>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// List manifests of all loaded plugins.
    pub fn loaded(&self) -> Vec<PluginManifest> {
        self.plugins
            .values()
            .map(|lp| lp.plugin.manifest())
            .collect()
    }
}

impl PluginRegistry {
    /// Discover and load all plugins from `plugin_dir`.
    pub fn discover_and_load(&mut self, plugin_dir: &Path) {
        let Ok(entries) = std::fs::read_dir(plugin_dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !["so", "dylib", "dll"].contains(&ext) {
                continue;
            }
            self.load_one(&path);
        }
    }

    fn load_one(&mut self, path: &Path) {
        let path_display = path.display();
        let lib = match unsafe { libloading::Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                warn!(path = %path_display, %e, "无法加载插件库");
                return;
            }
        };

        type CreateFn = unsafe fn() -> PluginBox;
        let create: libloading::Symbol<CreateFn> = match unsafe { lib.get(b"plugin_create") } {
            Ok(f) => f,
            Err(e) => {
                warn!(path = %path_display, %e, "插件缺少 plugin_create 入口");
                return;
            }
        };

        let mut plugin = unsafe { create() };
        let manifest = plugin.manifest();
        let id = manifest.id.clone();

        if self.plugins.contains_key(&id) {
            warn!(%id, "插件已加载，跳过: {}", path.display());
            return;
        }

        if let Err(e) = plugin.on_load() {
            error!(%e, "插件 on_load 失败");
            return;
        }

        info!(id = %id, name = %manifest.name, version = %manifest.version, "✅ 插件已加载");
        self.plugins.insert(id, LoadedPlugin { plugin, _lib: lib });
    }

    /// Call `on_init` on all loaded plugins.
    #[allow(dead_code)] // wired in daemon loop (future)
    pub fn init_all(&mut self) {
        for lp in self.plugins.values_mut() {
            let id = lp.plugin.manifest().id.clone();
            if let Err(e) = lp.plugin.on_init() {
                error!(%id, %e, "插件 on_init 失败");
            }
        }
    }

    /// Call `on_start` on all loaded plugins.
    #[allow(dead_code)] // wired in daemon loop (future)
    pub fn start_all(&mut self) {
        for lp in self.plugins.values_mut() {
            let id = lp.plugin.manifest().id.clone();
            if let Err(e) = lp.plugin.on_start() {
                error!(%id, %e, "插件 on_start 失败");
            }
        }
    }

    /// Call `on_stop` on all loaded plugins.
    #[allow(dead_code)] // wired in daemon loop (future)
    pub fn stop_all(&mut self) {
        for lp in self.plugins.values_mut() {
            let id = lp.plugin.manifest().id.clone();
            if let Err(e) = lp.plugin.on_stop() {
                error!(%id, %e, "插件 on_stop 失败");
            }
        }
    }

    /// Route a chat message to all plugins.  Returns `true` if any
    /// plugin consumed the message.
    #[allow(dead_code)] // wired in daemon loop (future)
    pub fn route_message(&mut self, peer_id: &str, content: &str) -> PluginResult<bool> {
        let msg = crate::plugin::PluginMessage {
            peer_id: peer_id.to_string(),
            content: content.to_string(),
        };
        for lp in self.plugins.values_mut() {
            if lp.plugin.on_chat_message(&msg)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
