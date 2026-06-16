//! Proptest property tests for Publication + ServiceAnnouncement serde.
//!
//! Verifies that random valid input survives serialize → deserialize roundtrip.

use agent_circle_core::publication::{ContentType, Publication, Subscriber, SubscriberList};
use proptest::prelude::*;

// ── Strategies ──────────────────────────────────────────────────

fn content_type_strat() -> impl Strategy<Value = ContentType> {
    prop_oneof![Just(ContentType::Text), Just(ContentType::Markdown)]
}

fn datetime_strat() -> impl Strategy<Value = chrono::DateTime<chrono::Utc>> {
    // Random timestamp within 2020-2030 range
    (1577836800i64..1893456000)
        .prop_map(|ts| chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default())
}

fn publication_strat() -> impl Strategy<Value = Publication> {
    (
        "[a-f0-9]{32}",          // id (hex)
        "[a-zA-Z0-9_\\-]{4,20}", // service_id
        ".{1,30}",               // title
        ".{0,100}",              // content
        content_type_strat(),
        datetime_strat(),
        1u32..1000, // version
        ".*",       // signature (can be empty)
    )
        .prop_map(
            |(id, service_id, title, content, content_type, timestamp, version, signature)| {
                Publication {
                    id,
                    service_id,
                    title,
                    content,
                    content_type,
                    timestamp,
                    version,
                    signature,
                }
            },
        )
}

fn subscriber_strat() -> impl Strategy<Value = Subscriber> {
    (
        "did:key:[a-zA-Z0-9]{10,30}",
        datetime_strat(),
        prop::bool::ANY,
    )
        .prop_map(|(peer_id, subscribed_at, approved)| Subscriber {
            peer_id,
            subscribed_at,
            approved,
        })
}

fn subscriber_list_strat() -> impl Strategy<Value = SubscriberList> {
    (
        "svc-[a-z0-9]{4,10}",
        prop::collection::vec(subscriber_strat(), 0..5),
        datetime_strat(),
    )
        .prop_map(|(service_id, subscribers, last_updated)| SubscriberList {
            service_id,
            subscribers,
            last_updated,
        })
}

// ── Property tests ──────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_publication_serde_roundtrip(pub_msg in publication_strat()) {
        let json = serde_json::to_string(&pub_msg).unwrap();
        let decoded: Publication = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(pub_msg.id, decoded.id);
        prop_assert_eq!(pub_msg.service_id, decoded.service_id);
        prop_assert_eq!(pub_msg.title, decoded.title);
        prop_assert_eq!(pub_msg.content, decoded.content);
        prop_assert_eq!(pub_msg.content_type, decoded.content_type);
        prop_assert_eq!(pub_msg.version, decoded.version);
        prop_assert_eq!(pub_msg.signature, decoded.signature);
    }

    #[test]
    fn prop_subscriber_list_serde_roundtrip(list in subscriber_list_strat()) {
        let json = serde_json::to_string(&list).unwrap();
        let decoded: SubscriberList = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(list.service_id, decoded.service_id);
        prop_assert_eq!(list.subscribers.len(), decoded.subscribers.len());
        for (a, b) in list.subscribers.iter().zip(decoded.subscribers.iter()) {
            prop_assert_eq!(&a.peer_id, &b.peer_id);
            prop_assert_eq!(a.approved, b.approved);
        }
    }

    #[test]
    fn prop_content_type_default_is_text(_ct in content_type_strat()) {
        let default = ContentType::default();
        prop_assert_eq!(default, ContentType::Text);
    }

    #[test]
    fn prop_subscriber_list_add_is_idempotent(n_subscribers in 1usize..5) {
        let mut list = SubscriberList::new("svc-test".into());
        for i in 0..n_subscribers {
            let peer = format!("did:key:peer{}", i);
            prop_assert!(list.subscribe(&peer));
            // Second call should be no-op
            prop_assert!(!list.subscribe(&peer));
        }
        prop_assert_eq!(list.len(), n_subscribers);
        prop_assert_eq!(list.active_count(), n_subscribers);
    }

    #[test]
    fn prop_subscriber_list_remove_restores_correct_size(
        peers in prop::collection::vec("did:key:[a-z]{3,8}", 1..4)
    ) {
        let mut list = SubscriberList::new("svc-test".into());
        let n = peers.len();
        for p in &peers {
            list.subscribe(p);
        }
        prop_assert_eq!(list.len(), n);

        // Remove all
        for p in &peers {
            prop_assert!(list.unsubscribe(p));
        }
        prop_assert_eq!(list.len(), 0);
        prop_assert!(list.is_empty());
    }
}
