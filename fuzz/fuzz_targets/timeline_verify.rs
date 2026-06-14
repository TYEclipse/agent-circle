#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // R45: arbitrary bytes → Timeline deser + verify
    // Timeline::verify() should never panic, only return Ok/Err.
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(tl) = serde_json::from_str::<agent_circle::timeline::Timeline>(s) {
            let _ = tl.verify();
        }
    }
});
