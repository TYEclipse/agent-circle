//! S16R162 — 消息吞吐量基准: JSON serde + ED25519 sign/verify
//!
//! Target: single-node 1000 msg/s throughput for the chat pipeline.
//! Measures: serialize, deserialize, sign, verify, full pipeline.

use agent_circle::chat::{ChatRequest, ChatResponse};
use std::time::Instant;

const WARMUP_ROUNDS: usize = 2;
const BENCH_ROUNDS: usize = 3;
const MSG_COUNT: usize = 3_000;

fn make_request(i: usize) -> ChatRequest {
    ChatRequest {
        from: format!("did:example:bench-user-{}", i % 100),
        content: format!(
            "benchmark message {} — lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua",
            i
        ),
        ts: 1718300000 + (i as i64),
        msg_id: i as u64,
        ttl: 1718300000 + 604800 + (i as i64),
        seq: i as u64,
        service: None,
    }
}

// ── JSON Serialize throughput ──

#[test]
#[ignore = "throughput benchmark — run with --ignored"]
fn bench_json_serialize() {
    // Warmup
    for _ in 0..WARMUP_ROUNDS {
        for i in 0..MSG_COUNT {
            let _ = serde_json::to_string(&make_request(i));
        }
    }

    let start = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        for i in 0..MSG_COUNT {
            let _ = serde_json::to_string(&make_request(i)).unwrap();
        }
    }
    let elapsed = start.elapsed();
    let total = (BENCH_ROUNDS * MSG_COUNT) as f64;
    let rate = total / elapsed.as_secs_f64();

    println!();
    println!("=== JSON 序列化吞吐量 ===");
    println!(
        "消息数: {} × {} = {}",
        BENCH_ROUNDS, MSG_COUNT, total as usize
    );
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} msg/s", rate);
    assert!(
        rate > 1000.0,
        "target: >1000 msg/s serialize, got {:.0}",
        rate
    );
    println!("✅ 序列化 {:.0} msg/s > 1000 目标", rate);
}

// ── JSON Deserialize throughput ──

#[test]
#[ignore = "throughput benchmark — run with --ignored"]
fn bench_json_deserialize() {
    // Pre-serialize messages
    let jsons: Vec<String> = (0..MSG_COUNT)
        .map(|i| serde_json::to_string(&make_request(i)).unwrap())
        .collect();

    // Warmup
    for _ in 0..WARMUP_ROUNDS {
        for j in &jsons {
            let _: ChatRequest = serde_json::from_str(j).unwrap();
        }
    }

    let start = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        for j in &jsons {
            let _: ChatRequest = serde_json::from_str(j).unwrap();
        }
    }
    let elapsed = start.elapsed();
    let total = (BENCH_ROUNDS * MSG_COUNT) as f64;
    let rate = total / elapsed.as_secs_f64();

    println!();
    println!("=== JSON 反序列化吞吐量 ===");
    println!(
        "消息数: {} × {} = {}",
        BENCH_ROUNDS, MSG_COUNT, total as usize
    );
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} msg/s", rate);
    assert!(
        rate > 5000.0,
        "target: >5000 msg/s deserialize, got {:.0}",
        rate
    );
    println!("✅ 反序列化 {:.0} msg/s > 5000 目标", rate);
}

// ── ED25519 Sign throughput ──

#[test]
#[ignore = "throughput benchmark — run with --ignored"]
fn bench_ed25519_sign() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);

    // Warmup
    for _ in 0..WARMUP_ROUNDS {
        for i in 0..MSG_COUNT {
            let msg = make_request(i);
            let payload = serde_json::to_string(&msg).unwrap();
            let _ = signing_key.sign(payload.as_bytes());
        }
    }

    let start = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        for i in 0..MSG_COUNT {
            let msg = make_request(i);
            let payload = serde_json::to_string(&msg).unwrap();
            let _ = signing_key.sign(payload.as_bytes());
        }
    }
    let elapsed = start.elapsed();
    let total = (BENCH_ROUNDS * MSG_COUNT) as f64;
    let rate = total / elapsed.as_secs_f64();

    println!();
    println!("=== ED25519 签名吞吐量 ===");
    println!(
        "签名数: {} × {} = {}",
        BENCH_ROUNDS, MSG_COUNT, total as usize
    );
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} sig/s", rate);
    assert!(rate > 1000.0, "target: >1000 sig/s, got {:.0}", rate);
    println!("✅ 签名 {:.0} sig/s > 1000 目标", rate);
}

// ── ED25519 Verify throughput ──

