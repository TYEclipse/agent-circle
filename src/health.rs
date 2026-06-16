//! Local HTTP health + metrics server
//!
//! Runs on `127.0.0.1:9099` alongside the daemon.
//! No external dependencies — raw HTTP/1.1 on tokio::net::TcpListener.
//!
//! Endpoints:
//!   GET /health  → JSON health check
//!   GET /metrics → OpenMetrics / Prometheus text format

use crate::errors::AcResult;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn the health/metrics HTTP server on 127.0.0.1:9099.
/// Returns the bound address so the daemon can log it.
pub async fn spawn(data_dir: PathBuf, peer_id: String) -> AcResult<SocketAddr> {
    let addr: SocketAddr = ([127, 0, 0, 1], 9099).into();
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        crate::errors::AcError::Network(format!("health server bind 127.0.0.1:9099 失败: {e}"))
    })?;

    let bound = listener.local_addr().map_err(|e| {
        crate::errors::AcError::Network(format!("health server local_addr 失败: {e}"))
    })?;

    let data_dir_clone = data_dir.clone();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    let data_dir = data_dir_clone.clone();
                    let peer_id = peer_id.clone();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        let n = match stream.read(&mut buf).await {
                            Ok(0) => return,
                            Ok(n) => n,
                            Err(_) => return,
                        };

                        let request = String::from_utf8_lossy(&buf[..n]);
                        let first_line = request.lines().next().unwrap_or("");
                        let parts: Vec<&str> = first_line.split_whitespace().collect();

                        let (status, content_type, body) = if parts.len() >= 2 {
                            match parts[1] {
                                "/health" => handle_health(&data_dir, &peer_id),
                                "/metrics" => handle_metrics(&data_dir),
                                _ => {
                                    let body = r#"{"error":"not found"}"#;
                                    ("404 Not Found", "application/json", body.to_string())
                                }
                            }
                        } else {
                            ("400 Bad Request", "text/plain", "Bad Request".to_string())
                        };

                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\n\r\n{body}",
                            len = body.len()
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.shutdown().await;
                    });
                }
                Err(_) => {
                    // Accept error; continue listening
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    });

    Ok(bound)
}

/// GET /health — quick JSON status
fn handle_health(data_dir: &Path, peer_id: &str) -> (&'static str, &'static str, String) {
    use crate::service_discovery;

    let control_port = data_dir.join("control.port");
    let daemon_running = control_port.exists();

    // Identity check
    let identity_ok = data_dir.join("identity.key").exists();

    // Storage check
    let storage_ok = data_dir.exists();

    // Network check
    let network_ok = daemon_running;

    let (peer_count, svc_count) = match service_discovery::load_registry(data_dir) {
        Ok(r) => (r.peer_count(), r.service_count()),
        Err(_) => (0, 0),
    };

    let status = if daemon_running && identity_ok && storage_ok {
        "ok"
    } else {
        "degraded"
    };

    let body = serde_json::json!({
        "status": status,
        "daemon": if daemon_running { "running" } else { "stopped" },
        "peer_id": peer_id,
        "checks": {
            "identity": identity_ok,
            "storage": storage_ok,
            "network": network_ok,
        },
        "stats": {
            "peers": peer_count,
            "services": svc_count,
        }
    })
    .to_string();

    ("200 OK", "application/json", body)
}

/// GET /metrics — OpenMetrics text (same as CLI, but from live daemon state)
fn handle_metrics(data_dir: &Path) -> (&'static str, &'static str, String) {
    match crate::metrics::collect_for_dir(data_dir) {
        Ok(text) => ("200 OK", "text/plain; version=0.0.4", text),
        Err(e) => {
            let body = format!("# Error collecting metrics: {e}\n# EOF\n");
            ("500 Internal Server Error", "text/plain", body)
        }
    }
}
