#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // R46: arbitrary bytes → AgentCard deser + verify
    // AgentCard::verify() should never panic, only return Ok/Err.
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(card) = serde_json::from_str::<agent_circle::identity::AgentCard>(s) {
            let _ = card.verify();
        }
    }
});
