#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_transport::{validate_p2p_grant_payload, P2pGrantPayload};

fuzz_target!(|data: &[u8]| {
    if let Ok(payload) = serde_json::from_slice::<P2pGrantPayload>(data) {
        let _ = validate_p2p_grant_payload(1_700_000_000, &payload);
    }
});
