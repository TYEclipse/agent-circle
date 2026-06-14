//! Crash dump handler (S11R118)
//!
//! On panic, writes a structured crash dump file to
//! `{data_dir}/crash/<iso8601>.dump` with:
//!   - timestamp, panic message, backtrace
//!   - system info (OS, hostname, PID)
//!   - agent state snapshot (identity, storage, network)
//!
//! The dump is JSON for easy parsing by diagnostic tooling.

use crate::errors::AcResult;
use std::fs;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Data directory for crash dumps — set at startup.
static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Initialize the crash dump handler. Must be called early in main().
pub fn init(data_dir: PathBuf) -> AcResult<()> {
    let dir = data_dir.join("crash");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| {
            crate::errors::AcError::Io(std::io::Error::other(format!(
                "无法创建 crash 目录 {}: {}",
                dir.display(),
                e
            )))
        })?;
    }
    let _ = CRASH_DIR.set(dir);

    // Install the panic hook — always chain with the default hook
    // so the panic message still prints to stderr.
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // Call default first so the user sees the panic message immediately.
        default_hook(info);

        // Try to write the crash dump.
        if let Some(crash_dir) = CRASH_DIR.get() {
            let _ = write_dump(crash_dir, info);
        }
    }));

    Ok(())
}

/// Write the crash dump file. Returns Ok(path) on success.
fn write_dump(crash_dir: &Path, info: &panic::PanicHookInfo) -> Result<PathBuf, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ts = chrono::DateTime::from_timestamp(now.as_secs() as i64, now.subsec_nanos())
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let filename = format!("{ts}.dump");
    let path = crash_dir.join(&filename);

    // Build the dump
    let info_text = info.to_string();
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
    let backtrace = std::backtrace::Backtrace::force_capture();

    // System info
    let hostname = hostname();
    let os = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    let pid = std::process::id();

    // Agent state snapshot — best-effort
    let agent_state = snapshot_agent_state();

    let dump = serde_json::json!({
        "crash": {
            "timestamp": ts,
            "message": info_text,
            "location": location,
        },
        "system": {
            "os": os,
            "hostname": hostname.unwrap_or_else(|| "unknown".into()),
            "pid": pid,
        },
        "backtrace": format!("{backtrace}"),
        "agent_state": agent_state,
    });

    let json =
        serde_json::to_string_pretty(&dump).map_err(|e| format!("crash dump 序列化失败: {e}"))?;

    fs::write(&path, &json)
        .map_err(|e| format!("crash dump 写入失败 {}: {}", path.display(), e))?;

    // Also write a plain-text symlink-friendly "latest.dump"
    let latest = crash_dir.join("latest.dump");
    let _ = fs::write(&latest, &json);
    let _ = fs::write(
        crash_dir.join("latest.txt"),
        format!("latest = {filename}\n"),
    );

    Ok(path)
}

/// Best-effort snapshot of agent state at crash time.
fn snapshot_agent_state() -> serde_json::Value {
    let data_dir = crate::DATA_DIR.get();
    let mut state = serde_json::json!({});

    if let Some(data_dir) = data_dir {
        // Identity
        state["identity"] = serde_json::json!({
            "exists": data_dir.join("identity.key").exists(),
        });

        // Card
        if data_dir.join("card.json").exists() {
            if let Ok(Some(card)) = crate::storage::load_card(Some(data_dir)) {
                state["card"] = serde_json::json!({
                    "name": card.name,
                    "owner": card.owner,
                    "model": card.model,
                    "status": card.status,
                });
            }
        }

        // Contacts
        if data_dir.join("contacts.json").exists() {
            if let Ok(contacts) = crate::storage::load_contacts(Some(data_dir)) {
                state["contacts"] = serde_json::json!({
                    "count": contacts.len(),
                    "names": contacts.iter().map(|c| &c.name).collect::<Vec<_>>(),
                });
            }
        }

        // Timeline
        if data_dir.join("timeline.json").exists() {
            if let Ok(tl) = crate::storage::load_timeline(Some(data_dir)) {
                state["timeline"] = serde_json::json!({
                    "posts": tl.len(),
                    "verified": tl.verify().is_ok(),
                });
            }
        }

        // Services
        if data_dir.join("services.json").exists() {
            if let Ok(r) = crate::service_discovery::load_registry(data_dir) {
                state["services"] = serde_json::json!({
                    "peers": r.peer_count(),
                    "services": r.service_count(),
                });
            }
        }

        // Offline queue
        if let Ok(q) = crate::message_queue::Queue::open(data_dir) {
            if let Ok((pending, delivered, failed)) = q.stats() {
                state["queue"] = serde_json::json!({
                    "pending": pending,
                    "delivered": delivered,
                    "failed": failed,
                });
            }
        }
    }

    state
}

fn hostname() -> Option<String> {
    // Cross-platform hostname detection
    if let Ok(host) = std::env::var("HOSTNAME") {
        if !host.is_empty() {
            return Some(host);
        }
    }
    if let Ok(host) = std::env::var("COMPUTERNAME") {
        if !host.is_empty() {
            return Some(host);
        }
    }
    // Fallback: try nix hostname via std::process
    if let Ok(output) = std::process::Command::new("hostname").output() {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_dir_creation() {
        let tmp = std::env::temp_dir().join(format!("ac_crash_test_{}", std::process::id()));
        // Clean up first
        let _ = fs::remove_dir_all(&tmp);
        assert!(init(tmp.clone()).is_ok());
        let crash_dir = tmp.join("crash");
        assert!(crash_dir.exists());
        // Clean up
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_double_init_does_not_panic() {
        let tmp = std::env::temp_dir().join(format!("ac_crash_test2_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        assert!(init(tmp.clone()).is_ok());
        // Second init just updates CRASH_DIR — should not error
        assert!(init(tmp.clone()).is_ok());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_agent_state_snapshot_empty() {
        // When DATA_DIR is unset, snapshot returns empty object
        let state = snapshot_agent_state();
        assert!(state.as_object().map(|o| o.is_empty()).unwrap_or(false));
    }
}
