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
