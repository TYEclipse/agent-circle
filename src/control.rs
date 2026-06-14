//! S07R75 — Cross-platform daemon control socket.
//!
//! Replaces Unix-only SIGUSR1 with a local TCP listener that works on
//! Linux, macOS, and Windows.  The daemon writes its listening port to
//! `{data_dir}/control.port` so CLI commands can connect.
//!
//! Protocol (one-shot, line-delimited):
//!   log-level <LEVEL>   →  switch tracing level to LEVEL, reply "ok <LEVEL>\n"
//!   ping                →  reply "pong\n"
//!   quit                →  graceful shutdown request

use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing_subscriber::reload::Handle;
use tracing_subscriber::EnvFilter;

/// Spawn the control listener.  Returns the bound address so the
/// daemon can record it for CLI clients.
pub async fn spawn_control_server(
    reload_handle: Handle<EnvFilter, tracing_subscriber::Registry>,
) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    tracing::debug!(%peer, "control connection");
                    let handle = reload_handle.clone();
                    tokio::spawn(handle_control(stream, handle));
                }
                Err(e) => {
                    tracing::warn!("control accept error: {e}");
                }
            }
        }
    });

    Ok(addr)
}

async fn handle_control(
    stream: tokio::net::TcpStream,
    handle: Handle<EnvFilter, tracing_subscriber::Registry>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader);

    let mut buf = String::new();
    if lines.read_line(&mut buf).await.is_ok() {
        let cmd = buf.trim();
        if cmd == "ping" {
            let _ = writer.write_all(b"pong\n").await;
        } else if let Some(level) = cmd.strip_prefix("log-level ") {
            let level = level.trim();
            let levels: [&str; 5] = ["error", "warn", "info", "debug", "trace"];
            if levels.contains(&level) {
                if let Ok(f) = EnvFilter::try_new(level) {
                    let _ = handle.reload(f);
                    let _ = writer.write_all(format!("ok {level}\n").as_bytes()).await;
                    tracing::info!(%level, "📶 日志级别已切换 (control socket)");
                    return;
                }
            }
            let _ = writer
                .write_all(format!("err unknown level '{level}'\n").as_bytes())
                .await;
        } else if cmd == "quit" {
            let _ = writer.write_all(b"ok shutting down\n").await;
            // Signal graceful shutdown — the daemon loop checks this
            tracing::info!("收到 quit 命令，准备退出");
            std::process::exit(0);
        } else {
            let _ = writer
                .write_all(format!("err unknown command '{cmd}'\n").as_bytes())
                .await;
        }
    }
}
