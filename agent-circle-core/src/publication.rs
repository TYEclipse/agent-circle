//! Publication module — data model for published content
//!
//! Models for agent services that publish content to subscribers.
//! Keeps the minimal-dependency
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

/// A single published message from a service.
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

// ── Ratings & Reviews ──────────────────────────────────

/// A subscriber's rating + optional review of a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    /// DID of the service being rated.
    pub service_id: String,
    /// DID of the reviewer (subscriber).
    pub reviewer_did: String,
    /// Score 1–5 (1 = worst, 5 = best).
    pub score: u8,
    /// Optional text review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// When the rating was submitted.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Aggregated rating statistics for a service.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RatingSummary {
    /// Number of ratings received.
    pub count: usize,
    /// Average score (0.0 if no ratings).
    pub average: f64,
    /// Distribution: score_dist[0] = count of 1-star, score_dist[4] = count of 5-star.
    pub score_dist: [usize; 5],
}

impl RatingSummary {
    /// Compute a summary from a slice of ratings.
    pub fn from_ratings(ratings: &[Rating]) -> Self {
        let count = ratings.len();
        if count == 0 {
            return Self::default();
        }
        let sum: usize = ratings.iter().map(|r| r.score as usize).sum();
        let average = sum as f64 / count as f64;
        let mut score_dist = [0usize; 5];
        for r in ratings {
            if (1..=5).contains(&r.score) {
                score_dist[(r.score - 1) as usize] += 1;
            }
        }
        Self {
            count,
            average,
            score_dist,
        }
    }

    /// Render as stars: "★★★★☆ 4.2 (3 ratings)"
    pub fn stars_display(&self) -> String {
        if self.count == 0 {
            return "☆☆☆☆☆  no ratings".to_string();
        }
        let stars: String = (1..=5)
            .map(|i| {
                if self.average >= i as f64 {
                    '★'
                } else {
                    '☆'
                }
            })
            .collect();
        format!(
            "{} {:.1} ({} rating{})",
            stars,
            self.average,
            self.count,
            if self.count == 1 { "" } else { "s" }
        )
    }
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

    // ── Rating tests ─────────────────────────────────────

