#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_transport::{validate_p2p_puzzle_payload, P2pPuzzlePayload};

fuzz_target!(|data: &[u8]| {
    if let Ok(payload) = serde_json::from_slice::<P2pPuzzlePayload>(data) {
        let _ = validate_p2p_puzzle_payload(&payload);
    }
});
