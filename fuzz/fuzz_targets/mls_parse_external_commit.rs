#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_mls::parse_external_commit;

// Fuzzes external commit parsing to ensure malformed untrusted payloads never panic.
fuzz_target!(|data: &[u8]| {
    let _ = parse_external_commit(data, 1);
});
