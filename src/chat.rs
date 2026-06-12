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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub ack: bool,
}
