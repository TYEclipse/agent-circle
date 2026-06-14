#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Publication deserialization — must not panic on arbitrary input
    let _ = serde_json::from_slice::<agent_circle_core::publication::Publication>(data);

    // PublishRequest deserialization — must not panic
    let _ = serde_json::from_slice::<agent_circle_core::publication::PublishRequest>(data);

    // SubscribeRequest deserialization — must not panic
    let _ = serde_json::from_slice::<agent_circle_core::publication::SubscribeRequest>(data);

    // DiscoverRequest deserialization — must not panic
    let _ = serde_json::from_slice::<agent_circle_core::publication::DiscoverRequest>(data);

    // Rating deserialization — must not panic
    let _ = serde_json::from_slice::<agent_circle_core::publication::Rating>(data);

    // ServicePermission deserialization — must not panic
    let _ = serde_json::from_slice::<agent_circle_core::publication::ServicePermission>(data);

    // SubscriberList deserialization — must not panic
    let _ = serde_json::from_slice::<agent_circle_core::publication::SubscriberList>(data);
});
