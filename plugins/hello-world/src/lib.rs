//! S09R98 — hello-world: the canonical Agent Circle plugin example.
//!
//! Listens for chat messages containing "hello" or "你好" and logs a
//! greeting.  Also demonstrates `declare_plugin!` macro usage.
//!
//! # Build
//!
//! ```bash
//! cargo build -p hello-world --release
//! ```
//!
//! # Install
//!
//! ```bash
//! cp target/release/libhello_world.so ~/.agent-circle/plugins/
//! ```

use agent_circle_plugin::*;

/// A plugin that listens for "hello" greetings.
#[derive(Default)]
struct HelloPlugin {
    /// Number of greetings received since `on_start`.
    greeted: u64,
}

impl AgentPlugin for HelloPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: PluginId::new("hello-world"),
            name: "Hello World".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            author: "Agent Circle Team".into(),
            description: "示例插件：收到 'hello' 时打招呼".into(),
        }
    }

    fn on_init(&mut self) -> PluginResult {
        tracing::info!("hello-world plugin initialised");
        Ok(())
    }

    fn on_start(&mut self) -> PluginResult {
        tracing::info!("hello-world plugin started — listening for greetings");
        self.greeted = 0;
        Ok(())
    }

    fn on_chat_message(&mut self, msg: &PluginMessage) -> PluginResult<bool> {
        let content = msg.content.to_lowercase();
        // Match both English and Chinese greetings
        if content.contains("hello") || content.contains("你好") {
            self.greeted += 1;
            tracing::info!(
                greet_count = self.greeted,
                from = %msg.peer_id,
                "hello-world plugin: greeting detected"
            );
            Ok(true) // consumed — no further processing
        } else {
            Ok(false) // pass to next handler
        }
    }

    fn on_stop(&mut self) -> PluginResult {
        tracing::info!(
            greet_count = self.greeted,
            "hello-world plugin stopping — {} greetings processed",
            self.greeted
        );
        Ok(())
    }

    fn cli_subcommands(&self) -> Vec<(String, String)> {
        vec![(
            "hello".into(),
            "Show hello-world plugin greeting count".into(),
        )]
    }
}

declare_plugin!(HelloPlugin);
