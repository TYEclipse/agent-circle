//! Message reliability — ACK tracking + retry with exponential backoff.
//!
//! Every sent 1-to-1 message is tracked by its `OutboundRequestId`.
//! When the recipient's auto-ACK (ChatResponse) arrives, the message
//! is marked delivered.  If the transport fails, the tracker retries
//! up to `MAX_RETRIES` times before handing off to the offline queue.
//!
//! This is the core mechanism that pushes delivery from "best effort"
//! toward the 99.9% target.

use libp2p::{request_response::OutboundRequestId, PeerId};
use std::collections::HashMap;
use std::time::Instant;

/// Maximum retries before handing a message off to the offline queue.
pub const MAX_RETRIES: u32 = 3;

/// Tracks in-flight 1-to-1 messages that are awaiting ACK.
pub struct PendingTracker {
    pending: HashMap<OutboundRequestId, PendingEntry>,
}

/// A single in-flight message.
#[derive(Debug, Clone)]
pub struct PendingEntry {
    pub peer: PeerId,
    pub from: String,
    pub content: String,
    pub ts: i64,
    pub retries: u32,
    pub created_at: Instant,
}

impl PendingTracker {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Register a newly-sent message so we can correlate the ACK later.
    pub fn track(
        &mut self,
        id: OutboundRequestId,
        peer: PeerId,
        from: String,
        content: String,
        ts: i64,
    ) {
        self.pending.insert(
            id,
            PendingEntry {
                peer,
                from,
                content,
                ts,
                retries: 0,
                created_at: Instant::now(),
            },
        );
    }

    /// Call when the recipient's ACK (`ChatResponse`) arrives.
    /// Returns the delivered entry so the caller can log stats.
    pub fn ack(&mut self, id: &OutboundRequestId) -> Option<PendingEntry> {
        self.pending.remove(id)
    }

    /// Call when an outbound failure is reported.
    ///
    /// Removes the entry from the tracker and returns it with `retries`
    /// incremented.  The caller should check `entry.retries`:
    ///   - `<= MAX_RETRIES` → re-send now, then `retrack()` with the new id.
    ///   - `>  MAX_RETRIES` → hand off to the offline queue.
    pub fn fail(&mut self, id: &OutboundRequestId) -> Option<PendingEntry> {
        self.pending.remove(id).map(|mut entry| {
            entry.retries += 1;
            entry
        })
    }

    /// Re-register a retried message under its new request id.
    pub fn retrack(&mut self, new_id: OutboundRequestId, entry: PendingEntry) {
        self.pending.insert(new_id, entry);
    }

    /// How many messages are currently awaiting ACK.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// True if no messages are in flight.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl Default for PendingTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_id(n: u64) -> OutboundRequestId {
        // OutboundRequestId wraps a u64 internally (libp2p 0.55)
        unsafe { std::mem::transmute(n) }
    }

    fn fake_peer() -> PeerId {
        "12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA"
            .parse()
            .unwrap()
    }

    #[test]
    fn track_and_ack() {
        let mut tracker = PendingTracker::new();
        let id = fake_id(1);
        let peer = fake_peer();
        tracker.track(id, peer, "alice".into(), "hello".into(), 42);
        assert_eq!(tracker.len(), 1);

        let entry = tracker.ack(&id).unwrap();
        assert_eq!(entry.content, "hello");
        assert_eq!(entry.retries, 0);
        assert_eq!(tracker.len(), 0);
        // Double-ack → None
        assert!(tracker.ack(&id).is_none());
    }

    #[test]
    fn fail_and_retry() {
        let mut tracker = PendingTracker::new();
        let id1 = fake_id(1);
        let peer = fake_peer();

        tracker.track(id1, peer, "alice".into(), "ping".into(), 1);
        assert_eq!(tracker.len(), 1);

        // First failure → should retry
        let entry = tracker.fail(&id1).unwrap();
        assert_eq!(entry.retries, 1);
        assert_eq!(tracker.len(), 0);

        // Retrack with new id
        let id2 = fake_id(2);
        tracker.retrack(id2, entry);
        assert_eq!(tracker.len(), 1);

        let entry = tracker.ack(&id2).unwrap();
        assert_eq!(entry.retries, 1); // retry count preserved
    }

    #[test]
    fn exhaust_retries() {
        let mut tracker = PendingTracker::new();
        let ids: Vec<_> = (1u64..=4).map(fake_id).collect();
        let peer = fake_peer();

        // Track initial send
        tracker.track(ids[0], peer, "a".into(), "msg".into(), 0);

        // Fail 3 times (retries 1,2,3) — still within MAX_RETRIES
        for i in 0..3 {
            let entry = tracker.fail(&ids[i]).unwrap();
            assert_eq!(entry.retries, (i + 1) as u32);
            if i < 2 {
                let next_id = ids[i + 1];
                tracker.retrack(next_id, entry);
            } else {
                // 3rd retry — this IS the last allowed retry (retries=3, =MAX_RETRIES)
                // caller should re-send once more (this is the 3rd retry)
                // but fail on that one will be retries=4 > MAX → exhausted
                tracker.retrack(ids[3], entry);
            }
        }

        // This 4th failure exceeds MAX_RETRIES
        let entry = tracker.fail(&ids[3]).unwrap();
        assert_eq!(entry.retries, 4);
        assert!(entry.retries > MAX_RETRIES);
    }

    #[test]
    fn fail_untracked_is_none() {
        let mut tracker = PendingTracker::new();
        assert!(tracker.fail(&fake_id(999)).is_none());
    }
}
