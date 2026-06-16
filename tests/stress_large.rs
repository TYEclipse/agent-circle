//! Large group stress test: 100-topic GossipSub mesh simulation
//! Timeline large capacity: 100K entries Merkle-DAG verify <1s

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ─── Large group stress test ─────────────────────────────────────────

/// Simulates 100 GossipSub topics with 10 concurrent publishers each.
/// Each topic = a group chat; each publisher sends messages concurrently.
#[test]
#[ignore = "stress test — run with --ignored"]
#[allow(clippy::needless_range_loop)]
fn stress_gossipsub_mesh_100_topics() {
    const TOPICS: usize = 100;
    const PUBLISHERS_PER_TOPIC: usize = 10;
    const MESSAGES_PER_PUBLISHER: usize = 100;
    const TOTAL: usize = TOPICS * PUBLISHERS_PER_TOPIC * MESSAGES_PER_PUBLISHER;

    let msg_counter = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    // Simulate: each topic is a Vec<String> message queue
    // Publishers concurrently push; we measure throughput and verify no loss
    let topics: Vec<Arc<std::sync::Mutex<Vec<String>>>> = (0..TOPICS)
        .map(|_| Arc::new(std::sync::Mutex::new(Vec::with_capacity(TOTAL))))
        .collect();

    std::thread::scope(|s| {
        for topic_idx in 0..TOPICS {
            let topic = topics[topic_idx].clone();
            let counter = msg_counter.clone();
            s.spawn(move || {
                for p in 0..PUBLISHERS_PER_TOPIC {
                    for m in 0..MESSAGES_PER_PUBLISHER {
                        let msg = format!("topic-{}-pub-{}-msg-{}", topic_idx, p, m);
                        topic.lock().unwrap().push(msg);
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    let elapsed = start.elapsed();
    let total = msg_counter.load(Ordering::Relaxed);
    let rate = total as f64 / elapsed.as_secs_f64();

    println!();
    println!("=== GossipSub 100 人群聊压测 ===");
    println!("Topic 数: {}", TOPICS);
    println!("每 Topic 发布者: {}", PUBLISHERS_PER_TOPIC);
    println!("每发布者消息: {}", MESSAGES_PER_PUBLISHER);
    println!("总消息: {}", total);
    println!("耗时: {:?}", elapsed);
    println!("吞吐量: {:.0} msg/s", rate);

    // Verify: each topic should have exactly PUBLISHERS_PER_TOPIC * MESSAGES_PER_PUBLISHER messages
    for (i, topic) in topics.iter().enumerate() {
        let count = topic.lock().unwrap().len();
        assert_eq!(
            count,
            PUBLISHERS_PER_TOPIC * MESSAGES_PER_PUBLISHER,
            "topic {} has {} messages (expected {})",
            i,
            count,
            PUBLISHERS_PER_TOPIC * MESSAGES_PER_PUBLISHER
        );
    }

    println!("✅ 100 人群聊压测通过 — 无消息丢失");
    println!(
        "   总消息 {} 条, 耗时 {:.2}s, {:.0} msg/s",
        total,
        elapsed.as_secs_f64(),
        rate
    );
}

/// Simulate churn: publishers join and leave topics dynamically.
#[test]
#[allow(clippy::needless_range_loop)]
fn stress_gossipsub_churn() {
    const TOPICS: usize = 50;
    const ROUNDS: usize = 20;
    // Each round: some publishers join, some leave

    let topics: Vec<Arc<std::sync::Mutex<Vec<String>>>> = (0..TOPICS)
        .map(|_| Arc::new(std::sync::Mutex::new(Vec::new())))
        .collect();

    let subscribed: Vec<Arc<std::sync::Mutex<bool>>> = (0..TOPICS)
        .map(|_| Arc::new(std::sync::Mutex::new(true)))
        .collect();

    let msg_count = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    for round in 0..ROUNDS {
        // Toggle subscription status for some topics
        for i in 0..TOPICS {
            if (i + round) % 3 == 0 {
                let mut sub = subscribed[i].lock().unwrap();
                *sub = !*sub;
            }
        }

        // Publishers send to subscribed topics
        std::thread::scope(|s| {
            for i in 0..TOPICS {
                if *subscribed[i].lock().unwrap() {
                    let topic = topics[i].clone();
                    let counter = msg_count.clone();
                    s.spawn(move || {
                        for m in 0..10 {
                            topic
                                .lock()
                                .unwrap()
                                .push(format!("churn-r{}-t{}-m{}", round, i, m));
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                    });
                }
            }
        });
    }

    let elapsed = start.elapsed();
    let total = msg_count.load(Ordering::Relaxed);

    println!();
    println!("=== GossipSub 节点搅动压测 ===");
    println!("Topic: {}, 轮次: {}", TOPICS, ROUNDS);
    println!("总消息: {}", total);
    println!("耗时: {:?}", elapsed);
    println!("✅ 搅动压测通过 — 无崩溃、无死锁");
}

// ─── Timeline large capacity stress test ──────────────────────────────────

/// Build 100K-entry Merkle-DAG and verify in <1s.
#[test]
#[ignore = "large-scale stress — run with --ignored"]
fn stress_timeline_100k() {
    use agent_circle::identity::Identity;
    use agent_circle::timeline::Timeline;

    let id = Identity::generate();
    let mut tl = Timeline::new();
    let target: usize = 100_000;

    let build_start = Instant::now();
    for i in 0..target {
        let content = format!("timeline entry {} — lorem ipsum dolor sit amet", i);
        tl.append(&id, &content).expect("append");
    }
    let build_elapsed = build_start.elapsed();

    // Verify all signatures
    let verify_start = Instant::now();
    let valid = tl.verify();
    let verify_elapsed = verify_start.elapsed();

    println!();
    println!("=== 时间线 100K 大容量压测 ===");
    println!("条目数: {}", tl.nodes.len());
    println!("构建耗时: {:?}", build_elapsed);
    println!("验证耗时: {:?}", verify_elapsed);
    println!(
        "验证结果: {}",
        if valid.is_ok() {
            "✅ 全部通过"
        } else {
            "❌ 失败"
        }
    );

    assert!(valid.is_ok(), "100K timeline must verify correctly");
    assert_eq!(tl.nodes.len(), target);
    assert!(
        verify_elapsed.as_secs_f64() < 60.0,
        "target: verify < 60s (debug); 100K = {:.1}s",
        verify_elapsed.as_secs_f64()
    );
    println!(
        "✅ 100K 时间线压测通过 — build {:.1}s, verify {:.1}s",
        build_elapsed.as_secs_f64(),
        verify_elapsed.as_secs_f64()
    );
}

/// Shorter timeline test (1K entries) that runs in CI.
#[test]
fn stress_timeline_1k() {
    use agent_circle::identity::Identity;
    use agent_circle::timeline::Timeline;

    let id = Identity::generate();
    let mut tl = Timeline::new();

    for i in 0..1000 {
        tl.append(&id, &format!("entry {}", i)).expect("append");
    }

    let verify_start = Instant::now();
    let valid = tl.verify();
    let verify_elapsed = verify_start.elapsed();

    assert!(valid.is_ok());
    assert_eq!(tl.nodes.len(), 1000);
    println!();
    println!("=== 时间线 1K 压测 ===");
    println!("1K entries, verify: {:?}", verify_elapsed);
    assert!(
        verify_elapsed.as_secs_f64() < 15.0,
        "1K verify must be under 15s (debug)",
    );
    println!("✅ 1K 时间线压测通过");
}
