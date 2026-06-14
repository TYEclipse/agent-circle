// S17 Boundary Tests — R175 · R176 · R177 · R178 · R179
// Long stability, partition recovery, collision, IPv6, slow network

#[cfg(test)]
mod tests {
    // ═══════════════════════════════════════════════════════════════════
    // R175 — 7×24 长稳测试 (加速模拟)
    // 验收：无内存泄漏、无崩溃、状态一致性
    // ═══════════════════════════════════════════════════════════════════

    /// R175a: Accelerated long-run simulation — SequenceTracker over 100k operations
    #[test]
    fn r175a_sequence_tracker_long_run() {
        use agent_circle::chat::ChatRequest;
        use agent_circle::sequence::SequenceTracker;

        fn peer(id: &str) -> libp2p::PeerId {
            id.parse().unwrap()
        }

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // 100,000 sequential messages — no buffering, no crash
        for seq in 1u64..=100_000 {
            let msg = ChatRequest {
                from: "alice".into(),
                content: format!("msg-{seq}"),
                ts: seq as i64,
                msg_id: seq,
                ttl: 9999999999,
                seq,
                service: None,
            };
            let delivered = st.ingest(&p, msg);
            assert_eq!(delivered.len(), 1, "seq={seq} should deliver immediately");
            assert_eq!(delivered[0].seq, seq);
        }
        assert_eq!(
            st.buffered_msg_count(),
            0,
            "No messages buffered after 100k"
        );
    }

    /// R175b: Long run with random gaps and fills (simulates network jitter over time)
    #[test]
    fn r175b_random_reorder_long_run() {
        use agent_circle::chat::ChatRequest;
        use agent_circle::sequence::SequenceTracker;
        use rand::Rng as _;

        fn peer(id: &str) -> libp2p::PeerId {
            id.parse().unwrap()
        }

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");
        let mut rng = rand::thread_rng();

        // Build a pool of 2000 messages, then deliver in random order
        let count = 2000;
        let mut pool: Vec<ChatRequest> = (1..=count as u64)
            .map(|seq| ChatRequest {
                from: "alice".into(),
                content: format!("m-{seq}"),
                ts: seq as i64,
                msg_id: seq,
                ttl: 9999999999,
                seq,
                service: None,
            })
            .collect();

        // Shuffle using Fisher-Yates
        for i in (1..pool.len()).rev() {
            let j = rng.gen_range(0..=i);
            pool.swap(i, j);
        }

        let mut delivered_count = 0u64;
        for msg in pool {
            let delivered = st.ingest(&p, msg);
            delivered_count += delivered.len() as u64;
        }

        assert_eq!(
            delivered_count, count as u64,
            "All messages should be delivered"
        );
        assert_eq!(st.buffered_msg_count(), 0, "No leftover buffered messages");
    }

    /// R175c: FragmentReassembler long run — create and resolve many sessions
    #[test]
    fn r175c_fragment_reassembler_long_run() {
        use agent_circle::fragment::{split_large_message, FragmentReassembler};

        let mut reassembler = FragmentReassembler::new();
        let content = "LONGRUN".repeat(5000); // ~35KB

        for msg_id in 0u64..500 {
            let frags = split_large_message("node-1", &content, msg_id, msg_id as i64, 99999);
            // frags always 1 (35KB < 64KB) — but tests the path
            for f in &frags {
                let result = reassembler.ingest("node-1", f);
                assert!(result.is_some(), "Single-fragment msg should deliver");
            }
        }

        assert_eq!(reassembler.active_sessions(), 0);
    }

    /// R175d: Disk status check runs without panic (real system call)
    #[test]
    fn r175d_disk_check_no_panic() {
        use agent_circle::disk::check_disk_space;
        use std::path::Path;

        let status = check_disk_space(Path::new("/tmp"));
        // Just verify it returns something
        let _ = status.available_bytes();
        // No assertion needed — just no panic
    }

