// SPDX-License-Identifier: MPL-2.0
#![no_main]

use libfuzzer_sys::fuzz_target;
use spex_transport::{parse_manifest_from_gossip, recover_manifest_from_gossip};

// Fuzzes manifest parsing and recovery boundaries for untrusted gossip payloads.
fuzz_target!(|data: &[u8]| {
    let _ = parse_manifest_from_gossip(data);

    let payloads = vec![
        data.to_vec(),
        b"{\"chunks\":[],\"total_len\":0}".to_vec(),
    ];
    let _ = recover_manifest_from_gossip(&payloads);
});
