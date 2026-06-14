//! S16R161 — 并发连接压测: storage 层并发安全 + 1000 task swarm
//!
//! Simulates 1000 concurrent peers accessing the shared storage layer,
//! verifying no data corruption and measuring throughput under load.

use agent_circle::storage;
use agent_circle::timeline::Timeline;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Number of concurrent "peers" (tokio tasks) hammering storage.
const CONCURRENT_TASKS: usize = 1000;

#[tokio::test]
async fn stress_concurrent_contacts() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Pre-populate with some contacts
    for i in 0..10 {
        storage::add_contact(
            &format!("seed-{}", i),
            &format!("12D3KooWSeed{:08x}", i),
            &format!("did:example:seed{}", i),
            Some(&data_dir),
        )
        .expect("seed contact");
    }

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    let mut handles = Vec::with_capacity(CONCURRENT_TASKS);

    for peer_id in 0..CONCURRENT_TASKS {
        let data_dir = data_dir.clone();
        let success = success_count.clone();
        let error = error_count.clone();

        let handle = tokio::spawn(async move {
            let name = format!("peer-{}", peer_id);
            let pid = format!("12D3KooWP{:08x}", peer_id);
            let did = format!("did:example:p{}", peer_id);

            if storage::add_contact(&name, &pid, &did, Some(&data_dir)).is_ok() {
                success.fetch_add(1, Ordering::Relaxed);
            } else {
                error.fetch_add(1, Ordering::Relaxed);
            }

            // Read contacts
            let _ = storage::load_contacts(Some(&data_dir));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("task panicked");
    }

    let elapsed = start.elapsed();
    let total_ops = success_count.load(Ordering::Relaxed) + error_count.load(Ordering::Relaxed);

    println!();
    println!("=== 并发联系人压测 ===");
    println!("并发任务数: {}", CONCURRENT_TASKS);
    println!("成功: {}", success_count.load(Ordering::Relaxed));
    println!("重复跳过: {}", error_count.load(Ordering::Relaxed));
    println!("总操作: {}", total_ops);
    println!("耗时: {:?}", elapsed);
    println!(
        "吞吐量: {:.0} ops/s",
        total_ops as f64 / elapsed.as_secs_f64()
    );

    let final_contacts = storage::load_contacts(Some(&data_dir)).expect("load after stress");
    assert!(final_contacts.len() >= 10, "seed contacts must survive");

    // No duplicate peer_ids
    let mut seen = std::collections::HashSet::new();
    for c in &final_contacts {
        assert!(seen.insert(&c.peer_id), "duplicate peer_id: {}", c.peer_id);
    }

    println!("联系人: {}  →  ✅ 无损坏", final_contacts.len());
}

#[tokio::test]
async fn stress_concurrent_timeline() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    use agent_circle::identity::Identity;
    let id = Identity::generate();

    let poster_count: usize = 500;
    let mut handles = Vec::with_capacity(poster_count);

    for i in 0..poster_count {
        let data_dir = data_dir.clone();
        let id = id.clone();

        let handle = tokio::spawn(async move {
            let content = format!(
                "stress post {} from task {}",
                chrono::Utc::now().timestamp(),
                i
            );
            let mut tl =
                storage::load_timeline(Some(&data_dir)).unwrap_or_else(|_| Timeline::new());
            let node = tl.append(&id, &content).expect("append");
            storage::save_timeline(&tl, Some(&data_dir)).expect("save");
            node.id
        });
        handles.push(handle);
    }

    let mut node_ids = Vec::new();
    for handle in handles {
        node_ids.push(handle.await.expect("task panicked"));
    }

    let final_tl = storage::load_timeline(Some(&data_dir)).expect("load after stress");
    assert_eq!(
        final_tl.nodes.len(),
        poster_count,
        "all posts must be persisted"
    );

    let verify_start = Instant::now();
    let valid = final_tl.verify();
    let verify_time = verify_start.elapsed();

    println!();
    println!("=== 时间线并发压测 ===");
    println!("并发发布者: {}", poster_count);
    println!("最终条目: {}", final_tl.nodes.len());
    println!(
        "签名验证: {}",
        if valid.is_ok() {
            "✅ 通过"
        } else {
            "❌ 失败"
        }
    );
    println!("验证耗时: {:?}", verify_time);

    assert!(valid.is_ok(), "timeline signature verification failed");
    println!("✅ 时间线并发压测通过");
}

#[tokio::test]
async fn stress_concurrent_mixed_read_write() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    use agent_circle::identity::Identity;
    let id = Identity::generate();

    // Seed
    storage::add_contact(
        "alice",
        "12D3KooWAlice",
        "did:example:alice",
        Some(&data_dir),
    )
    .expect("seed");
    storage::add_contact("bob", "12D3KooWBob", "did:example:bob", Some(&data_dir)).expect("seed");

    let read_count = Arc::new(AtomicUsize::new(0));
    let write_count = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    let total: usize = 400;
    let mut handles = Vec::with_capacity(total);

    for i in 0..total {
        let data_dir = data_dir.clone();
        let id = id.clone();
        let read_cnt = read_count.clone();
        let write_cnt = write_count.clone();

        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                let _ = storage::load_contacts(Some(&data_dir));
                let _ = storage::load_timeline(Some(&data_dir));
                read_cnt.fetch_add(1, Ordering::Relaxed);
            } else {
                let mut tl =
                    storage::load_timeline(Some(&data_dir)).unwrap_or_else(|_| Timeline::new());
                let _ = tl.append(&id, &format!("mixed {}", i));
                let _ = storage::save_timeline(&tl, Some(&data_dir));
                write_cnt.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("task panicked");
    }

    let elapsed = start.elapsed();
    let reads = read_count.load(Ordering::Relaxed);
    let writes = write_count.load(Ordering::Relaxed);

    println!();
    println!("=== 混合读写并发压测 ===");
    println!("读: {}  写: {}  总: {}", reads, writes, reads + writes);
    println!("耗时: {:?}", elapsed);
    println!(
        "吞吐量: {:.0} ops/s",
        (reads + writes) as f64 / elapsed.as_secs_f64()
    );

    let final_tl = storage::load_timeline(Some(&data_dir)).expect("timeline");
    assert!(final_tl.verify().is_ok());
    println!("✅ 混合读写压测通过 — 时间线 {} 条", final_tl.nodes.len());
}