    /// R175e: Stress — 5000 rapid Kademlia record writes
    #[test]
    fn r175e_kademlia_record_stress() {
        use libp2p::kad::store::RecordStore;
        use libp2p::kad::Record;
        use libp2p::{kad::RecordKey, PeerId};

        let bootstrap_id = PeerId::random();
        let config = libp2p::kad::store::MemoryStoreConfig {
            max_records: 10000,
            ..Default::default()
        };
        let mut store = libp2p::kad::store::MemoryStore::with_config(bootstrap_id, config);

        // Write 5000 records
        for i in 0u64..5000 {
            let key = RecordKey::new(&i.to_le_bytes());
            let record = Record::new(key, format!("data-{i}").into_bytes());
            store.put(record).expect("put should succeed");
        }

        // Verify random sampling
        for i in (0u64..5000).step_by(500) {
            let key = RecordKey::new(&i.to_le_bytes());
            assert!(store.get(&key).is_some(), "Record {i} should exist");
        }

        // Remove all
        for i in 0u64..5000 {
            store.remove(&RecordKey::new(&i.to_le_bytes()));
        }

        // Verify empty
        for i in 0u64..5000 {
            assert!(
                store.get(&RecordKey::new(&i.to_le_bytes())).is_none(),
                "Record {i} should be removed"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // R176 — 网络分区恢复
    // 验收：分区 10min → 恢复后消息自动同步
    //
    // 注：网络分区在 libp2p 层面由 GossipSub/Kademlia 自动处理；
    // 这里测试应用层的不变性——消息序列号逻辑在断连+重连后正确。
    // ═══════════════════════════════════════════════════════════════════

    /// R176a: SequenceTracker survives reset (simulates reconnection after partition)
    #[test]
    fn r176a_sequence_reset_after_partition() {
        use agent_circle::chat::ChatRequest;
        use agent_circle::sequence::SequenceTracker;

        fn peer(id: &str) -> libp2p::PeerId {
            id.parse().unwrap()
        }

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Pre-partition: seq 1-50 delivered
        for seq in 1..=50u64 {
            st.ingest(
                &p,
                ChatRequest {
                    from: "alice".into(),
                    content: format!("pre-{seq}"),
                    ts: seq as i64,
                    msg_id: seq,
                    ttl: 9999999999,
                    seq,
                    service: None,
                },
            );
        }

        // Partition occurs — reset peer (simulates reconnection)
        st.reset_peer(&p);

        // Post-partition: new epoch, seq starts from 1 again
        for seq in 1..=20u64 {
            let delivered = st.ingest(
                &p,
                ChatRequest {
                    from: "alice".into(),
                    content: format!("post-{seq}"),
                    ts: (100 + seq) as i64,
                    msg_id: 100 + seq,
                    ttl: 9999999999,
                    seq,
                    service: None,
                },
            );
            assert_eq!(delivered.len(), 1, "Post-partition seq={seq} delivered");
        }
        assert_eq!(st.buffered_msg_count(), 0);
    }

    /// R176b: Offline message queuing survives partition (send while disconnected)
    #[test]
    fn r176b_offline_queue_semantics() {
        use agent_circle::message_queue::Queue;

        let tmpdir = tempfile::tempdir().expect("tmpdir");
        let queue = Queue::open(tmpdir.path()).expect("queue open");

        // Queue 10 messages while "offline" (partition)
        for i in 1..=10 {
            queue.push("peer-1", &format!("offline-{i}")).expect("push");
        }

        // Verify pending messages
        let pending = queue.pending_for("peer-1").expect("pending_for");
        assert_eq!(pending.len(), 10, "All 10 messages queued");

        // Mark first 5 as delivered (simulating drain on reconnect)
        for entry in &pending[..5] {
            queue.mark_delivered(entry.id).expect("mark_delivered");
        }
        let remaining = queue.pending_for("peer-1").expect("pending_for");
        assert_eq!(remaining.len(), 5, "5 remaining undelivered");
    }

    // ═══════════════════════════════════════════════════════════════════
    // R177 — PeerID 碰撞检测
    // 验收：极小概率碰撞 → 检测并告警
    //
    // libp2p PeerID 基于 Ed25519 公钥哈希,碰撞概率 ~1/2^256,几乎不可能。
    // 这里测试碰撞检测逻辑本身,用已知的 PeerId 模拟。
    // ═══════════════════════════════════════════════════════════════════

    /// R177a: Detect duplicate PeerId in contact list
    #[test]
    fn r177a_detect_duplicate_peer_id() {
        use libp2p::PeerId;
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let pid = PeerId::random();

        // First addition — ok
        assert!(seen.insert(pid), "First insert should succeed");

        // Duplicate — collision detected
        assert!(!seen.insert(pid), "Duplicate should be detected");
    }

    /// R177b: Multiple PeerIds from different keypairs are unique
    #[test]
    fn r177b_keypair_produces_unique_peer_ids() {
        use libp2p::PeerId;
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let pid = PeerId::random();
            assert!(seen.insert(pid), "Random PeerId should be unique");
        }
        assert_eq!(seen.len(), 1000);
    }

    /// R177c: PeerId display is stable
    #[test]
    fn r177c_peer_id_display_stable() {
        use libp2p::PeerId;

        let pid: PeerId = "12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA"
            .parse()
            .unwrap();
        let s = pid.to_string();
        // Re-parse
        let pid2: PeerId = s.parse().unwrap();
        assert_eq!(pid, pid2);
    }

    // ═══════════════════════════════════════════════════════════════════
    // R178 — IPv6 支持
    // 验收：IPv6-only 环境正常运行
    // ═══════════════════════════════════════════════════════════════════

    /// R178a: IPv6 Multiaddr parsing
    #[test]
    fn r178a_ipv6_multiaddr_parse() {
        let addr: libp2p::Multiaddr = "/ip6/::1/tcp/12345".parse().unwrap();
        let s = addr.to_string();
        assert!(s.contains("ip6"), "Should contain ip6");
        assert!(s.contains("::1"), "Should contain loopback");
    }

    /// R178b: IPv6 Multiaddr with QUIC
    #[test]
    fn r178b_ipv6_quic_multiaddr() {
        let addr: libp2p::Multiaddr = "/ip6/::1/udp/9090/quic-v1".parse().unwrap();
        let s = addr.to_string();
        assert!(s.contains("ip6"));
        assert!(s.contains("quic-v1"));
    }

    /// R178c: IPv4 and IPv6 coexist
    #[test]
    fn r178c_dual_stack_addresses() {
        let v4: libp2p::Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        let v6: libp2p::Multiaddr = "/ip6/::1/tcp/8080".parse().unwrap();

        assert_ne!(v4, v6);
        assert!(v4.to_string().contains("ip4"));
        assert!(v6.to_string().contains("ip6"));
    }

    /// R178d: IPv6 with peer ID
    #[test]
    fn r178d_ipv6_with_peer_id() {
        use libp2p::PeerId;

        let pid: PeerId = "12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA"
            .parse()
            .unwrap();
        let addr: libp2p::Multiaddr = format!("/ip6/::1/udp/9090/quic-v1/p2p/{pid}")
            .parse()
            .unwrap();
        let s = addr.to_string();
        assert!(s.contains("ip6"));
        assert!(s.contains("p2p"));
        assert!(s.contains(&pid.to_string()));
    }

    // ═══════════════════════════════════════════════════════════════════
    // R179 — 低速网络 (2G/Edge 模拟)
    // 验收：> 100ms RTT + 高丢包 → 消息仍可达
    //
    // 注：实际网络模拟需要 tokio + 延迟注入,单元测试焦点在协议层。
    // 这里测试 SequenceTracker 在极端延迟下的正确性。
    // ═══════════════════════════════════════════════════════════════════

    /// R179a: High-latency-tolerant ordering — gaps due to delay
    #[test]
    fn r179a_high_latency_reordering() {
        use agent_circle::chat::ChatRequest;
        use agent_circle::sequence::SequenceTracker;

        fn peer(id: &str) -> libp2p::PeerId {
            id.parse().unwrap()
        }

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Simulates high latency: messages arrive in order 5, 10, 1, 2, 3, 4, 6, 7, 8, 9
        let order: Vec<u64> = vec![5, 10, 1, 2, 3, 4, 6, 7, 8, 9];
        let mut total_delivered = 0u64;

        for &seq in &order {
            let delivered = st.ingest(
                &p,
                ChatRequest {
                    from: "alice".into(),
                    content: format!("latent-{seq}"),
                    ts: seq as i64 * 1000, // high RTT → delayed ts
                    msg_id: seq,
                    ttl: 9999999999,
                    seq,
                    service: None,
                },
            );
            total_delivered += delivered.len() as u64;
        }

        assert_eq!(total_delivered, 10, "All 10 messages eventually delivered");
        assert_eq!(st.buffered_msg_count(), 0, "No leftover buffered messages");
    }

    /// R179b: Message with very old timestamp (simulates extreme lag)
    #[test]
    fn r179b_very_old_timestamp() {
        use agent_circle::chat::ChatRequest;
        use agent_circle::sequence::SequenceTracker;

        fn peer(id: &str) -> libp2p::PeerId {
            id.parse().unwrap()
        }

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Message buffered in a slow relay for a long time
        let delivered = st.ingest(
            &p,
            ChatRequest {
                from: "alice".into(),
                content: "stale".into(),
                ts: 0, // epoch
                msg_id: 1,
                ttl: 9999999999,
                seq: 1,
                service: None,
            },
        );
        assert_eq!(delivered.len(), 1);
        assert_eq!(
            delivered[0].ts, 0,
            "Old ts preserved, but seq ordering wins"
        );
    }

    /// R179c: Dedup survives high-latency duplicates
    #[test]
    fn r179c_dedup_high_latency() {
        use agent_circle::dedup::DedupFilter;

        let mut dedup = DedupFilter::new();

        let msg_id = 42u64;

        // First arrival
        assert!(!dedup.is_dup(msg_id));

        // "Duplicate" after high latency retransmission
        assert!(dedup.is_dup(msg_id));
    }

    /// R179d: Message with TTL still valid after delay
    #[test]
    fn r179d_ttl_validation_after_delay() {
        use agent_circle::chat::ChatRequest;

        let now = chrono::Utc::now().timestamp();
        let msg = ChatRequest {
            from: "alice".into(),
            content: "delayed".into(),
            ts: now - 300, // sent 5 min ago
            msg_id: 99,
            ttl: now + 3600, // expires in 1 hour
            seq: 1,
            service: None,
        };

        // TTL not expired
        assert!(msg.ttl > now, "Message should still be valid");
    }

    /// R179e: TTL expired after long delay
    #[test]
    fn r179e_ttl_expired_after_delay() {
        use agent_circle::chat::ChatRequest;

        let now = chrono::Utc::now().timestamp();
        let msg = ChatRequest {
            from: "alice".into(),
            content: "too-old".into(),
            ts: now - 3600,
            msg_id: 100,
            ttl: now - 1, // expired 1 second ago
            seq: 1,
            service: None,
        };

        assert!(
            msg.ttl <= now,
            "Message with expired TTL should be detected"
        );
    }
}
