use rand::RngCore;
use spex_core::hash::HashId;
use spex_transport::chunking::{chunk_data, reassemble_chunks, ChunkingConfig};
use std::time::{Duration, Instant};

const LARGE_PAYLOAD_MAX_CHUNK_MS: u64 = 3_000;
const LARGE_PAYLOAD_MAX_REASSEMBLE_MS: u64 = 3_000;

#[test]
fn stress_test_large_payload_chunking() {
    let payload_size = 10 * 1024 * 1024; // 10 MB
    let mut payload = vec![0u8; payload_size];
    rand::thread_rng().fill_bytes(&mut payload);

    let config = ChunkingConfig {
        chunk_size: 64 * 1024, // 64 KB
        hash_id: HashId::Sha256,
    };

    // Split
    let split_start = Instant::now();
    let chunks = chunk_data(&config, &payload);
    let split_elapsed = split_start.elapsed();
    // Correct calculation: (10*1024*1024 + 64*1024 - 1) / (64*1024) = 160
    assert_eq!(chunks.len(), payload_size.div_ceil(config.chunk_size));
    assert!(split_elapsed <= Duration::from_millis(LARGE_PAYLOAD_MAX_CHUNK_MS));

    // Reassemble
    let reassemble_start = Instant::now();
    let reassembled = reassemble_chunks(&chunks);
    let reassemble_elapsed = reassemble_start.elapsed();
    assert_eq!(reassembled, payload);
    assert!(reassemble_elapsed <= Duration::from_millis(LARGE_PAYLOAD_MAX_REASSEMBLE_MS));
}

#[test]
fn stress_test_tiny_chunks_many() {
    let payload_size = 1024 * 1024; // 1 MB
    let mut payload = vec![0u8; payload_size];
    rand::thread_rng().fill_bytes(&mut payload);

    let config = ChunkingConfig {
        chunk_size: 1024,
        hash_id: HashId::Sha256,
    };

    let chunks = chunk_data(&config, &payload);
    assert_eq!(chunks.len(), 1024);

    let reassembled = reassemble_chunks(&chunks);
    assert_eq!(reassembled, payload);
}