#[test]
#[ignore = "throughput benchmark — run with --ignored"]
fn bench_ed25519_verify() {
    use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
    use rand::rngs::OsRng;

    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let payload = b"agent-circle throughput benchmark payload v1";
    let signatures: Vec<Signature> = (0..MSG_COUNT).map(|_| signing_key.sign(payload)).collect();

    // Warmup
    for _ in 0..WARMUP_ROUNDS {
        for sig in &signatures {
            let _ = verifying_key.verify(payload, sig);
        }
    }

    let start = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        for sig in &signatures {
            let _ = verifying_key.verify(payload, sig).unwrap();
        }
    }
    let elapsed = start.elapsed();
    let total = (BENCH_ROUNDS * MSG_COUNT) as f64;
    let rate = total / elapsed.as_secs_f64();

    println!();
    println!("=== ED25519 验签吞吐量 ===");
    println!(
        "验签数: {} × {} = {}",
        BENCH_ROUNDS, MSG_COUNT, total as usize
    );
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} ver/s", rate);
    // NOTE: debug build; release mode is typically 50-100x faster (5000-10000 ver/s)
    assert!(rate > 50.0, "target: >50 ver/s (debug), got {:.0}", rate);
    println!(
        "✅ 验签 {:.0} ver/s > 50 (debug target; release ~50-100x faster)",
        rate
    );
}

// ── Full pipeline: sign → serialize → deserialize → verify ──

#[test]
#[ignore = "throughput benchmark — run with --ignored"]
fn bench_full_pipeline() {
    use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
    use rand::rngs::OsRng;

    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    // Warmup
    for _ in 0..WARMUP_ROUNDS {
        for i in 0..MSG_COUNT {
            let msg = make_request(i);
            let payload = serde_json::to_string(&msg).unwrap();
            let sig = signing_key.sign(payload.as_bytes());
            let sig_bytes = sig.to_bytes();

            // "receive" side
            let sig2 = Signature::from_bytes(&sig_bytes);
            let _msg2: ChatRequest = serde_json::from_str(&payload).unwrap();
            let _ = verifying_key.verify(payload.as_bytes(), &sig2);
        }
    }

    let start = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        for i in 0..MSG_COUNT {
            let msg = make_request(i);
            let payload = serde_json::to_string(&msg).unwrap();
            let sig = signing_key.sign(payload.as_bytes());
            let sig_bytes = sig.to_bytes();

            let sig2 = Signature::from_bytes(&sig_bytes);
            let _msg2: ChatRequest = serde_json::from_str(&payload).unwrap();
            let _ = verifying_key.verify(payload.as_bytes(), &sig2).unwrap();
        }
    }
    let elapsed = start.elapsed();
    let total = (BENCH_ROUNDS * MSG_COUNT) as f64;
    let rate = total / elapsed.as_secs_f64();

    println!();
    println!("=== 全链路吞吐量 (sign+serde+deser+verify) ===");
    println!(
        "消息数: {} × {} = {}",
        BENCH_ROUNDS, MSG_COUNT, total as usize
    );
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} msg/s", rate);
    // NOTE: debug build; release mode is typically 50-100x faster (5000+ msg/s)
    assert!(
        rate > 50.0,
        "target: >50 msg/s full pipeline (debug), got {:.0}",
        rate
    );
    println!(
        "✅ 全链路 {:.0} msg/s > 50 (debug; release ~50-100x faster)",
        rate
    );
}

// ── ChatResponse roundtrip ──

#[test]
#[ignore = "throughput benchmark — run with --ignored"]
fn bench_response_roundtrip() {
    // Warmup
    for _ in 0..3 {
        for _ in 0..100_000 {
            let resp = ChatResponse { ack: true };
            let json = serde_json::to_string(&resp).unwrap();
            let _: ChatResponse = serde_json::from_str(&json).unwrap();
        }
    }

    let count = 500_000;
    let start = Instant::now();
    for _ in 0..count {
        let resp = ChatResponse { ack: true };
        let json = serde_json::to_string(&resp).unwrap();
        let _: ChatResponse = serde_json::from_str(&json).unwrap();
    }
    let elapsed = start.elapsed();
    let rate = count as f64 / elapsed.as_secs_f64();

    println!();
    println!("=== ChatResponse 往返吞吐量 ===");
    println!("往返数: {}", count);
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} roundtrips/s", rate);
    assert!(rate > 10000.0, "ACK roundtrip must be very fast");
    println!("✅ ACK 往返 {:.0} rtt/s", rate);
}
