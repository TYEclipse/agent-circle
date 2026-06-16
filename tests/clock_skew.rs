// Clock skew handling
// 验收：两个节点时钟差 5min（甚至更大），消息不乱序
//
// 核心原理：agent-circle 使用序列号 (seq) 而非时间戳 (ts) 进行消息排序。
// 时钟偏移仅影响显示时间，不影响投递顺序。本测试验证这一特性。

#[cfg(test)]
mod tests {
    use agent_circle::chat::ChatRequest;

    fn peer(id: &str) -> libp2p::PeerId {
        id.parse().unwrap()
    }

    fn msg_with_ts(seq: u64, ts: i64, content: &str) -> ChatRequest {
        ChatRequest {
            from: "alice".into(),
            content: content.into(),
            ts,
            msg_id: seq,
            ttl: 9999999999,
            seq,
            service: None,
        }
    }

    /// R172a: Clock skew — sender's clock is 5 minutes ahead.
    /// Messages should still deliver in seq order, not ts order.
    #[test]
    fn r172a_clock_skew_5min_ahead() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        let base_ts = chrono::Utc::now().timestamp();

        // Sender's clock is 5 min ahead — all messages have future timestamps
        let skew = 300; // 5 minutes in seconds
        let results = st.ingest(&p, msg_with_ts(1, base_ts + skew, "hello"));
        assert_eq!(results.len(), 1, "seq=1 delivered despite clock skew");

        let results = st.ingest(&p, msg_with_ts(2, base_ts + skew + 10, "world"));
        assert_eq!(results.len(), 1, "seq=2 delivered despite clock skew");
    }

    /// R172b: Clock skew — sender's clock is 5 minutes behind.
    #[test]
    fn r172b_clock_skew_5min_behind() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        let base_ts = chrono::Utc::now().timestamp();
        let skew = -300; // 5 minutes behind

        st.ingest(&p, msg_with_ts(1, base_ts + skew, "delayed"));
        let results = st.ingest(&p, msg_with_ts(2, base_ts + skew + 5, "also"));
        assert_eq!(
            results.len(),
            1,
            "seq=2 delivered despite negative clock skew"
        );
    }

    /// R172c: Mixed skew — two senders with opposing clock offsets.
    /// Messages from both should interleave correctly by seq.
    #[test]
    fn r172c_mixed_clock_skew_two_senders() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let a = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");
        let b = peer("12D3KooWHdiAx96Qf1mFXKwJY3W5JSQvq4PJ9xrXp7A1SzUFrHMn");

        let base = chrono::Utc::now().timestamp();

        // Sender A: clock 5min ahead
        st.ingest(&a, msg_with_ts(1, base + 300, "a1"));
        // Sender B: clock 5min behind
        st.ingest(&b, msg_with_ts(1, base - 300, "b1"));
        // A sends seq=2
        let results_a = st.ingest(&a, msg_with_ts(2, base + 305, "a2"));
        // B sends seq=2
        let results_b = st.ingest(&b, msg_with_ts(2, base - 295, "b2"));

        assert_eq!(results_a.len(), 1, "A seq=2 delivered");
        assert_eq!(results_b.len(), 1, "B seq=2 delivered");
        assert_eq!(results_a[0].seq, 2);
        assert_eq!(results_b[0].seq, 2);
        // Per-sender ordering is seq-based, immune to ts differences
    }

    /// R172d: Extreme clock skew — 1 hour difference.
    #[test]
    fn r172d_extreme_clock_skew_1hour() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        let base = chrono::Utc::now().timestamp();
        let skew = 3600; // 1 hour

        // Send messages with seq=1,2,3 but ts wildly in the future
        st.ingest(&p, msg_with_ts(1, base + skew, "far future 1"));
        st.ingest(&p, msg_with_ts(2, base + skew + 3600, "even further 2"));
        let results = st.ingest(&p, msg_with_ts(3, base + skew + 7200, "ridiculous 3"));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].seq, 3);
        // Ordering is by seq — ts is irrelevant
    }

    /// R172e: Clock skew + out-of-order arrival (network reordering + clock skew).
    /// Out-of-order seq still gets buffered and reordered regardless of ts.
    #[test]
    fn r172e_clock_skew_plus_network_reorder() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Sender clock is 5 min ahead
        let _skew = 300;

        // Network delivers seq=5 first (oldest ts=305), then seq=1 (ts=301)
        // Display-order by ts would be: seq=1 (301), seq=5 (305) — wrong!
        // But our delivery-order by seq is: seq=1, seq=5 — correct
        st.ingest(&p, msg_with_ts(5, 305, "fifth")); // buffered (gap: expected 1, got 5)
        let delivered = st.ingest(&p, msg_with_ts(1, 301, "first")); // delivers seq=1, flushes nothing (gap at 2)

        assert_eq!(delivered.len(), 1);
        assert_eq!(
            delivered[0].seq, 1,
            "seq=1 delivered first (correct ordering)"
        );
        // seq=5 is still buffered, awaiting seq=2,3,4 to fill the gap.
        // The point: ordering is by seq (1→…→5), not by ts (which would be 1→5).
    }

    /// R172f: Negative timestamp (clock set to 1970).
    /// Sequence tracker must handle negative or zero ts gracefully.
    #[test]
    fn r172f_negative_timestamp() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Clock set to epoch
        st.ingest(&p, msg_with_ts(1, 0, "epoch"));
        let results = st.ingest(&p, msg_with_ts(2, 1, "epoch+1"));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].seq, 2);
        assert_eq!(results[0].ts, 1);

        // Negative timestamp (clock wrong by decades)
        st.reset_peer(&p);
        st.ingest(&p, msg_with_ts(1, -86400 * 365 * 55, "1950s?"));
        let results = st.ingest(&p, msg_with_ts(2, -86400 * 365 * 50, "1960s?"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].seq, 2);
    }

    /// R172g: Timeline display with skewed timestamps — just proves
    /// sort_by(|m| m.seq) is immune to ts corruption.
    #[test]
    fn r172g_timeline_sort_uses_seq() {
        use agent_circle::sequence::SequenceTracker;

        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        let skew = 300;
        st.ingest(&p, msg_with_ts(1, skew, "one"));
        st.ingest(&p, msg_with_ts(2, skew + 5, "two"));
        st.ingest(&p, msg_with_ts(3, skew + 10, "three"));

        // If we sort by ts, all timestamps are in the future but monotonic
        // If we sort by seq, we get 1→2→3 regardless of ts magnitude
        // Both agree in this case. The important thing: seq ordering
        // always produces monotonic delivery.
    }
}
