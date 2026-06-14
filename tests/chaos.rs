//! Chaos engineering — crash recovery, partition, flood.
//!
//! Tests that the system degrades gracefully under adverse conditions.

use agent_circle::message_queue::Queue;

// ── R47: Crash recovery (queue persistence) ─────────────────────────

#[test]
fn chaos_crash_recovery_queue_survives_restart() {
    let tmp = std::env::temp_dir().join("ac_chaos_r47");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Phase 1: open queue, push messages, drop it (simulate crash)
    {
        let q = Queue::open(&tmp).unwrap();
        for i in 0..10 {
            q.push("crash-recipient", &format!("msg-{i}")).unwrap();
        }
        // Queue dropped here — simulates crash
    }

    // Phase 2: re-open, verify all messages survived
    {
        let q = Queue::open(&tmp).unwrap();
        let pending = q.pending_for("crash-recipient").unwrap();
        assert_eq!(pending.len(), 10, "all 10 messages should survive crash");
        for (i, entry) in pending.iter().enumerate() {
            assert_eq!(entry.content, format!("msg-{i}"));
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── R47: Crash recovery (empty queue) ───────────────────────────────

#[test]
fn chaos_crash_recovery_empty_queue_on_first_open() {
    let tmp = std::env::temp_dir().join("ac_chaos_r47b");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let q = Queue::open(&tmp).unwrap();
    assert!(
        q.pending_for("nobody").unwrap().is_empty(),
        "empty queue should have no pending"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── R48: Network partition (offline queue) ──────────────────────────

#[test]
fn chaos_offline_queue_preserves_messages() {
    // Simulates: sender queues messages while recipient is offline.
    // When recipient comes back online, all messages should be there.
    let tmp = std::env::temp_dir().join("ac_chaos_r48");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let q = Queue::open(&tmp).unwrap();

    // Offline phase: push 5 messages while recipient is gone
    for i in 0..5 {
        q.push("offline-recipient", &format!("partition-msg-{i}"))
            .unwrap();
    }

    // "Reconnect" — recipient checks queue
    let pending = q.pending_for("offline-recipient").unwrap();
    assert_eq!(pending.len(), 5, "all offline messages preserved");

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── R49: Message flood (rapid fire) ─────────────────────────────────

#[test]
fn chaos_flood_queue_handles_rapid_inserts() {
    let tmp = std::env::temp_dir().join("ac_chaos_r49");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let q = Queue::open(&tmp).unwrap();

    // Flood: 200 messages in tight loop
    let count = 200;
    for i in 0..count {
        q.push("flood-recipient", &format!("flood-{i}")).unwrap();
    }

    // Verify all arrived
    let pending = q.pending_for("flood-recipient").unwrap();
    assert_eq!(pending.len(), count, "all flood messages should be queued");

    // Verify ordering preserved (oldest first)
    for (i, entry) in pending.iter().enumerate() {
        assert_eq!(entry.content, format!("flood-{i}"));
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── R49: Flood + drain (mark delivered) ─────────────────────────────

#[test]
fn chaos_flood_then_drain() {
    let tmp = std::env::temp_dir().join("ac_chaos_r49b");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let q = Queue::open(&tmp).unwrap();

    // Flood
    for i in 0..50 {
        q.push("drain-recipient", &format!("drain-{i}")).unwrap();
    }

    // Drain half
    let pending = q.pending_for("drain-recipient").unwrap();
    for entry in &pending[..25] {
        q.mark_delivered(entry.id).unwrap();
    }

    // Remaining half should be exactly 25
    let remaining = q.pending_for("drain-recipient").unwrap();
    assert_eq!(remaining.len(), 25);
    for (i, entry) in remaining.iter().enumerate() {
        assert_eq!(entry.content, format!("drain-{}", i + 25));
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── R47: Crash recovery with partial delivery ───────────────────────

#[test]
fn chaos_crash_recovery_partial_delivery() {
    let tmp = std::env::temp_dir().join("ac_chaos_r47c");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Phase 1: push 10, mark first 3 as delivered, "crash"
    {
        let q = Queue::open(&tmp).unwrap();
        for i in 0..10 {
            q.push("partial-recipient", &format!("p-{i}")).unwrap();
        }
        let pending = q.pending_for("partial-recipient").unwrap();
        for entry in &pending[..3] {
            q.mark_delivered(entry.id).unwrap();
        }
    }

    // Phase 2: re-open, only 7 should remain
    {
        let q = Queue::open(&tmp).unwrap();
        let remaining = q.pending_for("partial-recipient").unwrap();
        assert_eq!(
            remaining.len(),
            7,
            "7 of 10 should remain after 3 delivered"
        );
        for (i, entry) in remaining.iter().enumerate() {
            assert_eq!(entry.content, format!("p-{}", i + 3));
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
