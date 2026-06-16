// 超大消息分片与重组
//
// 验收：消息超过协议 MTU 时自动分片，接收方重组后交付。
// 默认分片阈值 64KB，与 libp2p 流控兼容。

use agent_circle_core::chat::{ChatRequest, ServiceCall};
use std::collections::HashMap;

/// Maximum payload size per fragment (64 KB, safe for libp2p streams).
pub const FRAGMENT_SIZE: usize = 64 * 1024;

/// Time-to-live for fragment assembly sessions (seconds).
/// Stale fragments are discarded after this duration.
pub const FRAGMENT_TTL_SECS: i64 = 60;

/// Fragment metadata embedded in each ChatRequest.
#[derive(Debug, Clone)]
pub struct FragmentInfo {
    /// Unique message ID shared across all fragments.
    pub msg_id: u64,
    /// Total number of fragments for this message.
    pub total: u32,
    /// Zero-based index of this fragment.
    pub index: u32,
}

impl FragmentInfo {
    /// Pack fragment metadata into a compact string for embedding
    /// in `ServiceCall::fragment_info`.
    pub fn encode(&self) -> String {
        format!("FRAG:{}:{}:{}", self.msg_id, self.index, self.total)
    }

    /// Parse fragment metadata from a ServiceCall's fragment_info field.
    pub fn from_service_call(sc: &ServiceCall) -> Option<Self> {
        sc.fragment_info.as_deref().and_then(FragmentInfo::decode)
    }

    /// Parse fragment metadata from an encoded string.
    pub fn decode(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.strip_prefix("FRAG:")?.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(FragmentInfo {
            msg_id: parts[0].parse().ok()?,
            index: parts[1].parse().ok()?,
            total: parts[2].parse().ok()?,
        })
    }

    pub fn is_first(&self) -> bool {
        self.index == 0
    }

    pub fn is_last(&self) -> bool {
        self.index + 1 == self.total
    }
}

/// Splits a large message payload into fragments.
pub fn split_large_message(
    from: &str,
    content: &str,
    msg_id: u64,
    ts: i64,
    ttl: i64,
) -> Vec<ChatRequest> {
    if content.len() <= FRAGMENT_SIZE {
        // Small enough — single message
        return vec![ChatRequest {
            from: from.to_string(),
            content: content.to_string(),
            ts,
            msg_id,
            ttl,
            seq: 0, // caller sets seq
            service: None,
        }];
    }

    let bytes = content.as_bytes();
    let total_fragments = bytes.len().div_ceil(FRAGMENT_SIZE) as u32;
    let mut fragments = Vec::with_capacity(total_fragments as usize);

    for i in 0..total_fragments {
        let start = i as usize * FRAGMENT_SIZE;
        let end = std::cmp::min(start + FRAGMENT_SIZE, bytes.len());
        let chunk = String::from_utf8_lossy(&bytes[start..end]).to_string();

        let info = FragmentInfo {
            msg_id,
            total: total_fragments,
            index: i,
        };

        fragments.push(ChatRequest {
            from: from.to_string(),
            content: chunk,
            ts,
            msg_id,
            ttl,
            seq: 0, // caller sets seq
            service: Some(ServiceCall {
                service_id: "fragment".into(),
                method: "v1".into(),
                params: serde_json::Value::Null,
                fragment_info: Some(info.encode()),
            }),
        });
    }

    fragments
}

/// State for reassembling a fragmented message.
struct ReassemblySession {
    fragments: HashMap<u32, String>,
    total: u32,
    received: u32,
    started_at: i64,
}

/// Manages reassembly of fragmented messages from multiple senders.
pub struct FragmentReassembler {
    /// Keyed by (sender_peer_id, msg_id)
    sessions: HashMap<(String, u64), ReassemblySession>,
}

