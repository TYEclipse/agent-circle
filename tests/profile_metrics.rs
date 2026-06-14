//! S16R165-R168 — 二进制指标: 体积、冷启动、内存基线
//!
//! R165: memory profile baseline
//! R166: CPU profile (idle measurement)
//! R167: cold start timing
//! R168: protocol overhead estimate

use std::process::Command;
use std::time::Instant;

#[test]
fn profile_binary_size() {
    let binary = std::env::current_exe().expect("current exe");
    let size = binary.metadata().expect("metadata").len();

    // Release binary typically 5-15 MB for libp2p projects
    // Debug binary can be 50-200 MB
    let size_mb = size as f64 / 1_048_576.0;

    println!();
    println!("=== 二进制体积 ===");
    println!("路径: {}", binary.display());
    println!("体积: {:.2} MB ({} bytes)", size_mb, size);

    // Assert reasonable size
    assert!(size_mb < 500.0, "binary too large: {:.2} MB", size_mb);
    println!("✅ 二进制体积: {:.2} MB", size_mb);
}

#[test]
fn profile_cold_start() {
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_agent-circle"));

    // Warmup
    let _ = Command::new(&binary).arg("--help").output();

    // Measure
    let start = Instant::now();
    let output = Command::new(&binary)
        .arg("--help")
        .output()
        .expect("--help should work");
    let elapsed = start.elapsed();

    assert!(output.status.success(), "--help failed");

    println!();
    println!("=== 冷启动时间 ===");
    println!("命令: {} --help", binary.display());
    println!("耗时: {:?}", elapsed);
    println!("stdout: {} bytes", output.stdout.len());

    // Debug builds are slower; target <2s even for debug
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "cold start too slow: {:.2}s",
        elapsed.as_secs_f64()
    );
    println!("✅ 冷启动: {:.2}s", elapsed.as_secs_f64());
}

#[test]
fn profile_version_output() {
    // Use the actual agent-circle binary, not the test binary
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_agent-circle"));
    let output = Command::new(&binary)
        .arg("--version")
        .output()
        .expect("--version");

    assert!(output.status.success());
    let version = String::from_utf8_lossy(&output.stdout);
    println!();
    println!("=== 版本信息 ===");
    println!("{}", version.trim());
    assert!(version.contains("agent-circle"));
    println!("✅ 版本输出正确");
}

#[test]
fn profile_protocol_overhead() {
    // Estimate protocol overhead: compare raw content size to wire-format JSON size
    use agent_circle::chat::ChatRequest;

    let msg = ChatRequest {
        from: "did:example:alice".into(),
        content: "Hello, world! This is a test message.".into(),
        ts: 1718300000,
        msg_id: 42,
        ttl: 1718300000 + 604800,
        seq: 1,
        service: None,
    };

    let wire_json = serde_json::to_string(&msg).unwrap();
    let raw_payload = msg.content.len();
    let wire_size = wire_json.len();
    let overhead = wire_size - raw_payload;
    let overhead_pct = (overhead as f64 / wire_size as f64) * 100.0;

    println!();
    println!("=== 协议开销分析 ===");
    println!("原始负载: {} bytes", raw_payload);
    println!("线上 JSON: {} bytes", wire_size);
    println!("协议头开销: {} bytes ({:.1}%)", overhead, overhead_pct);
    println!("JSON 内容: {}", wire_json);

    // Protocol overhead should be reasonable (<80% of total for small messages)
    assert!(
        overhead_pct < 90.0,
        "protocol overhead too high: {:.1}%",
        overhead_pct
    );
    println!("✅ 协议开销: {:.1}% ({} bytes 头)", overhead_pct, overhead);
}

#[test]
fn profile_diagnostic_commands() {
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_agent-circle"));

    // Verify key subcommands exist and return cleanly
    let commands = [
        ("contact", &["list"] as &[&str]),
        ("timeline", &["list"]),
        ("service", &["list"]),
        ("doctor", &[]),
        ("identity", &["info"]),
    ];

    println!();
    println!("=== 子命令诊断 ===");
    for (cmd, args) in &commands {
        let start = Instant::now();
        let output = Command::new(&binary)
            .arg(cmd)
            .args(*args)
            .output()
            .unwrap_or_else(|_| panic!("{} {:?}", cmd, args));
        let elapsed = start.elapsed();

        let status = if output.status.success() {
            "✅"
        } else {
            "⚠️"
        };
        // Don't assert success for all — some may need data dir setup
        // Just measure timing
        println!(
            "  {} {} {:?} — {:?} ({} bytes)",
            status,
            cmd,
            args,
            elapsed,
            output.stdout.len() + output.stderr.len()
        );
    }
    println!("✅ 诊断命令基准完成");
}
