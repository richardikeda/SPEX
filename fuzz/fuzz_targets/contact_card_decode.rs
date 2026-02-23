#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_core::types::ContactCard;

fuzz_target!(|data: &[u8]| {
    let _ = ContactCard::decode_ctap2(data);
});