impl FragmentReassembler {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Ingest a potentially fragmented ChatRequest.
    /// Returns:
    /// - `None` if the fragment was buffered (not yet complete)
    /// - `Some(content)` when all fragments are received and reassembled
    pub fn ingest(&mut self, sender: &str, msg: &ChatRequest) -> Option<String> {
        // Check if this is a fragmented message
        let info = match msg
            .service
            .as_ref()
            .and_then(FragmentInfo::from_service_call)
        {
            Some(info) => info,
            None => {
                // Not a fragment — deliver as-is
                return Some(msg.content.clone());
            }
        };

        let key = (sender.to_string(), info.msg_id);
        let now = msg.ts;

        let session = self
            .sessions
            .entry(key.clone())
            .or_insert_with(|| ReassemblySession {
                fragments: HashMap::new(),
                total: info.total,
                received: 0,
                started_at: now,
            });

        // Check for stale session
        if now - session.started_at > FRAGMENT_TTL_SECS {
            // Reset
            session.fragments.clear();
            session.received = 0;
            session.started_at = now;
            session.total = info.total;
        }

        // Insert fragment (overwrite duplicates silently)
        if session
            .fragments
            .insert(info.index, msg.content.clone())
            .is_none()
        {
            session.received += 1;
        }

        // Check if complete
        if session.received >= session.total {
            let mut result = String::new();
            for i in 0..session.total {
                if let Some(chunk) = session.fragments.get(&i) {
                    result.push_str(chunk);
                } else {
                    // Missing fragment — shouldn't happen, but guard
                    return None;
                }
            }
            self.sessions.remove(&key);
            Some(result)
        } else {
            None
        }
    }

    /// Get the number of active reassembly sessions (diagnostic).
    pub fn active_sessions(&self) -> usize {
        self.sessions.len()
    }

    /// Get total buffered fragments across all sessions (diagnostic).
    pub fn total_buffered_fragments(&self) -> usize {
        self.sessions.values().map(|s| s.received as usize).sum()
    }

    /// Clean up stale sessions (older than TTL).
    pub fn purge_stale(&mut self, now: i64) -> usize {
        let before = self.sessions.len();
        self.sessions
            .retain(|_, s| now - s.started_at <= FRAGMENT_TTL_SECS);
        before - self.sessions.len()
    }
}

