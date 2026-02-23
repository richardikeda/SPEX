#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_core::types::GrantToken;

fuzz_target!(|data: &[u8]| {
    let _ = GrantToken::decode_ctap2(data);
});
