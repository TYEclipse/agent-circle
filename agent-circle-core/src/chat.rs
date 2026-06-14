//! Chat protocol — 1-on-1 messaging over QUIC

use serde::{Deserialize, Serialize};

/// Default message TTL: 7 days in seconds.
const DEFAULT_TTL_SECS: i64 = 7 * 24 * 3600; // 604800

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Sender's DID
    pub from: String,
    /// Message content
    pub content: String,
    /// Unix timestamp (seconds)
    pub ts: i64,
    /// Unique message id for deduplication — retries reuse the same id
    pub msg_id: u64,
    /// Unix timestamp (seconds) when this message expires.
    /// After this time the message should not be retried or stored.
    pub ttl: i64,
    /// Monotonic sequence number per sender, used for ordering.
    /// Resets on daemon restart; receiver resets on connection establish.
    pub seq: u64,
    /// S10R104 — Optional service invocation: when set, this is a call
    /// to a remote service rather than a human chat message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceCall>,
}

/// S10R104 — A service invocation payload attached to a ChatRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCall {
    /// The service identifier (e.g. "weather-v1").
    pub service_id: String,
    /// The method to call on the service (e.g. "forecast").
    pub method: String,
    /// Arbitrary JSON parameters for the call.
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub ack: bool,
}

/// Generate a new unique message id (random u64).
/// Collision probability ~ 1/2^64 per pair — negligible.
pub fn new_msg_id() -> u64 {
    rand::random()
}

/// Default TTL: 7 days from now (unix timestamp).
pub fn default_ttl() -> i64 {
    chrono::Utc::now().timestamp() + DEFAULT_TTL_SECS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_msg_id_nonzero() {
        let id = new_msg_id();
        assert!(id > 0, "msg_id should be positive");
    }

    #[test]
    fn test_new_msg_id_unique() {
        // 100 calls, all unique (statistically certain with u64 random)
        let ids: Vec<u64> = (0..100).map(|_| new_msg_id()).collect();
        let mut dedup = std::collections::HashSet::new();
        for id in &ids {
            assert!(dedup.insert(*id), "msg_id collision: {id}");
        }
        assert_eq!(dedup.len(), 100);
    }

    #[test]
    fn test_default_ttl_is_future() {
        let now = chrono::Utc::now().timestamp();
        let ttl = default_ttl();
        // TTL should be ~7 days from now (±5s tolerance)
        let expected = now + 7 * 24 * 3600;
        let delta = (ttl - expected).abs();
        assert!(delta < 5, "TTL delta too large: {delta}s");
    }

    #[test]
    fn test_chat_request_serde_roundtrip() {
        let req = ChatRequest {
            from: "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK".into(),
            content: "Hello, world! 🚀".into(),
            ts: 1718000000,
            msg_id: 42,
            ttl: 1718604800,
            seq: 7,
            service: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ChatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from, req.from);
        assert_eq!(back.content, req.content);
        assert_eq!(back.ts, req.ts);
        assert_eq!(back.msg_id, req.msg_id);
        assert_eq!(back.ttl, req.ttl);
        assert_eq!(back.seq, req.seq);
    }

    #[test]
    fn test_chat_response_serde_roundtrip() {
        let resp = ChatResponse { ack: true };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"ack":true}"#);

        let back: ChatResponse = serde_json::from_str(&json).unwrap();
        assert!(back.ack);

        // Also test ack=false
        let resp2 = ChatResponse { ack: false };
        let json2 = serde_json::to_string(&resp2).unwrap();
        let back2: ChatResponse = serde_json::from_str(&json2).unwrap();
        assert!(!back2.ack);
    }

    #[test]
    fn test_chat_request_debug() {
        let req = ChatRequest {
            from: "alice".into(),
            content: "hi".into(),
            ts: 1,
            msg_id: 2,
            ttl: 3,
            seq: 4,
            service: None,
        };
        let debug = format!("{req:?}");
        assert!(debug.contains("alice"));
        assert!(debug.contains("hi"));
        assert!(debug.contains("msg_id: 2"));
        assert!(debug.contains("seq: 4"));
    }

    #[test]
    fn test_chat_request_clone() {
        let req = ChatRequest {
            from: "bob".into(),
            content: "test".into(),
            ts: 0,
            msg_id: 1,
            ttl: 2,
            seq: 3,
            service: None,
        };
        let cloned = req.clone();
        assert_eq!(cloned.from, req.from);
        assert_eq!(cloned.content, req.content);
        assert_eq!(cloned.msg_id, req.msg_id);
        assert_eq!(cloned.seq, req.seq);
    }

    #[test]
    fn test_chat_request_optional_fields() {
        // msg_id, ttl, seq can be 0 — verify serde handles them
        let req = ChatRequest {
            from: "minimal".into(),
            content: String::new(),
            ts: 0,
            msg_id: 0,
            ttl: 0,
            seq: 0,
            service: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ChatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from, "minimal");
        assert_eq!(back.content, "");
        assert_eq!(back.msg_id, 0);
        assert_eq!(back.ttl, 0);
        assert_eq!(back.seq, 0);
    }

    #[test]
    fn test_chat_response_no_extra_fields() {
        // ChatResponse only has 'ack' — verify serde is strict enough
        let json = r#"{"ack":true,"extra":42}"#;
        let result: Result<ChatResponse, _> = serde_json::from_str(json);
        // serde_json by default ignores unknown fields with derive
        // Actually serde_json with derive Deserialize ignores extra fields by default.
        // This is expected behaviour — test it documents that.
        let resp = result.unwrap();
        assert!(resp.ack);
    }
}
