#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // R43: arbitrary bytes → decode_did_key
    // Must never panic regardless of input.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = agent_circle::identity::decode_did_key(s);
    }
});