impl Default for FragmentReassembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r174a_no_fragment_for_small_message() {
        let frags = split_large_message("alice", "hello", 1, 0, 999);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].service.is_none());
        assert_eq!(frags[0].content, "hello");
    }

    #[test]
    fn r174b_split_large_message() {
        let big_content = "A".repeat(FRAGMENT_SIZE * 3 + 100); // slightly more than 3 fragments
        let frags = split_large_message("alice", &big_content, 42, 0, 999);

        assert_eq!(frags.len(), 4, "Should produce 4 fragments");
        let info0 = FragmentInfo::from_service_call(frags[0].service.as_ref().unwrap()).unwrap();
        let info3 = FragmentInfo::from_service_call(frags[3].service.as_ref().unwrap()).unwrap();
        assert_eq!(info0.index, 0);
        assert_eq!(info0.total, 4);
        assert_eq!(info3.index, 3);
        assert_eq!(info3.total, 4);
    }

    #[test]
    fn r174c_reassemble_fragments() {
        let content = "HELLO".repeat(50000); // ~250KB, 4 fragments
        let total_len = content.len();

        let frags = split_large_message("bob", &content, 100, 0, 999);
        assert!(frags.len() > 1, "Should be fragmented");

        let mut reassembler = FragmentReassembler::new();
        for (i, frag) in frags.iter().enumerate() {
            let result = reassembler.ingest("bob", frag);
            if i < frags.len() - 1 {
                assert!(result.is_none(), "Should not complete until last fragment");
            } else {
                assert!(result.is_some(), "Should complete on last fragment");
                assert_eq!(result.unwrap().len(), total_len);
            }
        }
    }

    #[test]
    fn r174d_reassemble_out_of_order() {
        let content = "WORLD".repeat(20000);
        let total_len = content.len();
        let frags = split_large_message("carol", &content, 200, 0, 999);

        assert!(frags.len() >= 2);

        let mut reassembler = FragmentReassembler::new();

        // Deliver all fragments in reverse order; the last one to arrive
        // (which was split first) should trigger completion
        let mut final_delivered = false;
        for frag in frags.iter().rev() {
            let result = reassembler.ingest("carol", frag);
            if let Some(assembled) = result {
                final_delivered = true;
                assert_eq!(assembled.len(), total_len);
            }
        }
        assert!(final_delivered, "Should eventually complete");
    }

    #[test]
    fn r174e_non_fragmented_messages_pass_through() {
        let mut reassembler = FragmentReassembler::new();
        let msg = ChatRequest {
            from: "dave".into(),
            content: "small message".into(),
            ts: 0,
            msg_id: 1,
            ttl: 999,
            seq: 1,
            service: None,
        };
        let result = reassembler.ingest("dave", &msg);
        assert_eq!(result, Some("small message".to_string()));
    }

    #[test]
    fn r174f_fragment_info_encode_decode() {
        let info = FragmentInfo {
            msg_id: 12345,
            total: 5,
            index: 2,
        };
        let encoded = info.encode();
        let decoded = FragmentInfo::decode(&encoded).unwrap();
        assert_eq!(decoded.msg_id, 12345);
        assert_eq!(decoded.total, 5);
        assert_eq!(decoded.index, 2);
        assert!(!decoded.is_first());
        assert!(!decoded.is_last());
    }

    #[test]
    fn r174g_is_first_and_last() {
        let first = FragmentInfo {
            msg_id: 1,
            total: 3,
            index: 0,
        };
        assert!(first.is_first());
        assert!(!first.is_last());

        let last = FragmentInfo {
            msg_id: 1,
            total: 3,
            index: 2,
        };
        assert!(!last.is_first());
        assert!(last.is_last());
    }

    #[test]
    fn r174h_invalid_frag_string() {
        assert!(FragmentInfo::decode("not-a-fragment").is_none());
        assert!(FragmentInfo::decode("FRAG:").is_none());
        assert!(FragmentInfo::decode("FRAG:abc:0:2").is_none()); // non-numeric msg_id
        assert!(FragmentInfo::decode("FRAG:1:x:2").is_none()); // non-numeric index
    }

    #[test]
    fn r174i_purge_stale_sessions() {
        let mut reassembler = FragmentReassembler::new();
        let content = "DATA".repeat(20000);
        let frags = split_large_message("eve", &content, 500, 0, 999);

        // Ingest first fragment only
        reassembler.ingest("eve", &frags[0]);
        assert_eq!(reassembler.active_sessions(), 1);

        // Purge with current time far in the future
        let purged = reassembler.purge_stale(1000); // 1000s later, > 60s TTL
        assert_eq!(purged, 1);
        assert_eq!(reassembler.active_sessions(), 0);
    }

    #[test]
    fn r174j_duplicate_fragments_handled() {
        let content = "DUPE".repeat(20000);
        let frags = split_large_message("frank", &content, 600, 0, 999);
        let mut reassembler = FragmentReassembler::new();

        // Send fragment 0 twice
        assert!(reassembler.ingest("frank", &frags[0]).is_none());
        assert!(reassembler.ingest("frank", &frags[0]).is_none()); // duplicate ignored
        assert_eq!(reassembler.total_buffered_fragments(), 1);

        // Deliver rest
        for frag in &frags[1..] {
            reassembler.ingest("frank", frag);
        }
        assert_eq!(reassembler.active_sessions(), 0);
    }

    #[test]
    fn r174k_exact_fragment_size_boundary() {
        // Exactly one fragment worth
        let content = "X".repeat(FRAGMENT_SIZE);
        let frags = split_large_message("grace", &content, 700, 0, 999);
        assert_eq!(frags.len(), 1, "Exactly fragment_size should not split");
    }

    #[test]
    fn r174l_one_byte_over_boundary() {
        let content = "Y".repeat(FRAGMENT_SIZE + 1);
        let frags = split_large_message("heidi", &content, 800, 0, 999);
        assert_eq!(frags.len(), 2, "One byte over should produce 2 fragments");
        assert_eq!(frags[0].content.len(), FRAGMENT_SIZE);
        assert_eq!(frags[1].content.len(), 1);
    }
}
