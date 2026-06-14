//! Message sequence ordering — ensures messages from each sender
//! are delivered in order even when retransmissions reorder them.
//!
//! Per-sender state resets on `ConnectionEstablished` (because
//! the sender's sequence counter can reset on its own restart).

use crate::chat::ChatRequest;
use libp2p::PeerId;
use std::collections::{BTreeMap, HashMap};

/// Tracks the last seen sequence number per sender and buffers
/// out-of-order messages until the gap is filled.
pub struct SequenceTracker {
    /// Per-sender: the highest consecutive sequence number delivered.
    last_seq: HashMap<PeerId, u64>,
    /// Per-sender: buffered messages keyed by sequence number,
    /// stored in-order so we can flush contiguous runs.
    buffer: HashMap<PeerId, BTreeMap<u64, ChatRequest>>,
}

impl SequenceTracker {
    pub fn new() -> Self {
        Self {
            last_seq: HashMap::new(),
            buffer: HashMap::new(),
        }
    }

    /// Process an incoming message.  Returns the messages that should
    /// be delivered *now* — either just this one, or this one plus
    /// any buffered successors that now form a contiguous run.
    ///
    /// If the message is a duplicate or stale (seq <= last_seq),
    /// an empty vec is returned (the caller should still ACK).
    pub fn ingest(&mut self, peer: &PeerId, msg: ChatRequest) -> Vec<ChatRequest> {
        let last = self.last_seq.get(peer).copied().unwrap_or(0);

        if msg.seq <= last {
            // Duplicate or already delivered — skip (dedup layer handles this too)
            return vec![];
        }

        if msg.seq == last + 1 {
            // Perfect — exactly the next one
            self.last_seq.insert(*peer, msg.seq);
            let mut delivered = vec![msg];

            // Flush any buffered messages that are now contiguous
            if let Some(buf) = self.buffer.get_mut(peer) {
                let mut next = last + 2;
                while let Some(buffered) = buf.remove(&next) {
                    self.last_seq.insert(*peer, next);
                    delivered.push(buffered);
                    next += 1;
                }
                if buf.is_empty() {
                    self.buffer.remove(peer);
                }
            }
            delivered
        } else {
            // Gap detected — buffer it
            let buf = self.buffer.entry(*peer).or_default();
            buf.insert(msg.seq, msg);
            vec![]
        }
    }

    /// Reset tracking state for a peer (called on new connection).
    /// Any buffered messages for this peer are dropped — they belong
    /// to an old sequence epoch and will be retransmitted if needed.
    pub fn reset_peer(&mut self, peer: &PeerId) {
        self.last_seq.remove(peer);
        self.buffer.remove(peer);
    }

    /// Number of peers with buffered messages (diagnostic).
    #[allow(dead_code)]
    pub fn buffered_peer_count(&self) -> usize {
        self.buffer.len()
    }

    /// Total buffered messages (diagnostic).
    #[allow(dead_code)]
    pub fn buffered_msg_count(&self) -> usize {
        self.buffer.values().map(|b| b.len()).sum()
    }
}

impl Default for SequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(id: &str) -> PeerId {
        id.parse().unwrap()
    }

    fn msg(seq: u64, content: &str) -> ChatRequest {
        ChatRequest {
            from: "alice".into(),
            content: content.into(),
            ts: 1,
            msg_id: seq, // reuse seq as msg_id for test simplicity
            ttl: 9999999999,
            seq,
            service: None,
        }
    }

    #[test]
    fn in_order_delivery() {
        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        let result = st.ingest(&p, msg(1, "hello"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");

        let result = st.ingest(&p, msg(2, "world"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "world");
        assert_eq!(st.buffered_peer_count(), 0);
    }

    #[test]
    fn out_of_order_reorder() {
        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Msg 2 arrives first → buffered
        let result = st.ingest(&p, msg(2, "second"));
        assert!(result.is_empty());
        assert_eq!(st.buffered_msg_count(), 1);

        // Msg 1 arrives → flushes both
        let result = st.ingest(&p, msg(1, "first"));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "first");
        assert_eq!(result[1].content, "second");
        assert_eq!(st.buffered_msg_count(), 0);
    }

    #[test]
    fn duplicate_ignored() {
        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        st.ingest(&p, msg(1, "a"));
        st.ingest(&p, msg(2, "b"));

        let result = st.ingest(&p, msg(1, "dup a"));
        assert!(result.is_empty());

        let result = st.ingest(&p, msg(2, "dup b"));
        assert!(result.is_empty());
    }

    #[test]
    fn large_gap_fills() {
        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        // Msgs 5,3,4,6 arrive before 1,2
        st.ingest(&p, msg(5, "five"));
        st.ingest(&p, msg(3, "three"));
        st.ingest(&p, msg(4, "four"));
        st.ingest(&p, msg(6, "six"));
        assert_eq!(st.buffered_msg_count(), 4);

        // Msg 1 fills the first gap
        st.ingest(&p, msg(1, "one"));
        assert_eq!(st.buffered_msg_count(), 4); // still missing 2

        // Msg 2 arrives — flushes 2,3,4,5,6 all at once
        let result = st.ingest(&p, msg(2, "two"));
        assert_eq!(result.len(), 5); // 2,3,4,5,6
        assert_eq!(result[0].content, "two");
        assert_eq!(result[4].content, "six");
        assert_eq!(st.buffered_msg_count(), 0);

        // Next message continues from 7
        let result = st.ingest(&p, msg(7, "seven"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "seven");
    }

    #[test]
    fn reset_on_new_connection() {
        let mut st = SequenceTracker::new();
        let p = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");

        st.ingest(&p, msg(5, "five"));
        assert_eq!(st.buffered_msg_count(), 1);

        // Reset — simulates sender crash+restart, seq back to 1
        st.reset_peer(&p);
        assert_eq!(st.buffered_msg_count(), 0);

        // New epoch messages accepted
        let result = st.ingest(&p, msg(1, "new one"));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn per_peer_isolation() {
        let mut st = SequenceTracker::new();
        let a = peer("12D3KooWEfmPmYbLgT5KXLQFnSwZKHWzsiMQinLuB9DVCfhHyKxA");
        let b = peer("12D3KooWHdiAx96Qf1mFXKwJY3W5JSQvq4PJ9xrXp7A1SzUFrHMn");

        st.ingest(&a, msg(3, "a-three")); // a buffered (missing 1,2)
        st.ingest(&b, msg(1, "b-one")); // b delivered immediately
        assert_eq!(st.buffered_peer_count(), 1); // only a buffered
        let b_seq = st.ingest(&b, msg(2, "b-two"));
        assert_eq!(b_seq.len(), 1); // b delivered
    }
}