    fn sample_rating(service_id: &str, reviewer: &str, score: u8) -> Rating {
        Rating {
            service_id: service_id.into(),
            reviewer_did: reviewer.into(),
            score,
            comment: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_rating_summary_empty() {
        let summary = RatingSummary::from_ratings(&[]);
        assert_eq!(summary.count, 0);
        assert_eq!(summary.average, 0.0);
        assert_eq!(summary.score_dist, [0, 0, 0, 0, 0]);
        assert!(summary.stars_display().contains("no ratings"));
    }

    #[test]
    fn test_rating_summary_single() {
        let ratings = [sample_rating("svc-1", "did:a", 4)];
        let summary = RatingSummary::from_ratings(&ratings);
        assert_eq!(summary.count, 1);
        assert!((summary.average - 4.0).abs() < 0.01);
        assert_eq!(summary.score_dist, [0, 0, 0, 1, 0]);
    }

    #[test]
    fn test_rating_summary_multiple() {
        let ratings = [
            sample_rating("svc-1", "did:a", 5),
            sample_rating("svc-1", "did:b", 3),
            sample_rating("svc-1", "did:c", 5),
            sample_rating("svc-1", "did:d", 4),
        ];
        let summary = RatingSummary::from_ratings(&ratings);
        assert_eq!(summary.count, 4);
        assert!((summary.average - 4.25).abs() < 0.01);
        assert_eq!(summary.score_dist, [0, 0, 1, 1, 2]);
    }

    #[test]
    fn test_rating_stars_display() {
        let ratings = [
            sample_rating("svc", "did:a", 5),
            sample_rating("svc", "did:b", 4),
        ];
        let summary = RatingSummary::from_ratings(&ratings);
        let display = summary.stars_display();
        assert!(display.contains("★★★★"));
        assert!(display.contains("4.5"));
        assert!(display.contains("2 ratings"));
    }

    #[test]
    fn test_rating_serde_roundtrip() {
        let rating = Rating {
            service_id: "did:key:svc".into(),
            reviewer_did: "did:key:alice".into(),
            score: 4,
            comment: Some("Great service!".into()),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&rating).unwrap();
        let decoded: Rating = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.service_id, "did:key:svc");
        assert_eq!(decoded.score, 4);
        assert_eq!(decoded.comment.as_deref(), Some("Great service!"));
    }

    #[test]
    fn test_service_permission_serde() {
        let perm = ServicePermission::Whitelist(vec!["did:a".into(), "did:b".into()]);
        let json = serde_json::to_string(&perm).unwrap();
        let decoded: ServicePermission = serde_json::from_str(&json).unwrap();
        assert_eq!(
            decoded,
            ServicePermission::Whitelist(vec!["did:a".into(), "did:b".into()])
        );
    }

    #[test]
    fn test_service_permission_default_is_public() {
        let perm: ServicePermission = serde_json::from_str("\"public\"").unwrap();
        assert_eq!(perm, ServicePermission::Public);
    }

    // ── Wire protocol serde tests ────────────────────────

    #[test]
    fn test_publish_request_serde() {
        let req = PublishRequest {
            service_id: "weather-v1".into(),
            title: "Storm Warning".into(),
            content: "Heavy rain expected".into(),
            content_type: ContentType::Text,
            expected_version: 3,
            signature: "0xdeadbeef".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: PublishRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.service_id, "weather-v1");
        assert_eq!(decoded.expected_version, 3);
        assert_eq!(decoded.content_type, ContentType::Text);
    }

    #[test]
    fn test_publish_response_success() {
        let resp = PublishResponse {
            publication: Some(Publication {
                id: "abc".into(),
                service_id: "svc".into(),
                title: "T".into(),
                content: "C".into(),
                content_type: ContentType::Markdown,
                timestamp: Utc::now(),
                version: 1,
                signature: String::new(),
            }),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: PublishResponse = serde_json::from_str(&json).unwrap();
        assert!(decoded.publication.is_some());
        assert!(decoded.error.is_none());
    }

    #[test]
    fn test_publish_response_error() {
        let resp = PublishResponse {
            publication: None,
            error: Some("signature verification failed".into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: PublishResponse = serde_json::from_str(&json).unwrap();
        assert!(decoded.publication.is_none());
        assert_eq!(decoded.error.unwrap(), "signature verification failed");
    }

    #[test]
    fn test_subscribe_request_serde() {
        let req = SubscribeRequest {
            service_id: "did:key:svc".into(),
            subscriber_did: "did:key:alice".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: SubscribeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.service_id, "did:key:svc");
        assert_eq!(decoded.subscriber_did, "did:key:alice");
    }

    #[test]
    fn test_subscribe_response_serde() {
        let resp = SubscribeResponse {
            accepted: true,
            message: "Subscription confirmed".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: SubscribeResponse = serde_json::from_str(&json).unwrap();
        assert!(decoded.accepted);
        assert_eq!(decoded.message, "Subscription confirmed");

        let rejected = SubscribeResponse {
            accepted: false,
            message: "Whitelist rejection".into(),
        };
        let json = serde_json::to_string(&rejected).unwrap();
        let decoded: SubscribeResponse = serde_json::from_str(&json).unwrap();
        assert!(!decoded.accepted);
    }

    #[test]
    fn test_discover_request_serde() {
        let req = DiscoverRequest {
            service_id: "news-v1".into(),
            limit: 10,
            offset: 5,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: DiscoverRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.service_id, "news-v1");
        assert_eq!(decoded.limit, 10);
        assert_eq!(decoded.offset, 5);
    }

    #[test]
    fn test_discover_request_default_limit() {
        let json = r#"{"service_id":"news-v1"}"#;
        let req: DiscoverRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.limit, 20);
        assert_eq!(req.offset, 0);
    }

    #[test]
    fn test_discover_response_serde() {
        let resp = DiscoverResponse {
            publications: vec![
                Publication {
                    id: "p1".into(),
                    service_id: "svc".into(),
                    title: "First".into(),
                    content: "Hello".into(),
                    content_type: ContentType::Text,
                    timestamp: Utc::now(),
                    version: 2,
                    signature: String::new(),
                },
                Publication {
                    id: "p2".into(),
                    service_id: "svc".into(),
                    title: "Second".into(),
                    content: "World".into(),
                    content_type: ContentType::Markdown,
                    timestamp: Utc::now(),
                    version: 1,
                    signature: "sig".into(),
                },
            ],
            total: 42,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: DiscoverResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.publications.len(), 2);
        assert_eq!(decoded.total, 42);
        assert_eq!(decoded.publications[0].title, "First");
        assert_eq!(decoded.publications[1].content_type, ContentType::Markdown);
    }
}
