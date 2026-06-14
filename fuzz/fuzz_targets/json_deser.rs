#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // R42: arbitrary JSON bytes → deserialize into all message types
    // Must never panic; failures should return Err, not crash.

    // Try to parse as ChatRequest
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<agent_circle::chat::ChatRequest>(s);
        let _ = serde_json::from_str::<agent_circle::chat::ChatResponse>(s);
    }

    // AgentCard
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<agent_circle::identity::AgentCard>(s);
    }

    // TimelineNode + Timeline
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<agent_circle::timeline::TimelineNode>(s);
        let _ = serde_json::from_str::<agent_circle::timeline::Timeline>(s);
    }
});
