// SPDX-License-Identifier: MPL-2.0
#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_core::cbor::parse_cbor_payload;

fuzz_target!(|data: &[u8]| {
    let _ = parse_cbor_payload(data);
});
