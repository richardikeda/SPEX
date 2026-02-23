#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_bridge::parse_inbox_store_request_bytes;

fuzz_target!(|data: &[u8]| {
    let _ = parse_inbox_store_request_bytes(data);
});
