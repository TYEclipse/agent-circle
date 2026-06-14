//! Test fixture factory — reduce boilerplate in tests.
//!
//! Every test that does `Identity::generate()` or manually constructs a
//! ChatRequest should consider using a fixture instead.
//!
//! ## Usage
//!
//! ```ignore
//! use common::fixtures::*;
//!
//! let id = random_identity();
//! let msg = valid_chat_request("test", "hello");
//! let card = random_agent_card();
//! ```
#![allow(dead_code)]

use agent_circle::chat::ChatRequest;
use agent_circle::identity::{AgentCard, Identity};
use agent_circle::timeline::{Timeline, TimelineNode};

// ── Identity ────────────────────────────────────────────────────────

/// Generate a random identity (system entropy).
pub fn random_identity() -> Identity {
    Identity::generate()
}

/// Deterministic identity from a 32-byte seed. Useful when you need
/// reproducible keys across test runs.
pub fn seeded_identity(seed: &[u8; 32]) -> Identity {
    Identity::from_seed(seed).expect("seeded identity should always succeed")
}

// ── Chat ────────────────────────────────────────────────────────────

/// A valid, minimal chat request with sensible defaults.
pub fn valid_chat_request(did: &str, content: &str) -> ChatRequest {
    ChatRequest {
        from: did.to_string(),
        content: content.to_string(),
        ts: 1_700_000_000,
        msg_id: 1,
        ttl: 9_999_999_999,
        seq: 1,
    }
}

/// Chat request with a specific sequence number.
pub fn chat_request_seq(did: &str, content: &str, seq: u64) -> ChatRequest {
    ChatRequest {
        from: did.to_string(),
        content: content.to_string(),
        ts: 1_700_000_000,
        msg_id: seq, // reuse seq as msg_id for simplicity
        ttl: 9_999_999_999,
        seq,
    }
}

/// A chat request with empty content — edge case.
pub fn empty_chat_request(did: &str) -> ChatRequest {
    valid_chat_request(did, "")
}

/// A chat request with zero values in every numeric field — boundary test.
pub fn zeroed_chat_request() -> ChatRequest {
    ChatRequest {
        from: "zero".to_string(),
        content: "".to_string(),
        ts: 0,
        msg_id: 0,
        ttl: 0,
        seq: 0,
    }
}

// ── Agent Card ──────────────────────────────────────────────────────

/// Generate a valid AgentCard from a random identity.
pub fn random_agent_card() -> AgentCard {
    let id = random_identity();
    agent_card_for(&id)
}

/// Generate a valid AgentCard for a specific identity.
pub fn agent_card_for(id: &Identity) -> AgentCard {
    id.create_card(
        "test-agent",
        "human:test",
        "test-model",
        &["chat".into(), "timeline".into()],
    )
    .expect("fixture agent_card should always succeed")
}

// ── Timeline ────────────────────────────────────────────────────────

/// A genesis timeline node signed by the given identity.
pub fn genesis_node(id: &Identity, content: &str) -> TimelineNode {
    Timeline::genesis(id, content).expect("genesis fixture should always succeed")
}

/// An empty timeline.
pub fn empty_timeline() -> Timeline {
    Timeline::new()
}

/// A timeline with `n` nodes appended sequentially by the same identity.
pub fn multi_node_timeline(id: &Identity, n: usize) -> Timeline {
    let mut tl = Timeline::new();
    for i in 0..n {
        tl.append(id, &format!("post-{i}"))
            .expect("append fixture should always succeed");
    }
    tl
}

// ── Malformed / error-path ──────────────────────────────────────────

/// A DID string that is definitely not a valid DID:key.
pub fn invalid_did() -> String {
    "did:nope:garbage".to_string()
}

/// An obviously invalid base64 signature (wrong character set + length).
pub fn malformed_signature() -> String {
    "!!!not-valid-base64!!!".to_string()
}

/// A ChatRequest with a tampered DID — the `from` field doesn't match
/// the key that signed the timeline message.
pub fn chat_request_from_did(did: &str) -> ChatRequest {
    ChatRequest {
        from: did.to_string(),
        content: "tampered".to_string(),
        ts: 1_700_000_000,
        msg_id: 999,
        ttl: 9_999_999_999,
        seq: 999,
    }
}

// ── Convenience ─────────────────────────────────────────────────────

/// Standard sleep duration for integration tests (500ms).
pub const SETTLE_MS: u64 = 500;
