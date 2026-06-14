//! Prometheus / OpenMetrics exporter (S11R116)
//!
//! `agent-circle metrics` outputs OpenMetrics text format consumable by
//! Prometheus, VictoriaMetrics, and other OpenMetrics-compatible scrapers.
//!
//! Metrics are gathered from local filesystem state (always available)
//! and enriched with daemon-side counters when the daemon is online.

use crate::errors::AcResult;
use crate::service_discovery;
use crate::storage;
use std::fmt::Write;

/// Collect all available metrics and format as OpenMetrics text.
pub fn collect() -> AcResult<String> {
    let data_dir = storage::resolve_data_dir(None::<&std::path::PathBuf>)?;
    let mut out = String::new();

    // ── HELP/TYPE metadata ───────────────────────────────────────
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0");

    emit_gauge(&mut out, "agent_circle_info", "Version and host info", 1.0);
    // Append version as a label-like comment since OpenMetrics labels
    // require static key names — we use a separate info metric instead.
    writeln!(out, "# agent_circle_version{{version=\"{version}\"}} 1").ok();

    // ── Daemon status ────────────────────────────────────────────
    let sock = data_dir.join("control.sock");
    let daemon_up = if sock.exists() { 1.0 } else { 0.0 };

    emit_gauge(
        &mut out,
        "agent_circle_daemon_up",
        "Whether the daemon is running (1=up, 0=down)",
        daemon_up,
    );

    // ── Storage ──────────────────────────────────────────────────
    if data_dir.exists() {
        if let Ok(size) = dir_size(&data_dir) {
            emit_gauge(
                &mut out,
                "agent_circle_storage_size_bytes",
                "Total size of the data directory in bytes",
                size as f64,
            );
        }

        // Storage files presence
        for (file, metric) in &[
            ("card.json", "agent_circle_storage_card_present"),
            ("identity.key", "agent_circle_storage_identity_present"),
            ("contacts.json", "agent_circle_storage_contacts_present"),
            ("timeline.json", "agent_circle_storage_timeline_present"),
            ("services.json", "agent_circle_storage_services_present"),
        ] {
            let present = if data_dir.join(file).exists() {
                1.0
            } else {
                0.0
            };
            emit_gauge(
                &mut out,
                metric,
                &format!("Whether {file} exists (1=yes)"),
                present,
            );
        }
    }

    // ── Contacts ─────────────────────────────────────────────────
    match storage::load_contacts(data_dir_opt()) {
        Ok(contacts) => {
            emit_gauge(
                &mut out,
                "agent_circle_contacts_count",
                "Number of saved contacts",
                contacts.len() as f64,
            );
        }
        Err(_) => {
            emit_gauge(
                &mut out,
                "agent_circle_contacts_count",
                "Number of saved contacts",
                -1.0,
            );
        }
    }

    // ── Timeline ─────────────────────────────────────────────────
    if data_dir.join("timeline.json").exists() {
        match storage::load_timeline(data_dir_opt()) {
            Ok(tl) => {
                emit_gauge(
                    &mut out,
                    "agent_circle_timeline_posts",
                    "Number of timeline posts",
                    tl.len() as f64,
                );
                let verified = if tl.verify().is_ok() { 1.0 } else { 0.0 };
                emit_gauge(
                    &mut out,
                    "agent_circle_timeline_verified",
                    "Whether timeline passes Merkle-DAG verification (1=yes)",
                    verified,
                );
            }
            Err(_) => {
                emit_gauge(
                    &mut out,
                    "agent_circle_timeline_posts",
                    "Number of timeline posts",
                    -1.0,
                );
            }
        }
    }

    // ── Services ─────────────────────────────────────────────────
    if data_dir.join("services.json").exists() {
        match service_discovery::load_registry(&data_dir) {
            Ok(r) => {
                emit_gauge(
                    &mut out,
                    "agent_circle_services_peers",
                    "Number of peers in service registry",
                    r.peer_count() as f64,
                );
                emit_gauge(
                    &mut out,
                    "agent_circle_services_total",
                    "Total number of services discovered",
                    r.service_count() as f64,
                );
            }
            Err(_) => {
                emit_gauge(
                    &mut out,
                    "agent_circle_services_peers",
                    "Number of peers in service registry",
                    -1.0,
                );
            }
        }
    }

    // ── Offline message queue ────────────────────────────────────
    if data_dir.exists() {
        if let Ok(q) = crate::message_queue::Queue::open(&data_dir) {
            if let Ok((pending, delivered, failed)) = q.stats() {
                emit_gauge(
                    &mut out,
                    "agent_circle_queue_pending",
                    "Pending offline messages",
                    pending as f64,
                );
                emit_gauge(
                    &mut out,
                    "agent_circle_queue_delivered",
                    "Delivered offline messages",
                    delivered as f64,
                );
                emit_gauge(
                    &mut out,
                    "agent_circle_queue_failed",
                    "Failed offline messages (>3 retries)",
                    failed as f64,
                );
                let total = pending + delivered + failed;
                if total > 0 {
                    let rate = (delivered as f64 / total as f64) * 100.0;
                    emit_gauge(
                        &mut out,
                        "agent_circle_queue_delivery_rate",
                        "Delivery rate percentage",
                        rate,
                    );
                }
            }
        }
    }

    // ── EOF (required by OpenMetrics spec) ───────────────────────
    out.push_str("# EOF\n");
    Ok(out)
}

/// Recursive directory size.
fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total += dir_size(&entry.path())?;
        } else {
            total += meta.len();
        }
    }
    Ok(total)
}

/// Emit a single gauge metric in OpenMetrics text format.
fn emit_gauge(out: &mut String, name: &str, help: &str, value: f64) {
    use std::fmt::Write;
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    if value == (value as i64) as f64 && value.fract() == 0.0 {
        let _ = writeln!(out, "{name} {}", value as i64);
    } else {
        let _ = writeln!(out, "{name} {value:.6}");
    }
}

fn data_dir_opt() -> Option<&'static std::path::PathBuf> {
    crate::DATA_DIR.get()
}
