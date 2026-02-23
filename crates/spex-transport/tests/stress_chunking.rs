use spex_transport::chunking::{chunk_data, reassemble_chunks, ChunkingConfig};
use spex_core::hash::HashId;
use rand::RngCore;

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
    let chunks = chunk_data(&config, &payload);
    // Correct calculation: (10*1024*1024 + 64*1024 - 1) / (64*1024) = 160
    assert_eq!(chunks.len(), (payload_size + config.chunk_size - 1) / config.chunk_size);

    // Reassemble
    let reassembled = reassemble_chunks(&chunks);
    assert_eq!(reassembled, payload);
}

#[test]
fn stress_test_tiny_chunks_many() {
    let payload_size = 1 * 1024 * 1024; // 1 MB
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
