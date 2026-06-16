//! Protocol version constants and SemVer negotiation strategy.
//!
//! All wire-protocol identifiers are defined here so version bumps
//! happen in one place.  The libp2p `identify` protocol advertises
//! support automatically; peers negotiate the highest common version.
//!
//! ## SemVer Policy (R82)
//!
//!   MAJOR.MINOR.PATCH  (e.g. 0.2.0)
//!
//!   MAJOR (0.x → x.0):  Wire-format breaking change.  Old peers
//!     see a new `/.../<N>.0` protocol string and connect via an
//!     older version if interop is still registered.
//!
//!   MINOR (x.N):  Backward-compatible feature addition (new field
//!     with default).  Protocol string unchanged; old peers ignore
//!     unknown fields thanks to serde `#[serde(default)]`.
//!
//!   PATCH (x.x.N):  Bug-fix, no protocol change.
//!
//! ## Interop table
//!
//!   When a node advertises both `/agent-circle/chat/0.1.0` and
//!   `/agent-circle/chat/0.2.0`, a 0.1.0 peer picks 0.1.0 while a
//!   0.2.0 peer picks 0.2.0.  Both sides communicate at the highest
//!   mutually-supported version.

/// Current application-level protocol version.
pub const VERSION: &str = "0.1.0";

// ── Protocol identifiers ──────────────────────────────────────────

/// Identify agent string reported to peers.
pub fn identify_agent() -> String {
    format!("/agent-circle/{VERSION}")
}

/// Chat request-response protocol (one-to-one messaging).
pub fn chat_protocol() -> String {
    format!("/agent-circle/chat/{VERSION}")
}

/// DHT record key for relay node discovery.
pub fn relay_dht_key() -> String {
    format!("/agent-circle/relays/{VERSION}")
}

/// GossipSub topic prefix for group chats.
pub fn group_topic_prefix() -> String {
    "agent-circle/group".to_string()
}

/// GossipSub topic for service discovery announcements.
/// Agents publish their services here; peers subscribe to discover them.
pub fn services_topic() -> String {
    format!("agent-circle/services/{VERSION}")
}

/// GossipSub topic for publication push.
/// Services publish new articles here; subscribers listen for updates.
pub fn publications_topic() -> String {
    format!("agent-circle/publications/{VERSION}")
}

/// Remote diagnostics request-response protocol.
pub fn doctor_protocol() -> String {
    format!("/agent-circle/doctor/{VERSION}")
}

// ── Future multi-version support ──────────────────────────────────

/// When we bump to 0.2.0, list ALL supported versions for backward
/// compatibility.  The request-response behaviour advertises every
/// entry and peers negotiate the highest common one.
///
/// Usage in `build_swarm`:
/// ```ignore
/// let protocols: Vec<_> = SUPPORTED_CHAT_PROTOCOLS
///     .iter()
///     .map(|v| (StreamProtocol::new(v.clone()), ProtocolSupport::Full))
///     .collect();
/// ```
#[allow(dead_code)] // reserved for future multi-version support (R81)
pub const SUPPORTED_CHAT_PROTOCOLS: &[&str] = &[
    "/agent-circle/chat/0.1.0",
    // "/agent-circle/chat/0.2.0",  // add when 0.2.0 is released
];
