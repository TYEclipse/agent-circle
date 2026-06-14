//! Chat protocol — 1-on-1 messaging over QUIC

use serde::{Deserialize, Serialize};

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
