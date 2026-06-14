//! Publication module — 公众号 (Public Account) data model
//!
//! Models for agent services that publish content to subscribers,
//! analogous to WeChat Official Accounts.  Keeps the minimal-dependency
//! contract of `agent-circle-core` (serde-only, no libp2p).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Content types ──────────────────────────────────────────────────

/// Supported content formats for publications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    #[default]
    Text,
    Markdown,
}

// ── Publication ────────────────────────────────────────────────────

/// A single published message from a service (公众号文章).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    /// Unique publication ID (UUID v4).
    pub id: String,
    /// DID of the publishing service.
    pub service_id: String,
    /// Title of the publication.
    pub title: String,
    /// Body content.
    pub content: String,
    /// Content format.
    #[serde(default)]
    pub content_type: ContentType,
    /// ISO-8601 publication timestamp.
    pub timestamp: DateTime<Utc>,
    /// Monotonic version number (increments per publish).
    pub version: u32,
    /// Ed25519 signature over (id || service_id || title || content || version.to_be_bytes()),
    /// hex-encoded.  Subscribers use the service's public key to verify.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
}

// ── PublicationHistory ─────────────────────────────────────────────

/// Ordered publication history for a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationHistory {
    /// DID of the service.
    pub service_id: String,
    /// Publications in reverse chronological order (newest first).
    #[serde(default)]
    pub publications: Vec<Publication>,
    /// When this history was last modified.
    pub last_updated: DateTime<Utc>,
}

impl PublicationHistory {
    /// Create an empty history for a service.
    pub fn new(service_id: String) -> Self {
        Self {
            service_id,
            publications: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Append a publication (at the front — reverse chronological).
    pub fn push(&mut self, publication: Publication) {
        self.publications.insert(0, publication);
        self.last_updated = Utc::now();
    }

    /// Number of publications.
    pub fn len(&self) -> usize {
        self.publications.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.publications.is_empty()
    }
}

// ── Subscriber ─────────────────────────────────────────────────────

/// A subscriber to a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    /// Peer ID (DID) of the subscriber.
    pub peer_id: String,
    /// When the subscription was created.
    pub subscribed_at: DateTime<Utc>,
    /// Whether the subscription has been approved (if the service
    /// requires approval).
    #[serde(default = "default_approved")]
    pub approved: bool,
}

fn default_approved() -> bool {
    true
}

// ── SubscriberList ─────────────────────────────────────────────────

/// Subscription list for a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberList {
    /// DID of the service.
    pub service_id: String,
    /// Active subscribers.
    #[serde(default)]
    pub subscribers: Vec<Subscriber>,
    /// When the list was last modified.
    pub last_updated: DateTime<Utc>,
}

impl SubscriberList {
    /// Create an empty subscriber list.
    pub fn new(service_id: String) -> Self {
        Self {
            service_id,
            subscribers: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Add a subscriber. Returns `false` if already subscribed.
    pub fn subscribe(&mut self, peer_id: &str) -> bool {
        if self.subscribers.iter().any(|s| s.peer_id == peer_id) {
            return false;
        }
        self.subscribers.push(Subscriber {
            peer_id: peer_id.to_string(),
            subscribed_at: Utc::now(),
            approved: true,
        });
        self.last_updated = Utc::now();
        true
    }

    /// Remove a subscriber. Returns `false` if not subscribed.
    pub fn unsubscribe(&mut self, peer_id: &str) -> bool {
        let len_before = self.subscribers.len();
        self.subscribers.retain(|s| s.peer_id != peer_id);
        if self.subscribers.len() != len_before {
            self.last_updated = Utc::now();
            true
        } else {
            false
        }
    }

    /// Get the count of approved subscribers.
    pub fn active_count(&self) -> usize {
        self.subscribers.iter().filter(|s| s.approved).count()
    }

    /// Total subscriber count.
    pub fn len(&self) -> usize {
        self.subscribers.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.subscribers.is_empty()
    }
}

// ── Permission model ───────────────────────────────────────────────

/// Access control model for a service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServicePermission {
    /// Anyone can subscribe and receive publications.
    #[default]
    Public,
    /// Subscription requires owner approval.
    ApprovalRequired,
    /// Only whitelisted peers can subscribe.
    Whitelist(Vec<String>),
}

// ── Wire protocol types ────────────────────────────────────────────

/// Request to publish content to a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    /// DID of the publishing service.
    pub service_id: String,
    /// Title of the publication.
    pub title: String,
    /// Body content.
    pub content: String,
    /// Content format.
    #[serde(default)]
    pub content_type: ContentType,
    /// Next expected version (for idempotency).
    pub expected_version: u32,
    /// Ed25519 signature proving ownership of the service identity.
    pub signature: String,
}

/// Response to a publish request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    /// The created publication (if successful).
    pub publication: Option<Publication>,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Request to subscribe to a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    /// DID of the service to subscribe to.
    pub service_id: String,
    /// DID of the subscriber.
    pub subscriber_did: String,
}

/// Response to a subscription request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResponse {
    /// Whether the subscription was accepted.
    pub accepted: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Request to discover a service's publications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverRequest {
    /// DID of the service to query.
    pub service_id: String,
    /// Maximum number of recent publications to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination.
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    20
}

/// Response to a discover request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverResponse {
    /// Publications (reverse chronological).
    pub publications: Vec<Publication>,
    /// Total number of publications available.
    pub total: usize,
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscriber_list_add_remove() {
        let mut list = SubscriberList::new("did:key:test123".into());
        assert!(list.is_empty());

        assert!(list.subscribe("did:key:alice"));
        assert!(!list.subscribe("did:key:alice")); // duplicate
        assert_eq!(list.len(), 1);
        assert_eq!(list.active_count(), 1);

        assert!(list.subscribe("did:key:bob"));
        assert_eq!(list.len(), 2);

        assert!(list.unsubscribe("did:key:alice"));
        assert_eq!(list.len(), 1);

        assert!(!list.unsubscribe("did:key:alice")); // already removed
    }

    #[test]
    fn test_publication_history_order() {
        let mut history = PublicationHistory::new("did:key:svc".into());
        assert!(history.is_empty());

        let p1 = Publication {
            id: "1".into(),
            service_id: "did:key:svc".into(),
            title: "First post".into(),
            content: "Hello".into(),
            content_type: ContentType::Text,
            timestamp: Utc::now(),
            version: 1,
            signature: String::new(),
        };

        let p2 = Publication {
            id: "2".into(),
            service_id: "did:key:svc".into(),
            title: "Second post".into(),
            content: "World".into(),
            content_type: ContentType::Markdown,
            timestamp: Utc::now(),
            version: 2,
            signature: String::new(),
        };

        history.push(p1);
        history.push(p2);
        assert_eq!(history.len(), 2);
        // Newest first
        assert_eq!(history.publications[0].version, 2);
        assert_eq!(history.publications[1].version, 1);
    }

    #[test]
    fn test_service_permission_defaults() {
        assert_eq!(ServicePermission::default(), ServicePermission::Public);
    }

    #[test]
    fn test_publication_serde_roundtrip() {
        let pub_msg = Publication {
            id: "test-id".into(),
            service_id: "did:key:svc".into(),
            title: "Hello World".into(),
            content: "This is a **markdown** publication.".into(),
            content_type: ContentType::Markdown,
            timestamp: Utc::now(),
            version: 1,
            signature: String::new(),
        };

        let json = serde_json::to_string(&pub_msg).unwrap();
        let decoded: Publication = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "test-id");
        assert_eq!(decoded.content_type, ContentType::Markdown);
        assert_eq!(decoded.version, 1);
    }
}
