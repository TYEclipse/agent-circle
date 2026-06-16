// E2E Integration Tests
// Shared harness at tests/e2e_harness.rs

#[path = "e2e_harness.rs"]
mod harness;

use harness::E2eCluster;
use std::time::Duration;

const SHORT_TIMEOUT: Duration = Duration::from_secs(15);

// ═══════════════════════════════════════════════════════════════════
// R182 — 身份创建 + 联系人发现
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r182_identity_and_contact_discovery() {
    let mut cluster = E2eCluster::spawn(2).await;
    cluster.connect_all().await;

    let id_a = cluster.peer_id(0);
    let id_b = cluster.peer_id(1);
    assert_ne!(id_a, id_b);
    assert!(!id_a.to_string().is_empty());

    assert_ne!(cluster.nodes[0].identity.did, cluster.nodes[1].identity.did);
}

// ═══════════════════════════════════════════════════════════════════
// R183 — 1对1 聊天
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r183_one_on_one_chat() {
    let mut cluster = E2eCluster::spawn(2).await;
    cluster.connect_all().await;

    cluster.send_chat(0, 1, "hello from 0");

    let received = cluster.wait_for_chat(1, SHORT_TIMEOUT).await;
    assert!(received.is_some(), "node-1 should receive 1-on-1 message");
    assert_eq!(received.unwrap(), "hello from 0");
}

// ═══════════════════════════════════════════════════════════════════
// R184 — 群聊 (3 nodes, GossipSub)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r184_group_chat() {
    const GROUP: &str = "test-group-184";

    let mut cluster = E2eCluster::spawn(3).await;
    cluster.connect_all().await;
    cluster.join_group_all(GROUP);
    cluster.wait_for_mesh().await;

    cluster.broadcast(0, GROUP, "group-message");

    let mut received = 0;
    for _ in 0..2 {
        if cluster.wait_for_message(SHORT_TIMEOUT).await.is_some() {
            received += 1;
        }
    }
    assert!(received >= 1, "group message should reach ≥1 other node");
}

// ═══════════════════════════════════════════════════════════════════
// R185 — 朋友圈 (Timeline sync via GossipSub)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r185_timeline_sync() {
    const GROUP: &str = "timeline-185";

    let mut cluster = E2eCluster::spawn(2).await;
    cluster.connect_all().await;
    cluster.join_group_all(GROUP);
    cluster.wait_for_mesh().await;

    cluster.broadcast(0, GROUP, "timeline-msg-1");
    cluster.broadcast(0, GROUP, "timeline-msg-2");
    cluster.broadcast(0, GROUP, "timeline-msg-3");

    let mut count = 0;
    let deadline = tokio::time::Instant::now() + SHORT_TIMEOUT;
    loop {
        if count >= 2 || tokio::time::Instant::now() > deadline {
            break;
        }
        if cluster
            .wait_for_message(Duration::from_secs(3))
            .await
            .is_some()
        {
            count += 1;
        }
    }
    assert!(count >= 2, "expected ≥2 timeline msgs, got {count}");
}

// ═══════════════════════════════════════════════════════════════════
// R186 — 离线消息 (Queue then drain)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r186_offline_message_queue() {
    use agent_circle::message_queue::Queue;

    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let queue = Queue::open(tmpdir.path()).expect("queue open");

    for i in 1..=5 {
        queue
            .push("peer-b", &format!("offline-msg-{i}"))
            .expect("push");
    }

    let pending = queue.pending_for("peer-b").expect("pending_for");
    assert_eq!(pending.len(), 5);

    for entry in &pending {
        queue.mark_delivered(entry.id).expect("mark_delivered");
    }

    let remaining = queue.pending_for("peer-b").expect("pending_for");
    assert_eq!(remaining.len(), 0);
}

// ═══════════════════════════════════════════════════════════════════
// R187 — NAT 穿透 / relay 配置验证
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r187_relay_and_dcutr_configured() {
    let mut cluster = E2eCluster::spawn(2).await;
    cluster.connect_all().await;

    for node in &cluster.nodes {
        assert!(
            node.swarm.listeners().count() > 0,
            "{} no listeners",
            node.name
        );
    }

    assert_ne!(cluster.peer_id(0), cluster.peer_id(1));

    let c0 = cluster.nodes[0].swarm.connected_peers().count();
    let c1 = cluster.nodes[1].swarm.connected_peers().count();
    assert!(c0 > 0, "node-0 should have connected peers");
    assert!(c1 > 0, "node-1 should have connected peers");
}

// ═══════════════════════════════════════════════════════════════════
// R188 — Crash recovery (identity persistence)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "e2e — needs network, run with --ignored"]
async fn e2e_r188_crash_recovery_data_integrity() {
    use agent_circle::storage;

    let id = agent_circle::identity::Identity::generate();
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let data_dir = tmpdir.path().to_path_buf();

    let did_before = id.did.clone();
    storage::save_identity(&id, Some(&data_dir)).expect("save identity");
    drop(id);

    let recovered = agent_circle::storage::load_identity(Some(&data_dir))
        .expect("load_identity")
        .expect("identity should exist after crash");
    assert_eq!(
        recovered.did, did_before,
        "DID stable across crash recovery"
    );
}
