//! Message deduplication — prevents double-processing when retries
//! deliver the same message more than once.
//!
//! Combined with ACK + retry (S02R15), this gives "effectively-once"
//! delivery semantics: the recipient may receive duplicates at the
//! transport layer, but the dedup filter ensures each message is
//! only processed once at the application layer.

use std::collections::{HashSet, VecDeque};

/// Maximum number of message IDs to track before evicting the oldest.
const MAX_TRACKED: usize = 10_000;

/// Tracks recently-seen message IDs so retransmissions are silently
/// acknowledged but not re-processed.
pub struct DedupFilter {
    seen: HashSet<u64>,
    order: VecDeque<u64>,
}

impl DedupFilter {
    pub fn new() -> Self {
        Self {
            seen: HashSet::with_capacity(MAX_TRACKED),
            order: VecDeque::with_capacity(MAX_TRACKED),
        }
    }

    /// Returns `true` if `id` has already been seen (duplicate).
    /// Otherwise records it and returns `false`.
    pub fn is_dup(&mut self, id: u64) -> bool {
        if self.seen.contains(&id) {
            return true;
        }
        // Evict oldest entry when at capacity
        if self.seen.len() >= MAX_TRACKED {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        self.seen.insert(id);
        self.order.push_back(id);
        false
    }

    /// How many IDs are currently tracked.
    #[allow(dead_code)] // used in monitoring
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// True if no IDs have been seen.
    #[allow(dead_code)] // used in monitoring
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl Default for DedupFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_is_not_dup() {
        let mut f = DedupFilter::new();
        assert!(!f.is_dup(42));
    }

    #[test]
    fn second_is_dup() {
        let mut f = DedupFilter::new();
        f.is_dup(1);
        assert!(f.is_dup(1));
    }

    #[test]
    fn different_ids_not_dup() {
        let mut f = DedupFilter::new();
        assert!(!f.is_dup(1));
        assert!(!f.is_dup(2));
        assert!(!f.is_dup(3));
    }

    #[test]
    fn evicts_oldest_when_full() {
        let mut f = DedupFilter::new();
        // Fill to capacity
        for i in 0..MAX_TRACKED as u64 {
            assert!(!f.is_dup(i));
        }
        assert_eq!(f.len(), MAX_TRACKED);

        // This should evict id 0
        let new_id = MAX_TRACKED as u64;
        assert!(!f.is_dup(new_id));
        assert_eq!(f.len(), MAX_TRACKED);

        // id 0 is evicted — no longer a dup
        assert!(!f.is_dup(0));
    }
}
